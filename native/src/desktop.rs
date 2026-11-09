use gtk::{gdk, gio, glib, prelude::*};
use pin_native::{
    board::{valid_date, Board, Task, COLORS},
    storage,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection,
    protocol::xproto::{ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, EventMask},
};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Settings {
    width: i32,
    height: i32,
    x: Option<i32>,
    y: Option<i32>,
    pinned: Option<bool>,
    folded: HashSet<String>,
}

#[derive(Clone)]
struct Selection {
    id: u64,
    caret: i32,
    anchor: i32,
    scroll: f64,
}

#[derive(Clone)]
struct Snapshot {
    board: Board,
    selection: Option<Selection>,
}

#[derive(Clone)]
struct NoteSnapshot {
    text: String,
    caret: i32,
    anchor: i32,
    scroll: f64,
}

struct State {
    board: Board,
    disk: Option<String>,
    clean: String,
    conflict: bool,
    readable: bool,
    save_error: bool,
    dirty_at: Instant,
    today: String,
    next_id: u64,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    typing: Option<(u64, Instant)>,
    before_selection: Option<Selection>,
    note_undo: Vec<NoteSnapshot>,
    note_redo: Vec<NoteSnapshot>,
    before_note: Option<NoteSnapshot>,
    note_previous: String,
    settings: Settings,
}

struct App {
    notes: bool,
    file: PathBuf,
    config: PathBuf,
    window: gtk::ApplicationWindow,
    list: gtk::Box,
    scroller: gtk::ScrolledWindow,
    note: gtk::TextView,
    notice: gtk::Box,
    message: gtk::Label,
    use_disk: gtk::Button,
    use_mine: gtk::Button,
    retry: gtk::Button,
    rows: RefCell<HashMap<u64, gtk::TextView>>,
    state: RefCell<State>,
    loading: Cell<bool>,
    css: gtk::CssProvider,
    _appearance: Option<gio::Settings>,
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
fn config_root() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}
fn expand(path: &str) -> PathBuf {
    path.strip_prefix("~/")
        .map_or_else(|| PathBuf::from(path), |p| home().join(p))
}
fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn data_file(notes: bool) -> PathBuf {
    let env_key = if notes {
        "STICKY_NOTES_FILE"
    } else {
        "PRIORITIES_FILE"
    };
    if let Some(path) = std::env::var_os(env_key) {
        return PathBuf::from(path);
    }
    let settings = config_root()
        .join(if notes { "StickyNotes" } else { "Pin" })
        .join("settings.json");
    if let Ok(text) = std::fs::read_to_string(settings) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(path) = value
                .get("file")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                return expand(path);
            }
        }
    }
    let documents = std::fs::read_to_string(config_root().join("user-dirs.dirs"))
        .ok()
        .and_then(|s| {
            s.lines().find_map(|line| {
                line.strip_prefix("XDG_DOCUMENTS_DIR=").map(|v| {
                    v.trim_matches('"')
                        .replace("$HOME", &home().to_string_lossy())
                })
            })
        })
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("Documents"));
    documents.join(if notes { "Sticky Notes.txt" } else { "Pin.md" })
}

pub fn run() -> glib::ExitCode {
    let notes = std::env::args().any(|a| a == "--notes")
        || std::env::args()
            .next()
            .is_some_and(|a| a.ends_with("sticky-notes"));
    let app = gtk::Application::builder()
        .application_id(if notes {
            "io.github.masihmoafi.StickyNotes"
        } else {
            "io.github.masihmoafi.Pin.Native"
        })
        .build();
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        App::open(app, notes);
    });
    app.run_with_args::<&str>(&[])
}

impl App {
    fn open(application: &gtk::Application, notes: bool) {
        let appearance = gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup("org.gnome.desktop.interface", true))
            .filter(|schema| schema.has_key("color-scheme"))
            .map(|schema| gio::Settings::new_full(&schema, gio::SettingsBackend::NONE, None));
        if let Some(appearance) = &appearance {
            apply_appearance(appearance);
            appearance.connect_changed(Some("color-scheme"), |settings, _| {
                apply_appearance(settings)
            });
        }
        let file = data_file(notes);
        let config = config_root()
            .join(if notes { "StickyNotes" } else { "Pin" })
            .join("native.json");
        let settings = std::fs::read_to_string(&config)
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .unwrap_or_default();
        let (disk, readable) = match storage::read(&file) {
            Ok(v) => (v, true),
            Err(e) => {
                eprintln!("Read {}: {e}", file.display());
                (None, false)
            }
        };
        let clean = disk.clone().unwrap_or_default();
        let mut board = Board::parse(&clean);
        let day = today();
        if !notes {
            board.rollover(&day);
            board.completed_last();
        }
        let next_id = board.tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let window = gtk::ApplicationWindow::builder()
            .application(application)
            .title(if notes { "Sticky Notes" } else { "Pin" })
            .default_width(if settings.width >= 280 {
                settings.width
            } else {
                460
            })
            .default_height(if settings.height >= 180 {
                settings.height
            } else if notes {
                380
            } else {
                720
            })
            .build();
        window.set_size_request(280, 180);
        window.add_css_class("pin-window");
        let header = gtk::HeaderBar::builder()
            .show_title_buttons(true)
            .decoration_layout("menu:minimize,maximize,close")
            .build();
        let title = gtk::Label::new(Some(if notes { "Notes" } else { "Pin" }));
        header.set_title_widget(Some(&title));
        let pin = gtk::ToggleButton::new();
        pin.set_tooltip_text(Some("Keep this window on top"));
        pin.set_active(settings.pinned.unwrap_or(true));
        pin.set_child(Some(&pin_image(pin.is_active())));
        header.pack_start(&pin);
        let add = gtk::Button::from_icon_name("list-add-symbolic");
        add.set_tooltip_text(Some("New task (Ctrl+N)"));
        if !notes {
            header.pack_start(&add);
        }
        let menu = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .build();
        let model = gio::Menu::new();
        model.append(Some("Undo"), Some("win.undo"));
        model.append(Some("Redo"), Some("win.redo"));
        if !notes {
            model.append(Some("New task"), Some("win.new"));
        }
        menu.set_menu_model(Some(&model));
        header.pack_end(&menu);
        window.set_titlebar(Some(&header));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let notice = gtk::Box::new(gtk::Orientation::Vertical, 6);
        notice.set_margin_start(12);
        notice.set_margin_end(12);
        notice.set_margin_top(8);
        notice.set_margin_bottom(8);
        let message = gtk::Label::new(None);
        message.set_wrap(true);
        message.set_xalign(0.0);
        let choices = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let use_disk = gtk::Button::with_label("Use disk");
        let use_mine = gtk::Button::with_label("Keep mine");
        let retry = gtk::Button::with_label("Retry");
        choices.append(&use_disk);
        choices.append(&use_mine);
        choices.append(&retry);
        notice.append(&message);
        notice.append(&choices);
        notice.set_visible(false);
        root.append(&notice);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .hexpand(true)
            .build();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        list.set_margin_start(12);
        list.set_margin_end(12);
        list.set_margin_top(8);
        list.set_margin_bottom(12);
        let note = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::WordChar)
            .left_margin(16)
            .right_margin(16)
            .top_margin(12)
            .bottom_margin(16)
            .vexpand(true)
            .hexpand(true)
            .build();
        note.add_css_class("scratch");
        note.buffer().set_text(&clean);
        note.buffer().set_enable_undo(false);
        let note_menu = gio::Menu::new();
        note_menu.append(Some("Undo"), Some("win.undo"));
        note_menu.append(Some("Redo"), Some("win.redo"));
        note.set_extra_menu(Some(&note_menu));
        if notes {
            scroller.set_child(Some(&note));
        } else {
            scroller.set_child(Some(&list));
        }
        root.append(&scroller);
        window.set_child(Some(&root));
        let css = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &WidgetExt::display(&window),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let this = Rc::new(Self {
            notes,
            file,
            config,
            window,
            list,
            scroller,
            note,
            notice,
            message,
            use_disk,
            use_mine,
            retry,
            rows: RefCell::new(HashMap::new()),
            state: RefCell::new(State {
                board,
                disk,
                note_previous: clean.clone(),
                clean,
                conflict: false,
                readable,
                save_error: false,
                dirty_at: Instant::now(),
                today: day,
                next_id,
                undo: Vec::new(),
                redo: Vec::new(),
                typing: None,
                before_selection: None,
                note_undo: Vec::new(),
                note_redo: Vec::new(),
                before_note: None,
                settings,
            }),
            loading: Cell::new(false),
            css,
            _appearance: appearance,
        });
        for (name, redo) in [("undo", false), ("redo", true)] {
            let action = gio::SimpleAction::new(name, None);
            let a = this.clone();
            action.connect_activate(move |_, _| a.history(redo));
            this.window.add_action(&action);
        }
        let action = gio::SimpleAction::new("new", None);
        let a = this.clone();
        action.connect_activate(move |_, _| a.add_task(None));
        this.window.add_action(&action);
        let a = this.clone();
        add.connect_clicked(move |_| a.add_task(None));
        let a = this.clone();
        pin.connect_toggled(move |b| {
            b.set_child(Some(&pin_image(b.is_active())));
            a.state.borrow_mut().settings.pinned = Some(b.is_active());
            a.set_top(b.is_active());
            a.save_settings();
        });
        let a = this.clone();
        this.window.connect_map(move |_| a.restore_window());
        let a = this.clone();
        this.window.connect_close_request(move |_| {
            a.save_settings();
            if a.flush() {
                glib::Propagation::Proceed
            } else {
                a.show_notice();
                glib::Propagation::Stop
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let a = this.clone();
        keys.connect_key_pressed(move |_, key, _, modifiers| a.global_key(key, modifiers));
        this.window.add_controller(keys);
        let a = this.clone();
        this.note.buffer().connect_begin_user_action(move |_| {
            if !a.loading.get() {
                let before = a.note_snapshot();
                a.state.borrow_mut().before_note = Some(before);
            }
        });
        let a = this.clone();
        this.note
            .buffer()
            .connect_changed(move |_| a.note_changed());
        let a = this.clone();
        this.use_disk.connect_clicked(move |_| a.load_latest(true));
        let a = this.clone();
        this.use_mine.connect_clicked(move |_| {
            match storage::read(&a.file) {
                Ok(disk) => {
                    let mut s = a.state.borrow_mut();
                    s.disk = disk;
                    s.conflict = false;
                    s.clean = "\0".into();
                    drop(s);
                    a.flush();
                }
                Err(e) => {
                    eprintln!("Read {}: {e}", a.file.display());
                    a.state.borrow_mut().readable = false;
                }
            }
            a.show_notice();
        });
        let a = this.clone();
        this.retry.connect_clicked(move |_| {
            a.load_latest(false);
            a.flush();
        });
        this.render(None);
        if let Some(settings) = gtk::Settings::default() {
            let weak = Rc::downgrade(&this);
            let pin_weak = pin.downgrade();
            settings.connect_gtk_application_prefer_dark_theme_notify(move |_| {
                if let Some(app) = weak.upgrade() {
                    if let Some(pin) = pin_weak.upgrade() {
                        pin.set_child(Some(&pin_image(pin.is_active())));
                    }
                    let focus = app.selection();
                    app.render(focus);
                }
            });
        }
        this.note.set_sensitive(readable);
        this.list.set_sensitive(readable);
        this.show_notice();
        let a = this.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            if !a.window.is_visible() {
                return glib::ControlFlow::Break;
            }
            a.tick();
            glib::ControlFlow::Continue
        });
        this.window.present();
    }

    fn text(&self) -> String {
        if self.notes {
            self.note
                .buffer()
                .text(
                    &self.note.buffer().start_iter(),
                    &self.note.buffer().end_iter(),
                    true,
                )
                .to_string()
        } else {
            self.state.borrow().board.markdown()
        }
    }

    fn selection(&self) -> Option<Selection> {
        self.rows.borrow().iter().find_map(|(id, view)| {
            if !view.has_focus() {
                return None;
            }
            let buffer = view.buffer();
            Some(Selection {
                id: *id,
                caret: buffer.cursor_position(),
                anchor: buffer.iter_at_mark(&buffer.selection_bound()).offset(),
                scroll: self.scroller.vadjustment().value(),
            })
        })
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            board: self.state.borrow().board.clone(),
            selection: self.selection(),
        }
    }

    fn note_snapshot(&self) -> NoteSnapshot {
        let buffer = self.note.buffer();
        NoteSnapshot {
            text: buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .to_string(),
            caret: buffer.cursor_position(),
            anchor: buffer.iter_at_mark(&buffer.selection_bound()).offset(),
            scroll: self.scroller.vadjustment().value(),
        }
    }

    fn note_changed(&self) {
        if self.loading.get() {
            return;
        }
        let now = self.note_snapshot();
        let mut s = self.state.borrow_mut();
        if now.text == s.note_previous {
            return;
        }
        if !s
            .typing
            .is_some_and(|(_, at)| at.elapsed() < Duration::from_millis(600))
        {
            let before = s.before_note.take().unwrap_or_else(|| NoteSnapshot {
                text: s.note_previous.clone(),
                ..now.clone()
            });
            s.note_undo.push(before);
            if s.note_undo.len() > 300 {
                s.note_undo.remove(0);
            }
            s.note_redo.clear();
        }
        s.note_previous = now.text;
        s.typing = Some((0, Instant::now()));
        s.dirty_at = Instant::now();
    }

    fn change(self: &Rc<Self>, focus: Option<Selection>, f: impl FnOnce(&mut State)) {
        let before = self.snapshot();
        let mut s = self.state.borrow_mut();
        f(&mut s);
        s.board.completed_last();
        if s.board != before.board {
            s.undo.push(before);
            if s.undo.len() > 300 {
                s.undo.remove(0);
            }
            s.redo.clear();
            s.typing = None;
            s.dirty_at = Instant::now();
        }
        drop(s);
        self.render(focus);
    }

    fn history(self: &Rc<Self>, redo: bool) {
        if self.notes {
            let current = self.note_snapshot();
            let mut s = self.state.borrow_mut();
            let next = if redo {
                s.note_redo.pop()
            } else {
                s.note_undo.pop()
            };
            let Some(next) = next else {
                return;
            };
            if redo {
                s.note_undo.push(current);
            } else {
                s.note_redo.push(current);
            }
            s.typing = None;
            s.note_previous = next.text.clone();
            s.dirty_at = Instant::now();
            drop(s);
            self.loading.set(true);
            let buffer = self.note.buffer();
            buffer.set_text(&next.text);
            let max = buffer.char_count();
            buffer.select_range(
                &buffer.iter_at_offset(next.caret.clamp(0, max)),
                &buffer.iter_at_offset(next.anchor.clamp(0, max)),
            );
            self.note.grab_focus();
            let adjustment = self.scroller.vadjustment();
            glib::idle_add_local_once(move || adjustment.set_value(next.scroll));
            self.loading.set(false);
            return;
        }
        let current = self.snapshot();
        let mut s = self.state.borrow_mut();
        let next = if redo { s.redo.pop() } else { s.undo.pop() };
        let Some(next) = next else {
            return;
        };
        if redo {
            s.undo.push(current);
        } else {
            s.redo.push(current);
        }
        s.board = next.board;
        s.typing = None;
        s.dirty_at = Instant::now();
        drop(s);
        self.render(next.selection);
    }

    fn focus(id: u64, caret: i32) -> Selection {
        Selection {
            id,
            caret,
            anchor: caret,
            scroll: -1.0,
        }
    }

    fn add_task(self: &Rc<Self>, parent: Option<u64>) {
        let id = self.state.borrow().next_id;
        self.change(Some(Self::focus(id, 0)), |s| {
            s.next_id += 1;
            let i = parent.and_then(|id| s.board.index(id));
            let depth = i.map_or(0, |i| s.board.tasks[i].depth + 1);
            let color = i
                .and_then(|i| s.board.tasks[i].color.clone())
                .or_else(|| s.board.tasks.first().and_then(|t| t.color.clone()));
            let task = Task::blank(id, depth, color);
            if let Some(i) = i {
                s.settings.folded.remove(&Self::fold_key(&s.board, i));
            }
            s.board.tasks.insert(i.map_or(0, |i| i + 1), task);
        });
    }

    fn fold_key(board: &Board, i: usize) -> String {
        let mut keys = vec![board.tasks[i].text.as_str()];
        let mut parent = board.parent(i);
        while let Some(p) = parent {
            keys.push(&board.tasks[p].text);
            parent = board.parent(p);
        }
        keys.reverse();
        keys.join("\u{1f}")
    }

    fn fold(self: &Rc<Self>, id: u64) {
        let focus = self.selection();
        let mut s = self.state.borrow_mut();
        if let Some(i) = s.board.index(id) {
            let key = Self::fold_key(&s.board, i);
            if !s.settings.folded.remove(&key) {
                s.settings.folded.insert(key);
            }
        }
        drop(s);
        self.render(focus);
        self.save_settings();
    }

    fn render(self: &Rc<Self>, focus: Option<Selection>) {
        self.loading.set(true);
        let s = self.state.borrow();
        let dark =
            gtk::Settings::default().is_some_and(|s| s.is_gtk_application_prefer_dark_theme());
        let mut css = String::from(".pin-window { background: @theme_bg_color; } textview.task, textview.task text { background: transparent; } textview { font-size: 15px; } .task-row { border-radius: 6px; padding: 5px 4px; } .task-row button { min-width: 20px; min-height: 22px; padding: 0 3px; } .task-row.done textview { opacity: 0.6; } .task-row.drop-before { border-top: 2px solid @theme_selected_bg_color; } .task-row.drop-after { border-bottom: 2px solid @theme_selected_bg_color; } .when { font-size: 11px; opacity: 0.8; } .today { color: #16a34a; } .late { color: #dc2626; } .grip { opacity: 0.5; } .scratch, .scratch text { background: @theme_base_color; }");
        for task in &s.board.tasks {
            if let Some(color) = task.color.as_deref().and_then(|c| {
                COLORS
                    .iter()
                    .find(|(m, _)| *m == c)
                    .map(|(_, hex)| *hex)
                    .or_else(|| c.starts_with('#').then_some(c))
            }) {
                css.push_str(&format!(
                    ".t{} {{ background: alpha({}, {}); }}",
                    task.id,
                    color,
                    if dark { "0.24" } else { "0.13" }
                ));
            }
        }
        self.css.load_from_data(&css);
        if self.notes {
            self.loading.set(false);
            return;
        }
        self.rows.borrow_mut().clear();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        if s.board.tasks.is_empty() {
            let label = gtk::Label::new(Some("Add a task with + or Ctrl+N."));
            label.set_wrap(true);
            label.set_margin_top(24);
            self.list.append(&label);
        }
        let mut hidden_depth = None;
        for (i, task) in s.board.tasks.iter().enumerate() {
            if hidden_depth.is_some_and(|depth| task.depth > depth) {
                continue;
            }
            hidden_depth = None;
            let children = s.board.end(i) > i + 1;
            let folded = s.settings.folded.contains(&Self::fold_key(&s.board, i));
            if folded {
                hidden_depth = Some(task.depth);
            }
            self.row(task, children, folded, &s.today);
        }
        drop(s);
        self.loading.set(false);
        if let Some(focus) = focus {
            if let Some(view) = self.rows.borrow().get(&focus.id) {
                view.grab_focus();
                let buffer = view.buffer();
                let max = buffer.char_count();
                buffer.select_range(
                    &buffer.iter_at_offset(focus.caret.clamp(0, max)),
                    &buffer.iter_at_offset(focus.anchor.clamp(0, max)),
                );
                if focus.scroll >= 0.0 {
                    let adjustment = self.scroller.vadjustment();
                    glib::idle_add_local_once(move || adjustment.set_value(focus.scroll));
                } else {
                    view.scroll_mark_onscreen(&buffer.get_insert());
                }
            }
        }
    }

    fn row(self: &Rc<Self>, task: &Task, children: bool, folded: bool, day: &str) {
        let id = task.id;
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        row.add_css_class("task-row");
        row.add_css_class(&format!("t{id}"));
        if task.done {
            row.add_css_class("done");
        }
        row.set_margin_start((task.depth.min(8) * 18) as i32);
        let fold = gtk::Button::with_label(if children {
            if folded {
                "▸"
            } else {
                "▾"
            }
        } else {
            " "
        });
        fold.set_sensitive(children);
        fold.set_has_frame(false);
        fold.set_tooltip_text(Some(if folded {
            "Show subtasks (Ctrl+.)"
        } else {
            "Hide subtasks (Ctrl+.)"
        }));
        let a = self.clone();
        fold.connect_clicked(move |_| a.fold(id));
        row.append(&fold);
        let check = gtk::CheckButton::new();
        check.set_active(task.done);
        check.set_valign(gtk::Align::Start);
        check.set_tooltip_text(Some("Done (Ctrl+Enter)"));
        let a = self.clone();
        check.connect_toggled(move |_| a.toggle(id));
        row.append(&check);
        let text = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::WordChar)
            .hexpand(true)
            .accepts_tab(false)
            .top_margin(2)
            .bottom_margin(2)
            .build();
        text.add_css_class("task");
        text.set_size_request(40, 26);
        text.buffer().set_text(&task.text);
        text.buffer().set_enable_undo(false);
        text.buffer().connect_insert_text(|buffer, iter, value| {
            if value.contains(['\n', '\r']) {
                buffer.stop_signal_emission_by_name("insert-text");
                buffer.insert(iter, &value.replace(['\n', '\r'], " "));
            }
        });
        let extra = gio::Menu::new();
        extra.append(Some("Undo"), Some("win.undo"));
        extra.append(Some("Redo"), Some("win.redo"));
        text.set_extra_menu(Some(&extra));
        let a = self.clone();
        text.buffer().connect_begin_user_action(move |_| {
            if !a.loading.get() {
                a.state.borrow_mut().before_selection = a.selection();
            }
        });
        let a = self.clone();
        text.buffer().connect_changed(move |buffer| {
            if a.loading.get() {
                return;
            }
            let value = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .to_string();
            let mut s = a.state.borrow_mut();
            let Some(i) = s.board.index(id) else {
                return;
            };
            if s.board.tasks[i].text == value {
                return;
            }
            let new_step = !s
                .typing
                .is_some_and(|(last, at)| last == id && at.elapsed() < Duration::from_millis(600));
            if new_step {
                let before = Snapshot {
                    board: s.board.clone(),
                    selection: s.before_selection.clone(),
                };
                s.undo.push(before);
                if s.undo.len() > 300 {
                    s.undo.remove(0);
                }
                s.redo.clear();
            }
            s.board.tasks[i].text = value;
            s.typing = Some((id, Instant::now()));
            s.dirty_at = Instant::now();
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let a = self.clone();
        keys.connect_key_pressed(move |_, key, _, modifiers| a.task_key(id, key, modifiers));
        text.add_controller(keys);
        self.rows.borrow_mut().insert(id, text.clone());
        row.append(&text);
        let metadata = gtk::Box::new(gtk::Orientation::Vertical, 0);
        metadata.set_valign(gtk::Align::Center);
        if task.daily {
            let daily = gtk::Label::new(Some("Daily"));
            daily.add_css_class("when");
            metadata.append(&daily);
        }
        if let Some(date) = &task.date {
            let label = gtk::Label::new(Some(&display_date(date)));
            label.add_css_class("when");
            label.set_wrap(true);
            label.set_max_width_chars(15);
            if date.get(..10) == Some(day) {
                label.add_css_class("today");
            } else if date.get(..10).is_some_and(|d| d < day) && !task.done {
                label.add_css_class("late");
            }
            metadata.append(&label);
        }
        row.append(&metadata);
        let grip = gtk::Label::new(Some("⠿"));
        grip.add_css_class("grip");
        grip.set_tooltip_text(Some("Drag to reorder (Alt+↑ / Alt+↓)"));
        grip.set_cursor_from_name(Some("grab"));
        let drag = gtk::DragSource::builder()
            .actions(gdk::DragAction::MOVE)
            .build();
        drag.connect_prepare(move |_, _, _| {
            Some(gdk::ContentProvider::for_value(&id.to_string().to_value()))
        });
        grip.add_controller(drag);
        row.append(&grip);
        let drop = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
        let r = row.downgrade();
        drop.connect_motion(move |_, _, y| {
            if let Some(row) = r.upgrade() {
                row.remove_css_class("drop-before");
                row.remove_css_class("drop-after");
                row.add_css_class(if y < f64::from(row.height()) / 2.0 {
                    "drop-before"
                } else {
                    "drop-after"
                });
            }
            gdk::DragAction::MOVE
        });
        let r = row.downgrade();
        drop.connect_leave(move |_| {
            if let Some(row) = r.upgrade() {
                row.remove_css_class("drop-before");
                row.remove_css_class("drop-after");
            }
        });
        let a = self.clone();
        let r = row.downgrade();
        drop.connect_drop(move |_, value, _, y| {
            let Some(moving) = value
                .get::<String>()
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
            else {
                return false;
            };
            let after = r.upgrade().is_some_and(|row| {
                row.remove_css_class("drop-before");
                row.remove_css_class("drop-after");
                y >= f64::from(row.height()) / 2.0
            });
            a.change(None, |s| {
                s.board.move_branch(moving, id, after);
            });
            true
        });
        row.add_controller(drop);
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .has_frame(false)
            .tooltip_text("Task options")
            .valign(gtk::Align::Start)
            .build();
        more.set_popover(Some(&self.options(task)));
        row.append(&more);
        self.list.append(&row);
    }

    fn options(self: &Rc<Self>, task: &Task) -> gtk::Popover {
        let id = task.id;
        let pop = gtk::Popover::new();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        let daily = gtk::CheckButton::with_label("Repeat daily");
        daily.set_active(task.daily);
        let a = self.clone();
        let p = pop.downgrade();
        daily.connect_toggled(move |button| {
            let active = button.is_active();
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            a.change(None, |s| {
                if let Some(i) = s.board.index(id) {
                    let t = &mut s.board.tasks[i];
                    t.daily = active;
                    t.completed = if active && t.done {
                        Some(s.today.clone())
                    } else {
                        None
                    };
                }
            });
        });
        content.append(&daily);
        let date = gtk::Entry::builder()
            .placeholder_text("YYYY-MM-DD HH:MM")
            .text(task.date.as_deref().unwrap_or(""))
            .tooltip_text("Date, optionally followed by a time")
            .build();
        let date_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        date_row.append(&date);
        let save_date = gtk::Button::with_label("Set");
        date_row.append(&save_date);
        let apply = {
            let a = self.clone();
            let d = date.downgrade();
            let p = pop.downgrade();
            move || {
                let Some(d) = d.upgrade() else {
                    return;
                };
                let value = d.text().trim().to_owned();
                if !valid_date(&value) {
                    d.add_css_class("error");
                    d.set_tooltip_text(Some("Use YYYY-MM-DD, optionally HH:MM (24-hour time)."));
                    return;
                }
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                a.change(None, |s| {
                    if let Some(i) = s.board.index(id) {
                        s.board.tasks[i].date = if value.is_empty() { None } else { Some(value) };
                    }
                });
            }
        };
        let apply = Rc::new(apply);
        let f = apply.clone();
        save_date.connect_clicked(move |_| f());
        date.connect_activate(move |_| apply());
        content.append(&date_row);
        let palette = gtk::Grid::builder()
            .column_spacing(4)
            .row_spacing(4)
            .build();
        for (i, (marker, hex)) in COLORS.iter().enumerate() {
            let button = gtk::Button::with_label("●");
            button.set_tooltip_text(Some(hex));
            let provider = gtk::CssProvider::new();
            provider.load_from_data(&format!(
                "button {{ color: {hex}; font-size: 22px; padding: 0 5px; }}"
            ));
            button
                .style_context()
                .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            let a = self.clone();
            let p = pop.downgrade();
            let marker = marker.to_string();
            button.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                a.change(None, |s| {
                    if let Some(i) = s.board.index(id) {
                        s.board.tasks[i].color = Some(marker.clone());
                    }
                });
            });
            palette.attach(&button, (i % 6) as i32, (i / 6) as i32, 1, 1);
        }
        content.append(&palette);
        let custom = gtk::ColorButton::new();
        custom.set_title("Task color");
        if let Some(color) = task.color.as_deref().and_then(|c| {
            COLORS
                .iter()
                .find(|(m, _)| *m == c)
                .map(|(_, h)| *h)
                .or(Some(c))
        }) {
            if let Ok(rgba) = gdk::RGBA::parse(color) {
                custom.set_rgba(&rgba);
            }
        }
        let a = self.clone();
        let p = pop.downgrade();
        custom.connect_color_set(move |b| {
            let c = b.rgba();
            let color = format!(
                "#{:02x}{:02x}{:02x}",
                (c.red() * 255.0).round() as u8,
                (c.green() * 255.0).round() as u8,
                (c.blue() * 255.0).round() as u8
            );
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            a.change(None, |s| {
                if let Some(i) = s.board.index(id) {
                    s.board.tasks[i].color = Some(color);
                }
            });
        });
        let colors = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let none = gtk::Button::with_label("No color");
        colors.append(&none);
        colors.append(&custom);
        content.append(&colors);
        let a = self.clone();
        let p = pop.downgrade();
        none.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            a.change(None, |s| {
                if let Some(i) = s.board.index(id) {
                    s.board.tasks[i].color = None;
                }
            });
        });
        let sub = gtk::Button::with_label("Add subtask (Ctrl+T)");
        let a = self.clone();
        let p = pop.downgrade();
        sub.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            a.add_task(Some(id));
        });
        content.append(&sub);
        let delete = gtk::Button::with_label("Delete (Ctrl+Shift+Backspace)");
        let a = self.clone();
        let p = pop.downgrade();
        delete.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            a.remove(id);
        });
        content.append(&delete);
        pop.set_child(Some(&content));
        pop
    }

    fn toggle(self: &Rc<Self>, id: u64) {
        let focus = self.selection();
        self.change(focus, |s| {
            let today = s.today.clone();
            s.board.toggle_done(id, &today);
        });
    }

    fn remove(self: &Rc<Self>, id: u64) {
        let focus = {
            let s = self.state.borrow();
            s.board.index(id).and_then(|i| {
                i.checked_sub(1)
                    .map(|j| Self::focus(s.board.tasks[j].id, i32::MAX))
            })
        };
        self.change(focus, |s| {
            if let Some(i) = s.board.index(id) {
                let end = s.board.end(i);
                s.board.tasks.drain(i..end);
            }
        });
    }

    fn global_key(self: &Rc<Self>, key: gdk::Key, m: gdk::ModifierType) -> glib::Propagation {
        if !m.contains(gdk::ModifierType::CONTROL_MASK) {
            return glib::Propagation::Proceed;
        }
        if gtk::prelude::GtkWindowExt::focus(&self.window)
            .is_some_and(|w| w.is::<gtk::Text>() || w.is::<gtk::Entry>())
        {
            return glib::Propagation::Proceed;
        }
        let shift = m.contains(gdk::ModifierType::SHIFT_MASK);
        if key == gdk::Key::z || key == gdk::Key::Z {
            self.history(shift);
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::y || key == gdk::Key::Y {
            self.history(true);
            return glib::Propagation::Stop;
        }
        if !self.notes && (key == gdk::Key::n || key == gdk::Key::N) {
            self.add_task(None);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    }

    fn task_key(
        self: &Rc<Self>,
        id: u64,
        key: gdk::Key,
        m: gdk::ModifierType,
    ) -> glib::Propagation {
        let ctrl = m.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = m.contains(gdk::ModifierType::SHIFT_MASK);
        if ctrl && (key == gdk::Key::t || key == gdk::Key::T) {
            self.add_task(Some(id));
        } else if ctrl && key == gdk::Key::period {
            self.fold(id);
        } else if ctrl && (key == gdk::Key::d || key == gdk::Key::D) {
            if let Some(view) = self.rows.borrow().get(&id) {
                let mut next = view.next_sibling();
                while let Some(widget) = next {
                    if let Ok(menu) = widget.clone().downcast::<gtk::MenuButton>() {
                        menu.popup();
                        break;
                    }
                    next = widget.next_sibling();
                }
            }
        } else if ctrl && shift && key == gdk::Key::BackSpace {
            self.remove(id);
        } else if ctrl && (key == gdk::Key::Return || key == gdk::Key::KP_Enter) {
            self.toggle(id);
        } else if key == gdk::Key::Return && !shift {
            let id_new = self.state.borrow().next_id;
            let selection = self.selection();
            self.change(Some(Self::focus(id_new, 0)), |s| {
                let Some(i) = s.board.index(id) else {
                    return;
                };
                let task = &mut s.board.tasks[i];
                let pos = selection.as_ref().map_or(task.text.chars().count(), |f| {
                    f.caret.min(f.anchor).max(0) as usize
                });
                let end = selection
                    .as_ref()
                    .map_or(pos, |f| f.caret.max(f.anchor).max(0) as usize);
                let tail: String = task.text.chars().skip(end).collect();
                task.text = task.text.chars().take(pos).collect();
                let mut new = Task::blank(id_new, task.depth, task.color.clone());
                new.text = tail;
                s.next_id += 1;
                let dest = s.board.end(i);
                s.board.tasks.insert(dest, new);
            });
        } else if key == gdk::Key::Tab || key == gdk::Key::ISO_Left_Tab {
            let focus = self.selection();
            self.change(focus, |s| {
                s.board.indent(id, shift || key == gdk::Key::ISO_Left_Tab);
            });
        } else if key == gdk::Key::BackSpace
            && self.state.borrow().board.index(id).is_some_and(|i| {
                self.state.borrow().board.tasks[i].text.is_empty()
                    && self.state.borrow().board.end(i) == i + 1
            })
        {
            self.remove(id);
        } else if m.contains(gdk::ModifierType::ALT_MASK)
            && (key == gdk::Key::Up || key == gdk::Key::Down)
        {
            let target = {
                let s = self.state.borrow();
                s.board.index(id).and_then(|i| {
                    let siblings = s.board.siblings(i);
                    let j = siblings.iter().position(|&v| v == i)?;
                    let next = if key == gdk::Key::Up {
                        j.checked_sub(1)?
                    } else {
                        j + 1
                    };
                    siblings.get(next).map(|&k| s.board.tasks[k].id)
                })
            };
            if let Some(target) = target {
                let focus = self.selection();
                self.change(focus, |s| {
                    s.board.move_branch(id, target, key == gdk::Key::Down);
                });
            }
        } else {
            return glib::Propagation::Proceed;
        }
        glib::Propagation::Stop
    }

    fn show_notice(&self) {
        let s = self.state.borrow();
        self.notice
            .set_visible(s.conflict || !s.readable || s.save_error);
        self.use_disk.set_visible(s.conflict);
        self.use_mine.set_visible(s.conflict);
        self.retry.set_visible(!s.readable || s.save_error);
        self.message.set_text(if s.conflict { "This file changed elsewhere. Choose which version to keep." } else { "Could not access the file. Check its folder permissions, then Retry. Your edits are still here." });
    }

    fn flush(&self) -> bool {
        let text = self.text();
        let mut s = self.state.borrow_mut();
        if !s.readable || s.conflict {
            return false;
        }
        if text == s.clean {
            return true;
        }
        match storage::save(&self.file, &text, &s.disk) {
            Ok(true) => {
                s.disk = Some(text.clone());
                s.clean = text;
                s.save_error = false;
            }
            Ok(false) => {
                s.conflict = true;
            }
            Err(e) => {
                if !s.save_error {
                    eprintln!("Save {}: {e}", self.file.display());
                }
                s.save_error = true;
            }
        }
        let saved = !s.conflict && !s.save_error;
        drop(s);
        self.show_notice();
        saved
    }

    fn load_latest(self: &Rc<Self>, explicit: bool) {
        let disk = match storage::read(&self.file) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Read {}: {e}", self.file.display());
                self.state.borrow_mut().readable = false;
                self.show_notice();
                return;
            }
        };
        let text = self.text();
        let mut s = self.state.borrow_mut();
        if !explicit && text != s.clean && disk != s.disk {
            s.conflict = true;
            drop(s);
            self.show_notice();
            return;
        }
        s.readable = true;
        s.save_error = false;
        s.conflict = false;
        if !explicit && disk == s.disk {
            drop(s);
            self.note.set_sensitive(true);
            self.list.set_sensitive(true);
            self.show_notice();
            return;
        }
        let new = disk.clone().unwrap_or_default();
        s.disk = disk;
        s.clean = new.clone();
        s.typing = None;
        if self.notes {
            drop(s);
            self.note.buffer().begin_user_action();
            let buffer = self.note.buffer();
            buffer.delete(&mut buffer.start_iter(), &mut buffer.end_iter());
            buffer.insert(&mut buffer.start_iter(), &new);
            buffer.end_user_action();
        } else {
            if explicit {
                let before = Snapshot {
                    board: s.board.clone(),
                    selection: None,
                };
                s.undo.push(before);
                s.redo.clear();
            } else {
                s.undo.clear();
                s.redo.clear();
            }
            s.board = Board::parse(&new);
            let day = s.today.clone();
            s.board.rollover(&day);
            s.board.completed_last();
            let max = s.board.tasks.iter().map(|t| t.id).max().unwrap_or(0);
            s.next_id = s.next_id.max(max + 1);
            drop(s);
            self.render(None);
        }
        self.note.set_sensitive(true);
        self.list.set_sensitive(true);
        self.show_notice();
    }

    fn tick(self: &Rc<Self>) {
        let day = today();
        if !self.notes && day != self.state.borrow().today {
            let focus = self.selection();
            self.change(focus, |s| {
                s.today = day.clone();
                s.board.rollover(&day);
            });
        }
        let disk = match storage::read(&self.file) {
            Ok(d) => d,
            Err(e) => {
                let mut s = self.state.borrow_mut();
                if s.readable {
                    eprintln!("Read {}: {e}", self.file.display());
                }
                s.readable = false;
                drop(s);
                self.show_notice();
                return;
            }
        };
        if disk != self.state.borrow().disk {
            let text = self.text();
            let mut s = self.state.borrow_mut();
            if disk.as_deref() == Some(&text) {
                s.disk = disk;
                s.clean = text;
                s.conflict = false;
                drop(s);
                self.show_notice();
            } else if text != s.clean
                || self.note.has_focus()
                || self.rows.borrow().values().any(|v| v.has_focus())
            {
                s.conflict = true;
                drop(s);
                self.show_notice();
            } else {
                drop(s);
                self.load_latest(false);
            }
        }
        if self.state.borrow().dirty_at.elapsed() >= Duration::from_millis(250) {
            self.flush();
        }
    }

    fn xid(&self) -> Option<u32> {
        use glib::translate::ToGlibPtr;
        let surface = self.window.surface()?;
        if !surface.type_().name().contains("X11") {
            return None;
        }
        unsafe extern "C" {
            fn gdk_x11_surface_get_xid(surface: *mut gdk::ffi::GdkSurface) -> std::ffi::c_ulong;
        }
        // Only call the X11 API on a live GdkX11Surface, owned by this window.
        Some(unsafe { gdk_x11_surface_get_xid(surface.to_glib_none().0) } as u32)
    }

    fn set_top(&self, on: bool) {
        let Some(xid) = self.xid() else {
            return;
        };
        let Ok((conn, screen)) = x11rb::connect(None) else {
            return;
        };
        let atom = |name: &[u8]| {
            conn.intern_atom(false, name)
                .ok()?
                .reply()
                .ok()
                .map(|r| r.atom)
        };
        let (Some(state), Some(above)) = (atom(b"_NET_WM_STATE"), atom(b"_NET_WM_STATE_ABOVE"))
        else {
            return;
        };
        let event = ClientMessageEvent::new(32, xid, state, [u32::from(on), above, 0, 1, 0]);
        let _ = conn.send_event(
            false,
            conn.setup().roots[screen].root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
        let _ = conn.flush();
    }

    fn restore_window(&self) {
        self.set_top(self.state.borrow().settings.pinned.unwrap_or(true));
        let Some(xid) = self.xid() else {
            return;
        };
        let Ok((conn, screen)) = x11rb::connect(None) else {
            return;
        };
        let s = self.state.borrow();
        let root = &conn.setup().roots[screen];
        let width = self
            .window
            .default_width()
            .min(i32::from(root.width_in_pixels));
        let height = self
            .window
            .default_height()
            .min(i32::from(root.height_in_pixels));
        let x = s
            .settings
            .x
            .unwrap_or(i32::from(root.width_in_pixels) - width - 20)
            .clamp(0, (i32::from(root.width_in_pixels) - width).max(0));
        let y = s
            .settings
            .y
            .unwrap_or(60)
            .clamp(0, (i32::from(root.height_in_pixels) - height).max(0));
        let _ = conn.configure_window(
            xid,
            &ConfigureWindowAux::new()
                .x(x)
                .y(y)
                .width(width as u32)
                .height(height as u32),
        );
        let _ = conn.flush();
    }

    fn save_settings(&self) {
        let mut s = self.state.borrow_mut();
        if !self.window.is_maximized() && self.window.width() > 0 {
            s.settings.width = self.window.width();
            s.settings.height = self.window.height();
        }
        if let Some(xid) = self.xid() {
            if let Ok((conn, screen)) = x11rb::connect(None) {
                if !self.window.is_maximized() {
                    if let Ok(cookie) = conn.get_geometry(xid) {
                        if let Ok(geometry) = cookie.reply() {
                            s.settings.width = i32::from(geometry.width);
                            s.settings.height = i32::from(geometry.height);
                        }
                    }
                }
                if let Ok(cookie) =
                    conn.translate_coordinates(xid, conn.setup().roots[screen].root, 0, 0)
                {
                    if let Ok(coords) = cookie.reply() {
                        s.settings.x = Some(i32::from(coords.dst_x));
                        s.settings.y = Some(i32::from(coords.dst_y));
                    }
                }
            }
        }
        match serde_json::to_string_pretty(&s.settings) {
            Ok(text) => match storage::read(&self.config)
                .and_then(|old| storage::save(&self.config, &text, &old))
            {
                Ok(true) => (),
                Ok(false) => eprintln!("Window settings changed elsewhere"),
                Err(e) => eprintln!("Window settings: {e}"),
            },
            Err(e) => eprintln!("Window settings: {e}"),
        }
    }
}

fn display_date(s: &str) -> String {
    let (day, time) = s.split_once(' ').unwrap_or((s, ""));
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|d| {
            format!(
                "{}{}{}",
                d.format("%a %-d %b"),
                if time.is_empty() { "" } else { ", " },
                time
            )
        })
        .unwrap_or_else(|_| s.to_owned())
}

fn apply_appearance(settings: &gio::Settings) {
    if let Some(gtk) = gtk::Settings::default() {
        gtk.set_gtk_application_prefer_dark_theme(settings.string("color-scheme") == "prefer-dark");
    }
}

fn pin_image(on: bool) -> gtk::Image {
    let dark = gtk::Settings::default().is_some_and(|s| s.is_gtk_application_prefer_dark_theme());
    let ink = if dark { "#f0ede5" } else { "#1d1f23" };
    // Lucide Pin (ISC); the same outline and filled state as the original header.
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="{}" stroke="{ink}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/></svg>"##,
        if on { ink } else { "none" }
    );
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_owned(svg.into_bytes()));
    match gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE) {
        Ok(pixbuf) => gtk::Image::from_pixbuf(Some(&pixbuf)),
        Err(_) => gtk::Image::from_icon_name("view-pin-symbolic"),
    }
}
