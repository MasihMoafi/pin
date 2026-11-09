//! Native Notes feasibility build; not used by the installed launchers.
use iced::keyboard::key::{Code, Physical};
use iced::widget::{
    button, column, container, mouse_area, operation, row, space, stack, svg, text, text_editor,
    tooltip,
};
use iced::{
    event, keyboard, mouse, window, Background, Border, Color, Element, Event, Fill, Font, Padding,
    Point, Size, Subscription, Task, Theme,
};
use pin_native::{paths, storage};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

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
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Undo,
    Redo,
    Save,
    Tick,
    Pin,
    Minimize,
    Close,
    Window(window::Id, window::Event),
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
}

struct Snapshot {
    text: String,
    cursor: text_editor::Cursor,
    scroll: i32,
}
#[derive(Clone, Copy, PartialEq)]
enum Group {
    Insert,
    Backspace,
    Delete,
}

struct Notes {
    content: text_editor::Content,
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
    pointer: Point,
    context: Option<Point>,
}

fn main() -> iced::Result {
    // The prototype uses a separate preference file; user documents still use the existing path contract.
    let settings_file = paths::config_root().join("StickyNotes/iced-prototype.json");
    let bounds: Bounds = std::fs::read_to_string(&settings_file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let file = paths::data_file(true);
    let disk = storage::read(&file).unwrap_or_else(|error| {
        eprintln!("Notes: {error}");
        None
    });
    let width = if bounds.width >= 280. {
        bounds.width
    } else {
        430.
    };
    let height = if bounds.height >= 180. {
        bounds.height
    } else {
        320.
    };
    let image = image::load_from_memory(include_bytes!("../../logo/notes-256.png"))
        .expect("Notes icon")
        .into_rgba8();
    let icon = window::icon::from_rgba(image.as_raw().clone(), image.width(), image.height()).ok();
    let initial_bounds = bounds.clone();
    iced::application(
        move || {
            (
                Notes::new(
                    file.clone(),
                    settings_file.clone(),
                    initial_bounds.clone(),
                    disk.clone(),
                ),
                operation::focus(EDITOR),
            )
        },
        Notes::update,
        Notes::view,
    )
    .title("Sticky Notes — Iced prototype")
    .window(window::Settings {
        size: Size::new(width, height),
        min_size: Some(Size::new(280., 180.)),
        position: match (bounds.x, bounds.y) {
            (Some(x), Some(y)) => window::Position::Specific(Point::new(x, y)),
            _ => window::Position::Centered,
        },
        decorations: false,
        level: if bounds.pinned.unwrap_or(true) {
            window::Level::AlwaysOnTop
        } else {
            window::Level::Normal
        },
        icon,
        exit_on_close_request: false,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "sticky-notes-iced".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .theme(|_: &Notes| {
        Theme::custom(
            "Pin",
            iced::theme::Palette {
                background: BG,
                text: FG,
                primary: FG,
                success: Color::from_rgb8(5, 150, 105),
                warning: Color::from_rgb8(180, 83, 9),
                danger: Color::from_rgb8(197, 48, 48),
            },
        )
    })
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
        _ => None,
    }
}

impl Notes {
    fn new(file: PathBuf, settings_file: PathBuf, bounds: Bounds, disk: Option<String>) -> Self {
        let content = text_editor::Content::with_text(disk.as_deref().unwrap_or_default());
        let mut initial = Notes {
            content,
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
            pointer: Point::ORIGIN,
            context: None,
        };
        initial
            .content
            .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        initial
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick),
            event::listen_with(|event, status, id| match event {
                Event::Window(
                    e @ (window::Event::Opened { .. }
                    | window::Event::Moved(_)
                    | window::Event::Resized(_)
                    | window::Event::CloseRequested),
                ) => Some(Message::Window(id, e)),
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    Some(Message::Pointer(position))
                }
                Event::Keyboard(keyboard::Event::KeyPressed {
                    physical_key,
                    modifiers,
                    ..
                }) if status == event::Status::Ignored => shortcut(physical_key, modifiers),
                _ => None,
            }),
        ])
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.content.text(),
            cursor: self.content.cursor(),
            scroll: self.scroll,
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
        } else {
            self.group = None;
        }
    }
    fn history(&mut self, redo: bool) {
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
            self.content = text_editor::Content::with_text(&next.text);
            self.content.move_to(next.cursor);
            self.scroll = next.scroll;
            self.content
                .perform(text_editor::Action::Scroll { lines: self.scroll });
            self.group = None;
            self.changed();
        }
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
            let text = serde_json::to_string_pretty(&self.bounds)?;
            let disk = storage::read(&self.settings_file)?;
            storage::save(&self.settings_file, &text, &disk)?;
            Ok(())
        };
        if let Err(error) = save() {
            eprintln!("Notes window: {error}");
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                self.context = None;
                self.edit(action);
            }
            Message::Undo => {
                self.context = None;
                self.history(false);
                return operation::focus(EDITOR);
            }
            Message::Redo => {
                self.context = None;
                self.history(true);
                return operation::focus(EDITOR);
            }
            Message::Save => {
                self.save(true);
            }
            Message::Tick => {
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
            Message::Context(open) => self.context = if open { Some(self.pointer) } else { None },
            Message::Copy(cut) => {
                self.context = None;
                if let Some(selection) = self.content.selection() {
                    if cut {
                        self.edit(text_editor::Action::Edit(text_editor::Edit::Backspace));
                    }
                    return iced::clipboard::write(selection);
                }
            }
            Message::Paste => {
                self.context = None;
                return iced::clipboard::read().map(Message::Pasted);
            }
            Message::Pasted(Some(s)) => self.edit(text_editor::Action::Edit(
                text_editor::Edit::Paste(Arc::new(s)),
            )),
            Message::Pasted(None) => (),
            Message::SelectAll => {
                self.context = None;
                self.edit(text_editor::Action::SelectAll);
                return operation::focus(EDITOR);
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
                    self.group = None;
                }
            }
            Message::KeepMine => {
                if let Ok(disk) = storage::read(&self.file) {
                    self.disk = disk;
                    self.conflict = false;
                    self.save(true);
                }
            }
        }
        Task::none()
    }
    fn view(&self) -> Element<'_, Message> {
        let header = row![
            mouse_area(
                container(text("Notes").size(13))
                    .width(Fill)
                    .height(Fill)
                    .align_y(iced::Center)
            )
            .on_press(Message::Drag),
            icon_button(
                "Undo (Ctrl+Z)",
                "undo",
                (!self.undo.is_empty()).then_some(Message::Undo)
            ),
            icon_button(
                "Redo (Ctrl+Shift+Z)",
                "redo",
                (!self.redo.is_empty()).then_some(Message::Redo)
            ),
            icon_button(
                "Save (Ctrl+S)",
                if self.saved_at.is_some() {
                    "check"
                } else {
                    "save"
                },
                Some(Message::Save)
            ),
            container(space())
                .width(1)
                .height(16)
                .style(|_| container::Style::default().background(LINE)),
            icon_button(
                if self.bounds.pinned.unwrap_or(true) {
                    "Unpin from top"
                } else {
                    "Keep on top"
                },
                "pin",
                Some(Message::Pin)
            ),
            icon_button("Minimize", "minus", Some(Message::Minimize)),
            icon_button("Close", "x", Some(Message::Close))
        ]
        .spacing(2)
        .align_y(iced::Center)
        .height(42)
        .padding([0, 12]);
        let editor = text_editor(&self.content)
            .id(EDITOR)
            .on_action(Message::Edit)
            .height(Fill)
            .padding([16, 20])
            .size(15)
            .line_height(iced::widget::text::LineHeight::Relative(1.85))
            .wrapping(iced::widget::text::Wrapping::Word)
            .style(|_, _| text_editor::Style {
                background: BG.into(),
                border: Border::default(),
                placeholder: MUTED,
                value: FG,
                selection: Color::from_rgba8(161, 161, 170, 0.3),
            })
            .key_binding(|press| {
                shortcut(press.physical_key, press.modifiers)
                    .map(text_editor::Binding::Custom)
                    .or_else(|| text_editor::Binding::from_key_press(press))
            });
        let mut body = column![
            header,
            container(space())
                .height(1)
                .width(Fill)
                .style(|_| container::Style::default().background(LINE))
        ];
        if self.conflict {
            body = body.push(
                container(
                    column![
                        text("This file changed elsewhere. Choose which version to keep.").size(12),
                        row![
                            button("Use disk").on_press(Message::UseDisk),
                            button("Keep mine").on_press(Message::KeepMine)
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
        body = body.push(mouse_area(editor).on_right_press(Message::Context(true)));
        let base = container(body)
            .width(Fill)
            .height(Fill)
            .style(|_| container::Style {
                background: Some(BG.into()),
                text_color: Some(FG),
                border: Border {
                    color: LINE,
                    width: 1.,
                    radius: 0.into(),
                },
                ..Default::default()
            });
        let mut layers = stack![base];
        if let Some(p) = self.context {
            let menu = column![
                menu_item("Undo", (!self.undo.is_empty()).then_some(Message::Undo)),
                menu_item("Redo", (!self.redo.is_empty()).then_some(Message::Redo)),
                menu_item("Copy", Some(Message::Copy(false))),
                menu_item("Cut", Some(Message::Copy(true))),
                menu_item("Paste", Some(Message::Paste)),
                menu_item("Select all", Some(Message::SelectAll))
            ]
            .padding(4)
            .spacing(1);
            let overlay = container(menu).width(170).style(|_| container::Style {
                background: Some(Color::WHITE.into()),
                border: Border {
                    color: LINE,
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
        // Narrow resize targets stay outside the text field and toolbar actions.
        let right = mouse_area(container(space()).width(4).height(Fill))
            .on_press(Message::Resize(window::Direction::East))
            .interaction(mouse::Interaction::ResizingHorizontally);
        let bottom = mouse_area(container(space()).height(4).width(Fill))
            .on_press(Message::Resize(window::Direction::South))
            .interaction(mouse::Interaction::ResizingVertically);
        layers = layers.push(
            container(right)
                .width(Fill)
                .height(Fill)
                .align_x(iced::Right),
        );
        layers = layers.push(
            container(bottom)
                .width(Fill)
                .height(Fill)
                .align_y(iced::Bottom),
        );
        layers.into()
    }
}

fn menu_item(label: &'static str, action: Option<Message>) -> Element<'static, Message> {
    button(text(label).size(13))
        .width(Fill)
        .padding([5, 8])
        .on_press_maybe(action)
        .style(ghost)
        .into()
}
fn icon_button(
    label: &'static str,
    name: &str,
    action: Option<Message>,
) -> Element<'static, Message> {
    let handle=svg::Handle::from_memory(format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#202023" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{}</svg>"##,icon_path(name)).into_bytes());
    tooltip(
        button(container(svg(handle).width(15).height(15)).center(26))
            .padding(0)
            .on_press_maybe(action)
            .style(ghost),
        container(text(label).size(12))
            .padding([4, 7])
            .style(|_| container::Style {
                background: Some(FG.into()),
                text_color: Some(Color::WHITE),
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
fn ghost(_: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(Color::from_rgb8(240, 240, 241))),
        text_color: if disabled { MUTED } else { FG },
        border: Border {
            radius: 5.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn icon_path(name: &str) -> &'static str {
    match name {
        "undo" => r#"<path d="M3 7v6h6"/><path d="M21 17a9 9 0 0 0-15.5-6.4L3 13"/>"#,
        "redo" => r#"<path d="M21 7v6h-6"/><path d="M3 17a9 9 0 0 1 15.5-6.4L21 13"/>"#,
        "save" => {
            r#"<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h12l4 4v12a2 2 0 0 1-2 2Z"/><path d="M17 21v-8H7v8M7 3v5h8V3"/>"#
        }
        "pin" => r#"<path d="M12 17v5M9 7V4h6v3l-1 4 4 4v2H6v-2l4-4-1-4Z"/>"#,
        "minus" => r#"<path d="M5 12h14"/>"#,
        "check" => r#"<path d="m5 12 4 4L19 6"/>"#,
        _ => r#"<path d="m6 6 12 12M6 18 18 6"/>"#,
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
    fn typed_phrase_and_selected_delete_share_toolbar_history() {
        let mut n = note("سلام\nA first note.\n");
        let before = n.content.text();
        let cursor = n.content.cursor();
        for ch in "A new thought".chars() {
            n.edit(text_editor::Action::Edit(text_editor::Edit::Insert(ch)));
        }
        let typed = n.content.text();
        n.history(false);
        assert_eq!(n.content.text(), before);
        assert_eq!(n.content.cursor(), cursor);
        n.history(true);
        assert_eq!(n.content.text(), typed);
        n.edit(text_editor::Action::SelectAll);
        n.edit(text_editor::Action::Edit(text_editor::Edit::Backspace));
        assert!(n.content.text().is_empty());
        n.history(false);
        assert_eq!(n.content.text(), typed);
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
