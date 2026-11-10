//! Shared native Pin and Notes window, persistence and history.
mod pin;
mod reveal;
use crate::{paths, storage};
use iced::keyboard::key::{Code, Physical};
use iced::widget::{
    button, column, container, mouse_area, operation, row, space, stack, svg, text, text_editor,
    tooltip,
};
use iced::{
    event, keyboard, mouse, window, Background, Border, Color, Element, Event, Fill, Font, Length,
    Padding, Point, Size, Subscription, Task, Theme,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation;

const EDITOR: &str = "notes";
const BG: Color = Color::from_rgb8(252, 252, 252);
const FG: Color = Color::from_rgb8(32, 32, 35);
const MUTED: Color = Color::from_rgb8(116, 116, 124);
const LINE: Color = Color::from_rgb8(231, 231, 233);

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Bounds {
    width: f32,
    height: f32,
    x: Option<f32>,
    y: Option<f32>,
    pinned: Option<bool>,
    folded: Vec<String>,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Board(pin::Message),
    Undo,
    Redo,
    Save,
    Tick,
    Pin,
    Minimize,
    Close,
    Window(window::Id, window::Event),
    Placement(window::Id, Point, bool, f32, Option<Size>),
    Drag,
    Resize(window::Direction),
    Pointer(Point),
    Context(bool),
    Copy(bool),
    Paste,
    Pasted(Option<String>),
    SelectAll,
    UseDisk,
    KeepMine,
    FocusTab(bool),
    Activate,
    MenuMove(i8),
    CalendarMove(i32),
    FramePresented,
    DeleteWord(bool),
}

struct Snapshot {
    text: String,
    cursor: text_editor::Cursor,
    scroll: i32,
    board: Option<pin::Snapshot>,
}
#[derive(Clone, Copy, PartialEq)]
enum Group {
    Insert,
    Backspace,
    Delete,
    Board(u64, u8),
}

struct Notes {
    content: text_editor::Content,
    board: Option<pin::State>,
    file: PathBuf,
    settings_file: PathBuf,
    bounds: Bounds,
    disk: Option<String>,
    dirty: bool,
    save_at: Option<Instant>,
    saved_at: Option<Instant>,
    conflict: bool,
    error: Option<(String, Instant)>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    group: Option<(Group, Instant)>,
    scroll: i32,
    window: Option<window::Id>,
    focus: u8,
    menu_focus: usize,
    pointer: Point,
    context: Option<Point>,
    initial_position: Option<(Point, bool)>,
}

pub fn run(priorities: bool) -> iced::Result {
    let settings_file = paths::config_root().join(if priorities {
        "Pin/iced.json"
    } else {
        "StickyNotes/iced.json"
    });
    let existing = std::fs::read_to_string(&settings_file).ok();
    let legacy = existing.is_none();
    let bounds: Bounds = existing
        .or_else(|| std::fs::read_to_string(settings_file.with_file_name("native.json")).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let file = paths::data_file(!priorities);
    let disk = storage::read(&file).unwrap_or_else(|error| {
        eprintln!("Notes: {error}");
        None
    });
    let width = if bounds.width >= 280. {
        bounds.width
    } else {
        if priorities {
            460.
        } else {
            430.
        }
    };
    let height = if bounds.height >= 180. {
        bounds.height
    } else {
        if priorities {
            720.
        } else {
            320.
        }
    };
    let image = image::load_from_memory(if priorities {
        include_bytes!("../../logo/pin-256.png").as_slice()
    } else {
        include_bytes!("../../logo/notes-256.png").as_slice()
    })
    .expect("Notes icon")
    .into_rgba8();
    let icon = window::icon::from_rgba(image.as_raw().clone(), image.width(), image.height()).ok();
    let initial_bounds = bounds.clone();
    iced::application(
        move || {
            let mut notes = Notes::new(
                file.clone(),
                settings_file.clone(),
                initial_bounds.clone(),
                disk.clone(),
            );
            if priorities {
                notes.board = Some(pin::State::new(
                    disk.as_deref().unwrap_or_default(),
                    &initial_bounds.folded,
                ));
                notes.content =
                    text_editor::Content::with_text(&notes.board.as_ref().unwrap().markdown());
                if disk.as_ref().is_some_and(|s| *s != notes.content.text()) {
                    notes.changed();
                }
            }
            notes.initial_position = initial_bounds
                .x
                .zip(initial_bounds.y)
                .map(|(x, y)| (Point::new(x, y), legacy))
                .or_else(|| priorities.then_some((Point::new(f32::MAX, 30.), false)));
            let focus = notes.focus_editor();
            (notes, focus)
        },
        Notes::update,
        Notes::view,
    )
    .title(if priorities { "Pin" } else { "Sticky Notes" })
    .window(window::Settings {
        size: Size::new(width, height),
        min_size: Some(Size::new(280., 180.)),
        // Resolve stored physical/logical coordinates against the real display after opening.
        position: window::Position::Centered,
        decorations: false,
        transparent: true,
        level: if bounds.pinned.unwrap_or(true) {
            window::Level::AlwaysOnTop
        } else {
            window::Level::Normal
        },
        icon,
        exit_on_close_request: false,
        platform_specific: window::settings::PlatformSpecific {
            application_id: if priorities { "pin" } else { "sticky-notes" }.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .theme(|_: &Notes| None::<Theme>)
    .subscription(Notes::subscription)
    .default_font(Font::with_name("Inter"))
    .run()
}

fn shortcut(physical: Physical, modifiers: keyboard::Modifiers) -> Option<Message> {
    if !modifiers.command() {
        return None;
    }
    match physical {
        Physical::Code(Code::KeyZ) => Some(if modifiers.shift() {
            Message::Redo
        } else {
            Message::Undo
        }),
        Physical::Code(Code::KeyY) => Some(Message::Redo),
        Physical::Code(Code::KeyS) => Some(Message::Save),
        Physical::Code(Code::KeyN) => Some(Message::Board(pin::Message::New(false))),
        _ => None,
    }
}

impl Notes {
    fn new(file: PathBuf, settings_file: PathBuf, bounds: Bounds, disk: Option<String>) -> Self {
        let content = text_editor::Content::with_text(disk.as_deref().unwrap_or_default());
        let mut initial = Notes {
            content,
            board: None,
            file,
            settings_file,
            bounds: bounds.clone(),
            disk,
            dirty: false,
            save_at: None,
            saved_at: None,
            conflict: false,
            error: None,
            undo: vec![],
            redo: vec![],
            group: None,
            scroll: 0,
            window: None,
            focus: 0,
            menu_focus: 0,
            pointer: Point::ORIGIN,
            context: None,
            initial_position: None,
        };
        initial
            .content
            .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        initial
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(
                if self.save_at.is_some() || self.saved_at.is_some() || self.error.is_some() {
                    Duration::from_millis(250)
                } else {
                    Duration::from_secs(2)
                },
            )
            .map(|_| Message::Tick),
            window::frames()
                .filter_map(|_| crate::x11::pending().then_some(Message::FramePresented)),
            event::listen_with(|event, status, id| match event {
                Event::Window(
                    e @ (window::Event::Opened { .. }
                    | window::Event::Moved(_)
                    | window::Event::Resized(_)
                    | window::Event::CloseRequested),
                ) => Some(Message::Window(id, e)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    Some(Message::Board(pin::Message::Drop))
                }
                Event::Mouse(mouse::Event::CursorLeft) => {
                    Some(Message::Board(pin::Message::CancelDrag))
                }
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    Some(Message::Pointer(position))
                }
                Event::Keyboard(keyboard::Event::KeyPressed {
                    physical_key,
                    modifiers,
                    key,
                    ..
                }) if status == event::Status::Ignored => {
                    shortcut(physical_key, modifiers).or_else(|| navigation(&key, modifiers))
                }
                _ => None,
            }),
        ])
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.content.text(),
            cursor: self.content.cursor(),
            scroll: self.scroll,
            board: self.board.as_ref().map(pin::State::snapshot),
        }
    }
    fn changed(&mut self) {
        self.dirty = true;
        self.save_at = Some(Instant::now() + Duration::from_millis(250));
        self.saved_at = None;
    }
    fn edit(&mut self, action: text_editor::Action) {
        let before = action.is_edit().then(|| self.snapshot());
        let kind = match action {
            text_editor::Action::Edit(text_editor::Edit::Insert(_)) => Some(Group::Insert),
            text_editor::Action::Edit(text_editor::Edit::Backspace) => Some(Group::Backspace),
            text_editor::Action::Edit(text_editor::Edit::Delete) => Some(Group::Delete),
            _ => None,
        };
        if let text_editor::Action::Scroll { lines } = action {
            self.scroll = (self.scroll + lines).max(0);
        }
        self.content.perform(action);
        if let Some(before) = before {
            if before.text != self.content.text() {
                self.record(before, kind);
            }
        } else {
            self.group = None;
        }
    }
    fn record(&mut self, before: Snapshot, kind: Option<Group>) {
        let now = Instant::now();
        if kind.is_none()
            || !self.group.is_some_and(|(k, at)| {
                Some(k) == kind && now.duration_since(at) < Duration::from_millis(600)
            })
        {
            self.undo.push(before);
            if self.undo.len() > 300 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.group = kind.map(|k| (k, now));
        self.changed();
    }
    fn focus_editor(&self) -> Task<Message> {
        self.board.as_ref().map_or_else(
            || operation::focus(EDITOR),
            |b| b.focus().map(Message::Board),
        )
    }
    fn selection(&self) -> Option<String> {
        self.board
            .as_ref()
            .map_or_else(|| self.content.selection(), pin::State::selection)
    }
    fn load_board(&mut self) {
        if let Some(board) = &self.board {
            let next = pin::State::new(&self.content.text(), &board.folded_keys());
            let value = next.markdown();
            self.board = Some(next);
            if value != self.content.text() {
                self.content = text_editor::Content::with_text(&value);
                self.changed();
            }
        }
    }
    fn history(&mut self, redo: bool) -> Task<Message> {
        let next = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        if let Some(next) = next {
            let current = self.snapshot();
            if redo {
                self.undo.push(current);
            } else {
                self.redo.push(current);
            }
            let task = match (&mut self.board, next.board) {
                (Some(board), Some(snapshot)) => board.restore(snapshot).map(Message::Board),
                _ => Task::none(),
            };
            let value = self.board.as_ref().map_or(next.text, pin::State::markdown);
            self.content = text_editor::Content::with_text(&value);
            self.content.move_to(next.cursor);
            self.scroll = next.scroll;
            self.content
                .perform(text_editor::Action::Scroll { lines: self.scroll });
            self.group = None;
            self.changed();
            return task;
        }
        Task::none()
    }
    fn save(&mut self, force: bool) -> bool {
        if self.conflict {
            return false;
        }
        if !self.dirty && !force {
            return true;
        }
        let value = self.content.text();
        match storage::save(&self.file, &value, &self.disk) {
            Ok(true) => {
                self.disk = Some(value);
                self.dirty = false;
                self.save_at = None;
                self.saved_at = Some(Instant::now());
                self.error = None;
                true
            }
            Ok(false) => {
                self.conflict = true;
                self.save_at = None;
                false
            }
            Err(error) => {
                eprintln!("Notes save: {error}");
                self.error = Some((
                    "Couldn’t save. Check the folder’s permissions and try Save again.".into(),
                    Instant::now(),
                ));
                self.save_at = None;
                false
            }
        }
    }
    fn preferences(&self) {
        let save = || -> std::io::Result<()> {
            let mut bounds = self.bounds.clone();
            if let Some(board) = &self.board {
                bounds.folded = board.folded_keys();
            }
            let text = serde_json::to_string_pretty(&bounds)?;
            let disk = storage::read(&self.settings_file)?;
            storage::save(&self.settings_file, &text, &disk)?;
            Ok(())
        };
        if let Err(error) = save() {
            eprintln!("Notes window: {error}");
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        let message = match (&self.board, message) {
            (Some(_), Message::Copy(cut)) => Message::Board(pin::Message::Copy(cut)),
            (Some(_), Message::Paste) => Message::Board(pin::Message::Paste),
            (Some(_), Message::SelectAll) => Message::Board(pin::Message::SelectAll),
            (_, message) => message,
        };
        match message {
            Message::Board(action) => {
                if self.board.is_none() {
                    return Task::none();
                }
                match action {
                    pin::Message::Undo => return self.update(Message::Undo),
                    pin::Message::Redo => return self.update(Message::Redo),
                    pin::Message::Save => return self.update(Message::Save),
                    pin::Message::Header(reverse) => {
                        self.focus = if reverse { 6 } else { 9 };
                        return Task::none();
                    }
                    pin::Message::Context(target) => {
                        self.board.as_mut().unwrap().set_active(target);
                        return self.update(Message::Context(true));
                    }
                    _ => (),
                }
                if matches!(
                    action,
                    pin::Message::Copy(_)
                        | pin::Message::Paste
                        | pin::Message::Pasted(_)
                        | pin::Message::SelectAll
                ) {
                    self.context = None;
                }
                let before = self.snapshot();
                let kind = pin::State::group(&action);
                let board = self.board.as_mut().unwrap();
                let revision = board.revision;
                let opening = matches!(action, pin::Message::Options(_));
                let task = board.update(action).map(Message::Board);
                if opening {
                    board.anchor_options(self.pointer);
                }
                if board.revision != revision {
                    self.content = text_editor::Content::with_text(&board.markdown());
                    self.record(before, kind);
                } else {
                    self.group = None;
                }
                self.focus = 0;
                return task;
            }
            Message::Edit(action) => {
                self.focus = 0;
                self.context = None;
                self.edit(action);
            }
            Message::Undo => {
                self.focus = 0;
                self.context = None;
                return Task::batch([self.history(false), self.focus_editor()]);
            }
            Message::Redo => {
                self.focus = 0;
                self.context = None;
                return Task::batch([self.history(true), self.focus_editor()]);
            }
            Message::Save => {
                self.save(true);
                if self.focus == 0 {
                    return self.focus_editor();
                }
            }
            Message::Tick => {
                if let Some(board) = &mut self.board {
                    let revision = board.revision;
                    let _ = board.update(pin::Message::Tick);
                    if board.revision != revision {
                        self.content = text_editor::Content::with_text(&board.markdown());
                        self.changed();
                    }
                }
                let now = Instant::now();
                if self.save_at.is_some_and(|at| now >= at) {
                    self.save(false);
                }
                if self
                    .saved_at
                    .is_some_and(|at| now.duration_since(at) > Duration::from_millis(1400))
                {
                    self.saved_at = None;
                }
                if self
                    .error
                    .as_ref()
                    .is_some_and(|(_, at)| now.duration_since(*at) > Duration::from_secs(6))
                {
                    self.error = None;
                }
                if !self.conflict {
                    if let Ok(disk) = storage::read(&self.file) {
                        if disk != self.disk {
                            if self.dirty {
                                self.conflict = true;
                            } else {
                                self.disk = disk;
                                self.content = text_editor::Content::with_text(
                                    self.disk.as_deref().unwrap_or_default(),
                                );
                                self.load_board();
                                self.undo.clear();
                                self.redo.clear();
                                self.group = None;
                                self.scroll = 0;
                            }
                        }
                    }
                }
            }
            Message::Pin => {
                self.bounds.pinned = Some(!self.bounds.pinned.unwrap_or(true));
                self.preferences();
                if let Some(id) = self.window {
                    return window::set_level(
                        id,
                        if self.bounds.pinned.unwrap_or(true) {
                            window::Level::AlwaysOnTop
                        } else {
                            window::Level::Normal
                        },
                    );
                }
            }
            Message::Minimize => {
                if let Some(id) = self.window {
                    return window::minimize(id, true);
                }
            }
            Message::Close => {
                if self.save(false) {
                    self.preferences();
                    return iced::exit();
                }
            }
            Message::Window(id, event) => {
                self.window = Some(id);
                match event {
                    window::Event::Opened { position, size } => {
                        self.bounds.width = size.width;
                        self.bounds.height = size.height;
                        if let Some(p) = position {
                            self.bounds.x = Some(p.x);
                            self.bounds.y = Some(p.y);
                        }
                        // Iced creates X11 windows hidden; apply this after mapping.
                        let level = window::set_level(
                            id,
                            if self.bounds.pinned.unwrap_or(true) {
                                window::Level::AlwaysOnTop
                            } else {
                                window::Level::Normal
                            },
                        );
                        let placement = if let Some((point, legacy)) = self.initial_position.take()
                        {
                            window::scale_factor(id).then(move |scale| {
                                window::monitor_size(id).map(move |monitor| {
                                    Message::Placement(id, point, legacy, scale, monitor)
                                })
                            })
                        } else {
                            Task::none()
                        };
                        let surface = window::run(id, |handle| {
                            #[cfg(target_os = "linux")]
                            if let (Ok(window), Ok(display)) =
                                (handle.window_handle(), handle.display_handle())
                            {
                                use window::raw_window_handle::{
                                    RawDisplayHandle, RawWindowHandle,
                                };
                                if let (
                                    RawWindowHandle::Xlib(window),
                                    RawDisplayHandle::Xlib(display),
                                ) = (window.as_raw(), display.as_raw())
                                {
                                    if let Some(display) = display.display {
                                        // Iced runs this on the window's event-loop thread.
                                        unsafe {
                                            crate::x11::retain_contents(
                                                display.as_ptr(),
                                                window.window,
                                            );
                                        }
                                    }
                                }
                            }
                        })
                        .discard();
                        return Task::batch([level, placement, surface]);
                    }
                    window::Event::Moved(p) => {
                        self.bounds.x = Some(p.x);
                        self.bounds.y = Some(p.y);
                    }
                    window::Event::Resized(size) => {
                        self.bounds.width = size.width;
                        self.bounds.height = size.height;
                    }
                    window::Event::CloseRequested => return self.update(Message::Close),
                    _ => (),
                }
            }
            Message::Placement(id, point, legacy, scale, monitor) => {
                let old = Size::new(self.bounds.width, self.bounds.height);
                if let Some(size) = monitor {
                    self.bounds.width = self.bounds.width.min(size.width);
                    self.bounds.height = self.bounds.height.min(size.height);
                }
                let position = restored_position(point, legacy, scale, monitor, &self.bounds);
                self.bounds.x = Some(position.x);
                self.bounds.y = Some(position.y);
                self.preferences();
                let size = Size::new(self.bounds.width, self.bounds.height);
                return Task::batch([
                    window::move_to(id, position),
                    if old != size {
                        window::resize(id, size)
                    } else {
                        Task::none()
                    },
                ]);
            }
            Message::FramePresented => {
                if let Some(id) = self.window {
                    return window::scale_factor(id).then(move |scale| {
                        window::run(id, move |_| crate::x11::frame_presented(scale)).discard()
                    });
                }
            }
            Message::Drag => {
                if let Some(id) = self.window {
                    return window::drag(id);
                }
            }
            Message::Resize(direction) => {
                if let Some(id) = self.window {
                    return window::drag_resize(id, direction);
                }
            }
            Message::Pointer(p) => self.pointer = p,
            Message::Context(open) => {
                let was_open = self.context.is_some();
                self.context = if open { Some(self.pointer) } else { None };
                self.menu_focus = (0..6).find(|&i| self.menu_action(i).is_some()).unwrap_or(4);
                if open && self.board.is_some() {
                    return operation::focus("pin-context");
                }
                if !open {
                    if !was_open && self.board.as_ref().is_some_and(pin::State::has_options) {
                        return self.update(Message::Board(pin::Message::CloseOptions));
                    }
                    self.focus = 0;
                    return self.focus_editor();
                }
            }
            Message::FocusTab(reverse) => {
                if self.context.is_some() {
                    self.move_menu(if reverse { -1 } else { 1 });
                } else {
                    if self.focus == 0 && self.board.as_ref().is_some_and(pin::State::has_options) {
                        return self.update(Message::Board(pin::Message::OptionsTab(reverse)));
                    }
                    let mut order = vec![0];
                    if self.board.is_some() {
                        order.push(9);
                    }
                    order.extend([1, 2, 3, 4, 5, 6]);
                    if self.conflict {
                        order.extend([7, 8]);
                    }
                    let at = order.iter().position(|i| *i == self.focus).unwrap_or(0);
                    for step in 1..=order.len() {
                        let next = if reverse {
                            (at + order.len() - step) % order.len()
                        } else {
                            (at + step) % order.len()
                        };
                        let index = order[next];
                        if index == 0 || self.header_action(index).is_some() {
                            self.focus = index;
                            break;
                        }
                    }
                }
                if self.context.is_none() && self.focus == 0 {
                    return self.focus_editor();
                }
            }
            Message::MenuMove(direction) => {
                if self.context.is_some() {
                    self.move_menu(direction);
                } else if self.board.as_ref().is_some_and(pin::State::calendar_open) {
                    return self.update(Message::Board(pin::Message::CalendarMove(
                        direction as i32 * 7,
                    )));
                } else if self.board.as_ref().is_some_and(pin::State::has_options) {
                    return self.update(Message::Board(pin::Message::OptionsTab(direction < 0)));
                }
            }
            Message::CalendarMove(step) => {
                if self.context.is_none()
                    && self.board.as_ref().is_some_and(pin::State::calendar_open)
                {
                    return self.update(Message::Board(pin::Message::CalendarMove(step)));
                }
            }
            Message::Activate => {
                if self.focus == 0
                    && self.context.is_none()
                    && self.board.as_ref().is_some_and(pin::State::has_options)
                {
                    return self.update(Message::Board(pin::Message::Activate));
                }
                let action = if self.context.is_some() {
                    self.menu_action(self.menu_focus)
                } else {
                    self.header_action(self.focus)
                };
                if let Some(action) = action {
                    return self.update(action);
                }
            }
            Message::DeleteWord(forward) => {
                let before = self.snapshot();
                delete_word(&mut self.content, forward);
                if self.content.text() != before.text {
                    self.record(before, None);
                }
            }

            Message::Copy(cut) => {
                self.context = None;
                if let Some(selection) = self.content.selection() {
                    if cut {
                        self.edit(text_editor::Action::Edit(text_editor::Edit::Backspace));
                    }
                    return Task::batch([
                        iced::clipboard::write(selection),
                        operation::focus(EDITOR),
                    ]);
                }
            }
            Message::Paste => {
                self.context = None;
                return iced::clipboard::read().map(Message::Pasted);
            }
            Message::Pasted(Some(s)) => {
                self.edit(text_editor::Action::Edit(text_editor::Edit::Paste(
                    Arc::new(s),
                )));
                self.focus = 0;
                return self.focus_editor();
            }
            Message::Pasted(None) => (),
            Message::SelectAll => {
                self.context = None;
                self.edit(text_editor::Action::SelectAll);
                return self.focus_editor();
            }
            Message::UseDisk => {
                if let Ok(disk) = storage::read(&self.file) {
                    self.undo.push(self.snapshot());
                    self.redo.clear();
                    self.disk = disk;
                    self.content =
                        text_editor::Content::with_text(self.disk.as_deref().unwrap_or_default());
                    self.dirty = false;
                    self.conflict = false;
                    self.save_at = None;
                    self.load_board();
                    self.group = None;
                    self.focus = 0;
                    return self.focus_editor();
                }
            }
            Message::KeepMine => {
                if let Ok(disk) = storage::read(&self.file) {
                    self.disk = disk;
                    self.conflict = false;
                    self.save(true);
                    self.focus = 0;
                    return self.focus_editor();
                }
            }
        }
        Task::none()
    }
    fn header_action(&self, index: u8) -> Option<Message> {
        match index {
            1 => (!self.undo.is_empty()).then_some(Message::Undo),
            2 => (!self.redo.is_empty()).then_some(Message::Redo),
            3 => Some(Message::Save),
            4 => Some(Message::Pin),
            5 => Some(Message::Minimize),
            6 => Some(Message::Close),
            7 => self.conflict.then_some(Message::UseDisk),
            8 => self.conflict.then_some(Message::KeepMine),
            9 => self
                .board
                .as_ref()
                .map(|_| Message::Board(pin::Message::New(false))),
            _ => None,
        }
    }
    fn menu_action(&self, index: usize) -> Option<Message> {
        match index {
            0 => self.header_action(1),
            1 => self.header_action(2),
            2 | 3 => self.selection().map(|_| Message::Copy(index == 3)),
            4 => Some(Message::Paste),
            5 => Some(Message::SelectAll),
            _ => None,
        }
    }
    fn move_menu(&mut self, direction: i8) {
        for _ in 0..6 {
            self.menu_focus = (self.menu_focus as i8 + direction + 6) as usize % 6;
            if self.menu_action(self.menu_focus).is_some() {
                break;
            }
        }
    }
    fn view(&self) -> Element<'_, Message> {
        let mut header = row![mouse_area(
            container(
                text(if self.board.is_some() { "Pin" } else { "Notes" })
                    .size(13)
                    .font(Font {
                        weight: iced::font::Weight::Semibold,
                        ..Font::with_name("Inter")
                    })
            )
            .width(Fill)
            .height(Fill)
            .align_y(iced::Center)
        )
        .on_press(Message::Drag),];
        if self.board.is_some() {
            header = header.push(icon_button(
                "New task (Ctrl+N)",
                "plus",
                Some(Message::Board(pin::Message::New(false))),
                self.focus == 9,
            ));
        }
        header = header
            .extend([
                icon_button(
                    "Undo (Ctrl+Z)",
                    "undo",
                    (!self.undo.is_empty()).then_some(Message::Undo),
                    self.focus == 1,
                ),
                icon_button(
                    "Redo (Ctrl+Shift+Z)",
                    "redo",
                    (!self.redo.is_empty()).then_some(Message::Redo),
                    self.focus == 2,
                ),
                icon_button(
                    "Save (Ctrl+S)",
                    if self.saved_at.is_some() {
                        "check"
                    } else {
                        "save"
                    },
                    Some(Message::Save),
                    self.focus == 3,
                ),
                container(space())
                    .width(1)
                    .height(16)
                    .style(|theme| container::Style::default().background(line(theme)))
                    .into(),
                icon_button(
                    if self.bounds.pinned.unwrap_or(true) {
                        "Unpin from top"
                    } else {
                        "Keep on top"
                    },
                    if self.bounds.pinned.unwrap_or(true) {
                        "pin-filled"
                    } else {
                        "pin"
                    },
                    Some(Message::Pin),
                    self.focus == 4,
                ),
                icon_button(
                    "Minimize",
                    "minus",
                    Some(Message::Minimize),
                    self.focus == 5,
                ),
                icon_button("Close", "x", Some(Message::Close), self.focus == 6),
            ])
            .spacing(2)
            .align_y(iced::Center)
            .height(42)
            .padding([0., 11.5]);
        let editor = text_editor(&self.content)
            .id(EDITOR)
            .on_action(Message::Edit)
            .height(Fill)
            .padding([14., 17.5])
            .size(15)
            .placeholder("Write a note…")
            .line_height(iced::widget::text::LineHeight::Relative(24.5 / 15.))
            .wrapping(iced::widget::text::Wrapping::Word)
            .style(|theme, _| text_editor::Style {
                background: bg(theme).into(),
                border: Border::default(),
                placeholder: muted(theme),
                value: fg(theme),
                selection: Color::from_rgba8(161, 161, 170, 0.3),
            })
            .key_binding(|press| {
                let nav = navigation(&press.key, press.modifiers);
                if matches!(nav, Some(Message::FocusTab(_))) {
                    return Some(text_editor::Binding::Sequence(vec![
                        text_editor::Binding::Unfocus,
                        text_editor::Binding::Custom(nav.unwrap()),
                    ]));
                }
                if self.context.is_some() || matches!(nav, Some(Message::Context(true))) {
                    if let Some(nav) = nav {
                        return Some(text_editor::Binding::Custom(nav));
                    }
                }
                if press.modifiers.jump() {
                    match press.key.as_ref() {
                        keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                            return Some(text_editor::Binding::Custom(Message::DeleteWord(false)))
                        }
                        keyboard::Key::Named(keyboard::key::Named::Delete) => {
                            return Some(text_editor::Binding::Custom(Message::DeleteWord(true)))
                        }
                        _ => (),
                    }
                }
                shortcut(press.physical_key, press.modifiers)
                    .map(text_editor::Binding::Custom)
                    .or_else(|| text_editor::Binding::from_key_press(press))
            });
        let mut body = column![
            header,
            container(space())
                .height(1)
                .width(Fill)
                .style(|theme| container::Style::default().background(line(theme)))
        ];
        if self.conflict {
            body = body.push(
                container(
                    column![
                        text("This file changed elsewhere. Choose which version to keep.").size(12),
                        row![
                            button(text("Use disk").size(12))
                                .padding([5, 10])
                                .on_press(Message::UseDisk)
                                .style(|theme, status| conflict_style(
                                    theme,
                                    status,
                                    self.focus == 7,
                                    false
                                )),
                            button(text("Keep mine").size(12))
                                .padding([5, 10])
                                .on_press(Message::KeepMine)
                                .style(|theme, status| conflict_style(
                                    theme,
                                    status,
                                    self.focus == 8,
                                    true
                                ))
                        ]
                        .spacing(8)
                    ]
                    .spacing(8),
                )
                .padding(12),
            );
        } else if let Some((message, _)) = &self.error {
            body = body.push(container(text(message).size(12)).padding(12));
        }
        body = if let Some(board) = &self.board {
            body.push(
                board
                    .view(Size::new(self.bounds.width, self.bounds.height))
                    .map(Message::Board),
            )
        } else {
            body.push(mouse_area(editor).on_right_press(Message::Context(true)))
        };
        let base = container(body)
            .width(Fill)
            .height(Fill)
            .style(|theme| container::Style {
                background: Some(bg(theme).into()),
                text_color: Some(fg(theme)),
                border: Border {
                    color: line(theme),
                    width: 1.,
                    radius: 0.into(),
                },
                ..Default::default()
            });
        let mut layers = stack![crate::x11::frame(), base];
        if let Some(p) = self.context {
            let menu = column((0..6).map(|i| {
                menu_item(
                    ["Undo", "Redo", "Copy", "Cut", "Paste", "Select all"][i],
                    self.menu_action(i),
                    self.menu_focus == i,
                )
            }))
            .padding(4)
            .spacing(1);
            let overlay = container(menu).width(170).style(|theme| container::Style {
                background: Some(bg(theme).into()),
                text_color: Some(fg(theme)),
                border: Border {
                    color: line(theme),
                    width: 1.,
                    radius: 6.into(),
                },
                ..Default::default()
            });
            layers = layers.push(
                mouse_area(container(space()).width(Fill).height(Fill))
                    .on_press(Message::Context(false)),
            );
            layers = layers.push(container(overlay).padding(Padding {
                top: p.y.min((self.bounds.height - 210.).max(0.)),
                left: p.x.min((self.bounds.width - 175.).max(0.)),
                right: 0.,
                bottom: 0.,
            }));
        }
        for direction in [
            window::Direction::North,
            window::Direction::South,
            window::Direction::West,
            window::Direction::East,
            window::Direction::NorthWest,
            window::Direction::NorthEast,
            window::Direction::SouthWest,
            window::Direction::SouthEast,
        ] {
            layers = layers.push(resize_handle(direction));
        }
        layers.into()
    }
}

fn menu_item(
    label: &'static str,
    action: Option<Message>,
    focused: bool,
) -> Element<'static, Message> {
    button(text(label).size(13))
        .width(Fill)
        .padding([5, 8])
        .on_press_maybe(action)
        .style(move |theme, status| focus_style(theme, status, focused))
        .into()
}
fn icon_button(
    label: &'static str,
    name: &str,
    action: Option<Message>,
    focused: bool,
) -> Element<'static, Message> {
    let disabled = action.is_none();
    let data = icon_data(name);
    let bytes: Cow<'static, [u8]> = if name == "pin-filled" {
        Cow::Owned(
            String::from_utf8_lossy(data)
                .replace("fill=\"none\"", "fill=\"currentColor\"")
                .into_bytes(),
        )
    } else {
        Cow::Borrowed(data)
    };
    let handle = svg::Handle::from_memory(bytes);
    tooltip(
        button(
            container(
                svg(handle)
                    .width(15)
                    .height(15)
                    .style(move |theme, _| svg::Style {
                        color: Some(if disabled { muted(theme) } else { fg(theme) }),
                    }),
            )
            .center(26),
        )
        .padding(0)
        .on_press_maybe(action)
        .style(move |theme, status| focus_style(theme, status, focused)),
        container(text(label).size(12))
            .padding([4, 7])
            .style(|theme| container::Style {
                background: Some(fg(theme).into()),
                text_color: Some(bg(theme)),
                border: Border {
                    radius: 5.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        tooltip::Position::Bottom,
    )
    .delay(Duration::from_millis(300))
    .into()
}
fn ghost(theme: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed).then_some(
            Background::Color(if is_dark(theme) {
                Color::from_rgb8(43, 43, 48)
            } else {
                Color::from_rgb8(240, 240, 241)
            }),
        ),
        text_color: if disabled { muted(theme) } else { fg(theme) },
        border: Border {
            radius: 5.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn icon_data(name: &str) -> &'static [u8] {
    match name {
        "plus" => include_bytes!("../icons/plus.svg"),
        "grip" => include_bytes!("../icons/grip-vertical.svg"),
        "more" => include_bytes!("../icons/ellipsis.svg"),
        "down" => include_bytes!("../icons/chevron-down.svg"),
        "right" => include_bytes!("../icons/chevron-right.svg"),
        "undo" => include_bytes!("../icons/undo-2.svg"),
        "redo" => include_bytes!("../icons/redo-2.svg"),
        "save" => include_bytes!("../icons/save.svg"),
        "pin" | "pin-filled" => include_bytes!("../icons/pin.svg"),
        "minus" => include_bytes!("../icons/minus.svg"),
        "check" => include_bytes!("../icons/check.svg"),
        _ => include_bytes!("../icons/x.svg"),
    }
}

fn is_dark(theme: &Theme) -> bool {
    theme.palette().background.r < 0.5
}
fn bg(theme: &Theme) -> Color {
    if is_dark(theme) {
        Color::from_rgb8(25, 25, 28)
    } else {
        BG
    }
}
fn fg(theme: &Theme) -> Color {
    if is_dark(theme) {
        Color::from_rgb8(239, 239, 241)
    } else {
        FG
    }
}
fn muted(theme: &Theme) -> Color {
    if is_dark(theme) {
        Color::from_rgb8(161, 161, 170)
    } else {
        MUTED
    }
}
fn line(theme: &Theme) -> Color {
    if is_dark(theme) {
        Color::from_rgb8(51, 51, 56)
    } else {
        LINE
    }
}

fn focus_style(theme: &Theme, status: button::Status, focused: bool) -> button::Style {
    let mut style = ghost(theme, status);
    if focused {
        style.border = Border {
            color: muted(theme),
            width: 1.,
            radius: 5.into(),
        };
    }
    style
}
fn navigation(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
    use keyboard::key::Named;
    match key.as_ref() {
        keyboard::Key::Named(Named::Tab) => Some(Message::FocusTab(modifiers.shift())),
        keyboard::Key::Named(Named::Enter | Named::Space) => Some(Message::Activate),
        keyboard::Key::Character(" ") => Some(Message::Activate),
        keyboard::Key::Named(Named::Escape) => Some(Message::Context(false)),
        keyboard::Key::Named(Named::ContextMenu) => Some(Message::Context(true)),
        keyboard::Key::Named(Named::F10) if modifiers.shift() => Some(Message::Context(true)),
        keyboard::Key::Named(Named::ArrowUp) => Some(Message::MenuMove(-1)),
        keyboard::Key::Named(Named::ArrowDown) => Some(Message::MenuMove(1)),
        keyboard::Key::Named(Named::ArrowLeft) => Some(Message::CalendarMove(-1)),
        keyboard::Key::Named(Named::ArrowRight) => Some(Message::CalendarMove(1)),
        keyboard::Key::Named(Named::PageUp) => Some(Message::Board(pin::Message::Month(false))),
        keyboard::Key::Named(Named::PageDown) => Some(Message::Board(pin::Message::Month(true))),
        _ => None,
    }
}

fn resize_handle(direction: window::Direction) -> Element<'static, Message> {
    use window::Direction::*;
    let vertical = matches!(direction, East | West);
    let horizontal = matches!(direction, North | South);
    let target = mouse_area(
        container(space())
            .width(if horizontal {
                Length::Fill
            } else {
                Length::Fixed(if vertical { 8. } else { 16. })
            })
            .height(if vertical {
                Length::Fill
            } else {
                Length::Fixed(if horizontal { 8. } else { 16. })
            }),
    )
    .on_press(Message::Resize(direction))
    .interaction(if vertical {
        mouse::Interaction::ResizingHorizontally
    } else if horizontal {
        mouse::Interaction::ResizingVertically
    } else if matches!(direction, NorthEast | SouthWest) {
        mouse::Interaction::ResizingDiagonallyUp
    } else {
        mouse::Interaction::ResizingDiagonallyDown
    });
    container(target)
        .width(Fill)
        .height(Fill)
        .align_x(if matches!(direction, East | NorthEast | SouthEast) {
            iced::Right
        } else {
            iced::Left
        })
        .align_y(if matches!(direction, South | SouthEast | SouthWest) {
            iced::Bottom
        } else {
            iced::Top
        })
        .into()
}

fn conflict_style(
    theme: &Theme,
    status: button::Status,
    focused: bool,
    primary: bool,
) -> button::Style {
    let mut style = ghost(theme, status);
    if primary {
        style.background = Some(fg(theme).into());
        style.text_color = bg(theme);
    }
    style.border = Border {
        color: if focused { muted(theme) } else { line(theme) },
        width: if focused { 2. } else { 1. },
        radius: 5.into(),
    };
    style
}

fn delete_word(content: &mut text_editor::Content, forward: bool) {
    let cursor = content.cursor();
    let value = content.text();
    if content.selection().is_none() {
        let offset = content
            .lines()
            .take(cursor.position.line)
            .map(|l| l.text.len() + l.ending.as_str().len().max(1))
            .sum::<usize>()
            + cursor.position.column;
        let target = if forward {
            value[offset..]
                .unicode_word_indices()
                .next()
                .map_or(value.len(), |(i, w)| offset + i + w.len())
        } else {
            value[..offset]
                .unicode_word_indices()
                .next_back()
                .map_or(0, |(i, _)| i)
        };
        if target == offset {
            return;
        }
        let mut base = 0;
        let mut position = cursor.position;
        for (i, line) in content.lines().enumerate() {
            if target <= base + line.text.len() {
                position = text_editor::Position {
                    line: i,
                    column: target - base,
                };
                break;
            }
            base += line.text.len() + line.ending.as_str().len().max(1);
        }
        content.move_to(text_editor::Cursor {
            position: cursor.position,
            selection: Some(position),
        });
    }
    content.perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
}

fn restored_position(
    point: Point,
    legacy: bool,
    scale: f32,
    monitor: Option<Size>,
    bounds: &Bounds,
) -> Point {
    let point = if legacy {
        Point::new(point.x / scale.max(1.), point.y / scale.max(1.))
    } else {
        point
    };
    if let Some(monitor) = monitor {
        Point::new(
            point.x.clamp(0., (monitor.width - bounds.width).max(0.)),
            point.y.clamp(0., (monitor.height - bounds.height).max(0.)),
        )
    } else {
        point
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(value: &str) -> Notes {
        Notes::new(
            PathBuf::from("unused.txt"),
            PathBuf::from("unused.json"),
            Bounds::default(),
            Some(value.into()),
        )
    }

    #[test]
    fn preserves_plain_text_unicode_and_trailing_newlines() {
        for value in [
            "",
            "A thought",
            "A thought\n",
            "سلام\n\nAnother thought.\n\n",
        ] {
            assert_eq!(note(value).content.text(), value);
        }
    }

    #[test]
    fn loading_an_external_completed_daily_task_schedules_its_reopened_version_for_save() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Pin.md");
        let external = "- [x] Daily 📅 2000-01-01 09:30 🔁 daily ✅ 2000-01-01\n- [x] One-off\n";
        std::fs::write(&file, external).unwrap();
        let mut app = Notes::new(
            file.clone(),
            dir.path().join("iced.json"),
            Bounds::default(),
            Some("- [ ] Local edit\n".into()),
        );
        app.board = Some(pin::State::new("- [ ] Local edit\n", &[]));
        app.conflict = true;
        let _ = app.update(Message::UseDisk);
        assert!(
            app.dirty,
            "Rollover must reach the file as well as the task view"
        );
        assert!(app.save(false));
        let day = chrono::Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(
            std::fs::read_to_string(file).unwrap(),
            format!("- [ ] Daily 📅 {day} 09:30 🔁 daily\n- [x] One-off\n")
        );
    }

    #[test]
    fn typed_phrase_and_selected_delete_share_toolbar_history() {
        let mut n = note("سلام\nA first note.\n");
        let before = n.content.text();
        let cursor = n.content.cursor();
        for ch in "A new thought".chars() {
            n.edit(text_editor::Action::Edit(text_editor::Edit::Insert(ch)));
        }
        let typed = n.content.text();
        let _ = n.history(false);
        assert_eq!(n.content.text(), before);
        assert_eq!(n.content.cursor(), cursor);
        let _ = n.history(true);
        assert_eq!(n.content.text(), typed);
        n.edit(text_editor::Action::SelectAll);
        n.edit(text_editor::Action::Edit(text_editor::Edit::Backspace));
        assert!(n.content.text().is_empty());
        let _ = n.history(false);
        assert_eq!(n.content.text(), typed);
    }

    #[test]
    fn word_delete_restores_original_caret_and_text_on_undo() {
        let mut n = note("سلام world");
        let cursor = n.content.cursor();
        let _ = n.update(Message::DeleteWord(false));
        assert_eq!(n.content.text(), "سلام ");
        let _ = n.history(false);
        assert_eq!(n.content.text(), "سلام world");
        assert_eq!(n.content.cursor(), cursor);
    }

    #[test]
    fn tab_reaches_actions_and_skips_disabled_history_buttons() {
        let mut n = note("Hello");
        for expected in [3, 4, 5, 6, 0] {
            let _ = n.update(Message::FocusTab(false));
            assert_eq!(n.focus, expected);
        }
        let _ = n.update(Message::FocusTab(true));
        assert_eq!(n.focus, 6);
        n.conflict = true;
        for expected in [7, 8, 0] {
            let _ = n.update(Message::FocusTab(false));
            assert_eq!(n.focus, expected);
        }
    }

    #[test]
    fn legacy_physical_position_migrates_to_visible_logical_bounds() {
        let bounds = Bounds {
            width: 430.,
            height: 320.,
            ..Bounds::default()
        };
        assert_eq!(
            restored_position(
                Point::new(1920., 248.),
                true,
                2.,
                Some(Size::new(2048., 1152.)),
                &bounds
            ),
            Point::new(960., 124.)
        );
        assert_eq!(
            restored_position(
                Point::new(9999., -100.),
                false,
                1.,
                Some(Size::new(800., 600.)),
                &bounds
            ),
            Point::new(370., 0.)
        );
    }

    #[test]
    fn shortcut_actions_use_physical_keys() {
        assert!(matches!(
            shortcut(Physical::Code(Code::KeyZ), keyboard::Modifiers::CTRL),
            Some(Message::Undo)
        ));
        assert!(matches!(
            shortcut(
                Physical::Code(Code::KeyZ),
                keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT
            ),
            Some(Message::Redo)
        ));
        assert!(matches!(
            shortcut(Physical::Code(Code::KeyS), keyboard::Modifiers::CTRL),
            Some(Message::Save)
        ));
    }
}
