use super::{bg, fg, focus_style, icon_button, icon_data, line, muted, Group};
use crate::board::{valid_date, Board, Task as Item, COLORS};
use chrono::{Datelike, Days, Local, Months, NaiveDate, NaiveDateTime};
use iced::keyboard::{
    self,
    key::{Code, Named, Physical},
};
use iced::widget::{
    button, checkbox, column, container, keyed_column, mouse_area, operation, row, scrollable,
    space, stack, svg, text, text_editor,
};
use iced::{Border, Color, Element, Fill, Length, Padding, Point, Size, Task, Theme};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

const LIST: &str = "pin-list";
const OPTIONS: &str = "pin-options";
const TEXT_SIZE: f32 = 15.;
const LINE_HEIGHT: f32 = 24.5;
const EDITOR_PADDING: f32 = 3.;
const CONTROL_HEIGHT: f32 = LINE_HEIGHT + 2. * EDITOR_PADDING;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Item(u64),
    Color,
}
impl Target {
    fn id(self) -> String {
        match self {
            Self::Item(id) => format!("task-{id}"),
            Self::Color => "pin-color".into(),
        }
    }
    fn key(self) -> u64 {
        match self {
            Self::Item(id) => id,
            Self::Color => u64::MAX - 2,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Edit(Target, text_editor::Action),
    New(bool),
    Split(u64),
    Remove(u64),
    Toggle(u64),
    Fold(u64),
    Indent(u64, bool),
    Move(u64, bool),
    Options(u64),
    CloseOptions,
    Panel(Panel),
    Month(bool),
    CalendarMove(i32),
    PickDate(NaiveDate),
    TimePart(bool, String),
    Daily,
    Color(Option<String>),
    ApplyDate,
    ClearDate,
    ClearTime,
    DismissOptions,
    ApplyColor,
    OptionsTab(bool),
    Activate,
    Undo,
    Redo,
    Save,
    Header(bool),
    Context(Target),
    Copy(bool),
    Paste,
    Pasted(Option<String>),
    SelectAll,
    DeleteWord(Target, bool),
    Grip(u64),
    Over(u64, bool),
    Leave(u64),
    Drop,
    CancelDrag,
    Scroll(scrollable::Viewport),
    OptionsScroll(scrollable::Viewport),
    ScrollTo(&'static str, f32),
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Main,
    Date,
    Time,
    Color,
    Hours,
    Minutes,
}

struct Options {
    id: u64,
    date: text_editor::Content,
    time: text_editor::Content,
    color: text_editor::Content,
    focus: u8,
    offset: scrollable::AbsoluteOffset,
    error: Option<(String, Instant)>,
    panel: Panel,
    calendar: NaiveDate,
    anchor: Point,
}
struct EditorSnapshot {
    text: String,
    cursor: text_editor::Cursor,
}
impl EditorSnapshot {
    fn new(content: &text_editor::Content) -> Self {
        Self {
            text: content.text(),
            cursor: content.cursor(),
        }
    }
    fn restore(self) -> text_editor::Content {
        let mut c = text_editor::Content::with_text(&self.text);
        c.move_to(self.cursor);
        c
    }
}
pub struct Snapshot {
    board: Board,
    editors: BTreeMap<u64, EditorSnapshot>,
    folded: HashSet<u64>,
    active: Target,
    options: Option<Options>,
    offset: scrollable::AbsoluteOffset,
}
pub struct State {
    board: Board,
    editors: BTreeMap<u64, text_editor::Content>,
    folded: HashSet<u64>,
    active: Target,
    options: Option<Options>,
    next_id: u64,
    today: String,
    offset: scrollable::AbsoluteOffset,
    dragging: Option<u64>,
    drop: Option<(u64, bool)>,
    pub revision: u64,
}

impl State {
    pub fn new(md: &str, folded: &[String]) -> Self {
        let mut board = Board::parse(md);
        let today = Local::now().format("%Y-%m-%d").to_string();
        board.rollover(&today);
        board.completed_last();
        let editors = board
            .tasks
            .iter()
            .map(|t| (t.id, text_editor::Content::with_text(&t.text)))
            .collect();
        let folds = board
            .tasks
            .iter()
            .enumerate()
            .filter(|(i, _)| folded.contains(&fold_key(&board, *i)))
            .map(|(_, t)| t.id)
            .collect();
        let active = Target::Item(board.tasks.first().map_or(0, |t| t.id));
        let next_id = board.tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        Self {
            board,
            editors,
            folded: folds,
            active,
            options: None,
            next_id,
            today,
            offset: scrollable::AbsoluteOffset::default(),
            dragging: None,
            drop: None,
            revision: 0,
        }
    }
    pub fn has_options(&self) -> bool {
        self.options.is_some()
    }
    pub fn markdown(&self) -> String {
        self.board.markdown()
    }
    pub fn folded_keys(&self) -> Vec<String> {
        self.board
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| self.folded.contains(&t.id))
            .map(|(i, _)| fold_key(&self.board, i))
            .collect()
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            board: self.board.clone(),
            editors: self
                .editors
                .iter()
                .map(|(id, c)| (*id, EditorSnapshot::new(c)))
                .collect(),
            folded: self.folded.clone(),
            active: self.active,
            options: self.options.as_ref().map(|o| Options {
                id: o.id,
                date: EditorSnapshot::new(&o.date).restore(),
                time: EditorSnapshot::new(&o.time).restore(),
                color: EditorSnapshot::new(&o.color).restore(),
                focus: o.focus,
                offset: o.offset,
                panel: o.panel,
                calendar: o.calendar,
                anchor: o.anchor,
                error: None,
            }),
            offset: self.offset,
        }
    }
    pub fn restore(&mut self, snapshot: Snapshot) -> Task<Message> {
        self.board = snapshot.board;
        self.board.rollover(&self.today);
        self.editors = snapshot
            .editors
            .into_iter()
            .map(|(id, c)| (id, c.restore()))
            .collect();
        self.folded = snapshot.folded;
        self.active = snapshot.active;
        self.options = snapshot.options;
        self.offset = snapshot.offset;
        self.dragging = None;
        self.drop = None;
        self.sync();
        Task::batch([
            self.focus(),
            operation::scroll_to(LIST, self.offset),
            self.options
                .as_ref()
                .map_or_else(Task::none, |o| operation::scroll_to(OPTIONS, o.offset)),
        ])
    }
    pub fn focus(&self) -> Task<Message> {
        if let Some(o) = &self.options {
            return if o.panel == Panel::Color && o.focus == 18 {
                operation::focus(Target::Color.id())
            } else {
                operation::focus("pin-menu")
            };
        }
        operation::focus(self.active.id())
    }
    fn focus_and_reveal(&self) -> Task<Message> {
        let (scroll, target) = if let Some(o) = &self.options {
            (
                OPTIONS,
                match o.focus {
                    18 if o.panel == Panel::Color => Target::Color.id(),
                    n => format!("option-{n}"),
                },
            )
        } else {
            (LIST, self.active.id())
        };
        Task::batch([
            self.focus(),
            iced::advanced::widget::operate(super::reveal::Reveal::new(scroll, target)),
        ])
    }
    pub fn selection(&self) -> Option<String> {
        self.editor(self.active)
            .and_then(text_editor::Content::selection)
    }
    pub fn set_active(&mut self, target: Target) {
        self.active = target;
        if let Some(o) = &mut self.options {
            match target {
                Target::Color => o.focus = 18,
                _ => (),
            }
        }
    }
    fn item_id(&self) -> u64 {
        match self.active {
            Target::Item(id) => id,
            _ => self.options_item(),
        }
    }
    fn options_item(&self) -> u64 {
        self.options.as_ref().map_or(0, |o| o.id)
    }
    fn editor(&self, target: Target) -> Option<&text_editor::Content> {
        match target {
            Target::Item(id) => self.editors.get(&id),
            Target::Color => self.options.as_ref().map(|o| &o.color),
        }
    }
    fn editor_mut(&mut self, target: Target) -> Option<&mut text_editor::Content> {
        match target {
            Target::Item(id) => self.editors.get_mut(&id),
            Target::Color => self.options.as_mut().map(|o| &mut o.color),
        }
    }
    pub fn group(message: &Message) -> Option<Group> {
        match message {
            Message::Edit(target, action) => match action {
                text_editor::Action::Edit(text_editor::Edit::Insert(_)) => {
                    Some(Group::Board(target.key(), 0))
                }
                text_editor::Action::Edit(text_editor::Edit::Backspace) => {
                    Some(Group::Board(target.key(), 1))
                }
                text_editor::Action::Edit(text_editor::Edit::Delete) => {
                    Some(Group::Board(target.key(), 2))
                }
                _ => None,
            },
            _ => None,
        }
    }
    fn sync(&mut self) {
        self.editors
            .retain(|id, _| self.board.tasks.iter().any(|t| t.id == *id));
        for t in &self.board.tasks {
            let c = self.editors.entry(t.id).or_default();
            if c.text() != t.text {
                *c = text_editor::Content::with_text(&t.text);
            }
        }
    }
    fn edit(&mut self, target: Target, action: text_editor::Action) {
        self.set_active(target);
        let Some(c) = self.editor_mut(target) else {
            return;
        };
        let before = c.text();
        let action = match action {
            text_editor::Action::Edit(text_editor::Edit::Paste(value)) => {
                text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(
                    value.replace(['\r', '\n'], " "),
                )))
            }
            action => action,
        };
        c.perform(action);
        let value = c.text();
        if before != value {
            if let Target::Item(id) = target {
                if let Some(i) = self.board.index(id) {
                    self.board.tasks[i].text = value.replace(['\r', '\n'], " ");
                    self.sync();
                }
            }
            self.revision += 1;
        }
    }
    fn new_item(&mut self, parent: Option<u64>, text: String) {
        let p = parent.and_then(|id| self.board.index(id));
        let mut item = Item::blank(
            self.next_id,
            p.map_or(0, |i| self.board.tasks[i].depth + 1),
            match p {
                Some(i) => self.board.tasks[i].color.clone(),
                None => self.board.tasks.first().and_then(|t| t.color.clone()),
            },
        );
        self.next_id += 1;
        item.text = text;
        self.active = Target::Item(item.id);
        if let Some(p) = parent {
            self.folded.remove(&p);
        }
        self.board.tasks.insert(p.map_or(0, |i| i + 1), item);
        self.board.completed_last();
        self.sync();
        self.revision += 1;
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(t, a) => self.edit(t, a),
            Message::New(sub) => {
                let parent = sub.then(|| self.item_id());
                self.options = None;
                self.new_item(parent, String::new());
                return self.focus_and_reveal();
            }
            Message::Split(id) => {
                if let Some(i) = self.board.index(id) {
                    let c = &self.editors[&id];
                    let cursor = c.cursor();
                    let (a, b) = cursor.selection.map_or(
                        (cursor.position.column, cursor.position.column),
                        |s| {
                            (
                                s.column.min(cursor.position.column),
                                s.column.max(cursor.position.column),
                            )
                        },
                    );
                    let original = self.board.tasks[i].clone();
                    let mut item =
                        Item::blank(self.next_id, original.depth, original.color.clone());
                    self.next_id += 1;
                    item.text = original.text[b.min(original.text.len())..].to_owned();
                    self.board.tasks[i].text =
                        original.text[..a.min(original.text.len())].to_owned();
                    self.active = Target::Item(item.id);
                    let end = self.board.end(i);
                    self.board.tasks.insert(end, item);
                    self.board.completed_last();
                    self.sync();
                    self.revision += 1;
                    return self.focus_and_reveal();
                }
            }
            Message::Remove(id) => {
                if let Some(i) = self.board.index(id) {
                    let end = self.board.end(i);
                    let prev = i
                        .checked_sub(1)
                        .and_then(|p| self.board.tasks.get(p))
                        .map(|t| t.id);
                    self.board.tasks.drain(i..end);
                    self.options = None;
                    self.active = Target::Item(
                        prev.or_else(|| self.board.tasks.first().map(|t| t.id))
                            .unwrap_or(0),
                    );
                    self.sync();
                    self.revision += 1;
                    return self.focus_and_reveal();
                }
            }
            Message::Toggle(id) => {
                self.board.toggle_done(id, &self.today);
                self.revision += 1;
                return self.focus_and_reveal();
            }
            Message::Fold(id) => {
                if !self.folded.remove(&id) {
                    self.folded.insert(id);
                }
                self.revision += 1;
            }
            Message::Indent(id, out) => {
                if self.board.indent(id, out) {
                    self.revision += 1;
                }
                return self.focus_and_reveal();
            }
            Message::Move(id, down) => {
                if let Some(i) = self.board.index(id) {
                    let siblings = self.board.siblings(i);
                    let at = siblings.iter().position(|j| *j == i).unwrap_or(0);
                    let next = if down {
                        Some(at + 1)
                    } else {
                        at.checked_sub(1)
                    };
                    if let Some(j) = next.and_then(|n| siblings.get(n)) {
                        let dest = self.board.tasks[*j].id;
                        if self.board.move_branch(id, dest, down) {
                            self.revision += 1;
                        }
                    }
                }
                return self.focus_and_reveal();
            }
            Message::Options(id) => {
                if let Some(i) = self.board.index(id) {
                    let t = &self.board.tasks[i];
                    let (d, time) = t
                        .date
                        .as_deref()
                        .unwrap_or("")
                        .split_once(' ')
                        .unwrap_or((t.date.as_deref().unwrap_or(""), ""));
                    self.options = Some(Options {
                        id,
                        date: text_editor::Content::with_text(d),
                        time: text_editor::Content::with_text(time),
                        color: text_editor::Content::with_text(
                            &t.color.as_ref().map_or(String::new(), |c| color_hex(c)),
                        ),
                        focus: 1,
                        offset: scrollable::AbsoluteOffset::default(),
                        error: None,
                        panel: Panel::Main,
                        calendar: NaiveDate::parse_from_str(d, "%Y-%m-%d")
                            .unwrap_or_else(|_| Local::now().date_naive()),
                        anchor: Point::new(f32::MAX, 8.),
                    });
                    self.active = Target::Item(id);
                    return self.focus_and_reveal();
                }
            }
            Message::DismissOptions => {
                let id = self.options_item();
                self.options = None;
                self.active = Target::Item(id);
                return self.focus_and_reveal();
            }
            Message::CloseOptions => {
                if self
                    .options
                    .as_ref()
                    .is_some_and(|o| o.panel != Panel::Main)
                {
                    return self.update(Message::Panel(Panel::Main));
                }
                let id = self.options_item();
                self.options = None;
                self.active = Target::Item(id);
                return operation::scroll_to(LIST, self.offset).chain(self.focus_and_reveal());
            }
            Message::Panel(panel) => {
                if let Some(o) = &mut self.options {
                    o.panel = panel;
                    o.focus = match panel {
                        Panel::Main => 1,
                        Panel::Date => 32 + o.calendar.day() as u8 - 1,
                        Panel::Time => 25,
                        Panel::Color => 5,
                        Panel::Hours => {
                            64 + o
                                .time
                                .text()
                                .trim()
                                .get(..2)
                                .and_then(|s| s.parse::<u8>().ok())
                                .unwrap_or(9)
                        }
                        Panel::Minutes => {
                            64 + o
                                .time
                                .text()
                                .trim()
                                .get(3..5)
                                .and_then(|s| s.parse::<u8>().ok())
                                .unwrap_or(0)
                        }
                    };
                    o.offset = scrollable::AbsoluteOffset::default();
                    return operation::scroll_to(OPTIONS, o.offset).chain(self.focus());
                }
            }
            Message::Month(next) => {
                if let Some(o) = &mut self.options {
                    o.calendar = if next {
                        o.calendar.checked_add_months(Months::new(1))
                    } else {
                        o.calendar.checked_sub_months(Months::new(1))
                    }
                    .unwrap_or(o.calendar);
                    o.focus = 32 + o.calendar.day() as u8 - 1;
                }
            }
            Message::CalendarMove(delta) => {
                if let Some(o) = &mut self.options {
                    if o.panel == Panel::Date {
                        o.calendar = if delta >= 0 {
                            o.calendar.checked_add_days(Days::new(delta as u64))
                        } else {
                            o.calendar.checked_sub_days(Days::new((-delta) as u64))
                        }
                        .unwrap_or(o.calendar);
                        o.focus = 32 + o.calendar.day() as u8 - 1;
                    }
                }
            }
            Message::PickDate(date) => {
                if let Some(o) = &mut self.options {
                    o.date = text_editor::Content::with_text(&date.format("%Y-%m-%d").to_string());
                    o.calendar = date;
                }
                let task = self.update(Message::ApplyDate);
                return task.chain(self.update(Message::Panel(Panel::Main)));
            }
            Message::TimePart(hour, value) => {
                if let Some(o) = &mut self.options {
                    let current = o.time.text();
                    let (h, m) = current.trim().split_once(':').unwrap_or(("09", "00"));
                    o.time = text_editor::Content::with_text(&if hour {
                        format!("{value}:{m}")
                    } else {
                        format!("{h}:{value}")
                    });
                    if o.date.text().trim().is_empty() {
                        o.date = text_editor::Content::with_text(&self.today);
                    }
                }
                let save = self.update(Message::ApplyDate);
                return save.chain(self.update(Message::Panel(Panel::Time)));
            }
            Message::Daily => {
                let id = self.options_item();
                if let Some(i) = self.board.index(id) {
                    let t = &mut self.board.tasks[i];
                    t.daily = !t.daily;
                    t.completed = (t.daily && t.done).then(|| self.today.clone());
                    self.revision += 1;
                }
            }
            Message::Color(color) => {
                if let Some(i) = self.board.index(self.options_item()) {
                    self.board.tasks[i].color = color;
                    self.revision += 1;
                }
            }
            Message::ApplyColor => {
                let value = self
                    .options
                    .as_ref()
                    .map(|o| o.color.text().trim().to_owned())
                    .unwrap_or_default();
                if value.len() == 7
                    && value.starts_with('#')
                    && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return self.update(Message::Color(Some(value.to_lowercase())));
                }
                self.options_error("Use a color like #ec4899.");
            }
            Message::ApplyDate => {
                if let Some(o) = &self.options {
                    let d = o.date.text().trim().to_owned();
                    let time = o.time.text().trim().to_owned();
                    let value = if time.is_empty() {
                        d.clone()
                    } else {
                        format!("{d} {time}")
                    };
                    if valid_date(&value) {
                        if let Some(i) = self.board.index(self.options_item()) {
                            self.board.tasks[i].date = (!value.is_empty()).then_some(value);
                            self.revision += 1;
                            self.options.as_mut().unwrap().error = None;
                        }
                    } else {
                        self.options_error("Use YYYY-MM-DD and optional HH:MM.");
                    }
                }
            }
            Message::ClearTime => {
                if let Some(o) = &mut self.options {
                    o.time = text_editor::Content::new();
                }
                return self.update(Message::ApplyDate);
            }
            Message::ClearDate => {
                if let Some(i) = self.board.index(self.options_item()) {
                    self.board.tasks[i].date = None;
                    if let Some(o) = &mut self.options {
                        o.date = text_editor::Content::new();
                        o.time = text_editor::Content::new();
                        o.error = None;
                    }
                    self.revision += 1;
                }
            }
            Message::OptionsTab(reverse) => {
                let order = self.option_order();
                if let Some(o) = &mut self.options {
                    let at = order.iter().position(|n| *n == o.focus).unwrap_or(0);
                    o.focus = order[(at + if reverse { order.len() - 1 } else { 1 }) % order.len()];
                    let target = match o.focus {
                        18 if o.panel == Panel::Color => Some(Target::Color),
                        _ => None,
                    };
                    if let Some(target) = target {
                        self.active = target;
                    }
                    return self.focus_and_reveal();
                }
            }
            Message::Activate => {
                if let Some(o) = &self.options {
                    if let Some(action) = self.option_action(o.focus) {
                        return self.update(action);
                    }
                }
            }
            Message::Copy(cut) => {
                if let Some(value) = self.selection() {
                    if cut {
                        self.edit(
                            self.active,
                            text_editor::Action::Edit(text_editor::Edit::Backspace),
                        );
                    }
                    return Task::batch([iced::clipboard::write(value), self.focus()]);
                }
            }
            Message::Paste => return iced::clipboard::read().map(Message::Pasted),
            Message::Pasted(Some(value)) => {
                self.edit(
                    self.active,
                    text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(
                        value.replace(['\r', '\n'], " "),
                    ))),
                );
                return self.focus_and_reveal();
            }
            Message::Pasted(None) => (),
            Message::SelectAll => {
                self.edit(self.active, text_editor::Action::SelectAll);
                return self.focus_and_reveal();
            }
            Message::DeleteWord(target, forward) => {
                self.active = target;
                if let Some(c) = self.editor_mut(target) {
                    let before = c.text();
                    super::delete_word(c, forward);
                    if c.text() != before {
                        if let Target::Item(id) = target {
                            let value = self.editors[&id].text();
                            if let Some(i) = self.board.index(id) {
                                self.board.tasks[i].text = value;
                            }
                        }
                        self.revision += 1;
                    }
                }
            }
            Message::Grip(id) => {
                self.dragging = Some(id);
                self.drop = None;
            }
            Message::Over(target, after) => {
                if let Some(source) = self.dragging {
                    if let (Some(i), Some(j)) = (self.board.index(source), self.board.index(target))
                    {
                        self.drop =
                            (!(i..self.board.end(i)).contains(&j)).then_some((target, after));
                    }
                }
            }
            Message::Leave(id) => {
                if self.drop.is_some_and(|(target, _)| target == id) {
                    self.drop = None;
                }
            }
            Message::Drop => {
                if let (Some(id), Some((target, after))) = (self.dragging.take(), self.drop.take())
                {
                    if self.board.move_branch(id, target, after) {
                        self.revision += 1;
                    }
                }
            }
            Message::CancelDrag => {
                self.dragging = None;
                self.drop = None;
            }
            Message::Scroll(v) => self.offset = v.absolute_offset(),
            Message::OptionsScroll(v) => {
                if let Some(o) = &mut self.options {
                    o.offset = v.absolute_offset();
                }
            }
            Message::ScrollTo(id, y) => {
                return operation::scroll_to(id, scrollable::AbsoluteOffset { x: 0., y })
            }
            Message::Tick => {
                let day = Local::now().format("%Y-%m-%d").to_string();
                if day != self.today {
                    self.today = day;
                    if self.board.rollover(&self.today) {
                        self.revision += 1;
                    }
                }
                if let Some(o) = &mut self.options {
                    if o.error
                        .as_ref()
                        .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(6))
                    {
                        o.error = None;
                    }
                }
            }
            Message::Undo
            | Message::Redo
            | Message::Save
            | Message::Header(_)
            | Message::Context(_) => (),
        }
        Task::none()
    }
    fn options_error(&mut self, value: &str) {
        if let Some(o) = &mut self.options {
            o.error = Some((value.into(), Instant::now()));
        }
    }
    fn option_action(&self, focus: u8) -> Option<Message> {
        let o = self.options.as_ref()?;
        Some(match focus {
            0 => Message::Daily,
            1 => Message::Panel(Panel::Date),
            2 => Message::Panel(Panel::Time),
            3 => Message::ApplyDate,
            4 => Message::ClearDate,
            5..=16 => Message::Color(Some(COLORS[(focus - 5) as usize].0.into())),
            17 => Message::Color(None),
            18 if o.panel == Panel::Main => Message::Panel(Panel::Color),
            19 => Message::ApplyColor,
            20 => Message::New(true),
            21 => Message::Remove(self.options_item()),
            22 => Message::CloseOptions,
            23 => Message::Month(false),
            24 => Message::Month(true),
            25 => Message::Panel(Panel::Hours),
            26 => Message::Panel(Panel::Minutes),
            27 => Message::ClearTime,
            64..=123 if matches!(o.panel, Panel::Hours | Panel::Minutes) => {
                Message::TimePart(o.panel == Panel::Hours, format!("{:02}", focus - 64))
            }
            32..=62 => Message::PickDate(o.calendar.with_day((focus - 31) as u32)?),
            _ => return None,
        })
    }

    fn option_order(&self) -> Vec<u8> {
        let Some(o) = &self.options else {
            return vec![];
        };
        match o.panel {
            Panel::Main => vec![0, 1, 2, 18, 20, 21, 22],
            Panel::Date => {
                let mut order = vec![22, 23, 24];
                order.extend(
                    (1..=31).filter_map(|day| o.calendar.with_day(day).map(|_| day as u8 + 31)),
                );
                order.push(4);
                order
            }
            Panel::Time => vec![22, 25, 26, 27],
            Panel::Hours => (64..88).chain([22]).collect(),
            Panel::Minutes => (64..124).chain([22]).collect(),
            Panel::Color => (5..=19).chain([22]).collect(),
        }
    }

    pub fn calendar_open(&self) -> bool {
        self.options
            .as_ref()
            .is_some_and(|o| o.panel == Panel::Date)
    }

    pub fn anchor_options(&mut self, pointer: Point) {
        if let Some(o) = &mut self.options {
            o.anchor = Point::new(pointer.x, (pointer.y - 38.).max(6.));
        }
    }

    fn binding(
        &self,
        target: Target,
        press: text_editor::KeyPress,
    ) -> Option<text_editor::Binding<Message>> {
        use text_editor::Binding as B;
        if !matches!(press.status, text_editor::Status::Focused { .. }) {
            return None;
        }
        let command = press.modifiers.command();
        let shift = press.modifiers.shift();
        let id = match target {
            Target::Item(id) => id,
            _ => self.options_item(),
        };
        let custom = if command {
            match press.physical_key {
                Physical::Code(Code::KeyZ) => {
                    Some(if shift { Message::Redo } else { Message::Undo })
                }
                Physical::Code(Code::KeyY) => Some(Message::Redo),
                Physical::Code(Code::KeyS) => Some(Message::Save),
                Physical::Code(Code::KeyN) => Some(Message::New(false)),
                Physical::Code(Code::KeyT) => Some(Message::New(true)),
                Physical::Code(Code::KeyD) => Some(Message::Options(id)),
                Physical::Code(Code::Period) => Some(Message::Fold(id)),
                _ => None,
            }
        } else {
            None
        };
        if let Some(message) = custom {
            return Some(B::Custom(message));
        }
        let message = match press.key.as_ref() {
            keyboard::Key::Named(Named::Tab) if command => Some(Message::Header(shift)),
            keyboard::Key::Named(Named::Tab) if self.options.is_some() => {
                Some(Message::OptionsTab(shift))
            }
            keyboard::Key::Named(Named::Tab) => Some(Message::Indent(id, shift)),
            keyboard::Key::Named(Named::Enter) if command => Some(Message::Toggle(id)),
            keyboard::Key::Named(Named::Enter) if self.options.is_some() => {
                Some(if target == Target::Color {
                    Message::ApplyColor
                } else {
                    Message::ApplyDate
                })
            }
            keyboard::Key::Named(Named::Enter) => Some(Message::Split(id)),
            keyboard::Key::Named(Named::Backspace) if command && shift => Some(Message::Remove(id)),
            keyboard::Key::Named(Named::Backspace) if press.modifiers.jump() => {
                Some(Message::DeleteWord(target, false))
            }
            keyboard::Key::Named(Named::Delete) if press.modifiers.jump() => {
                Some(Message::DeleteWord(target, true))
            }
            keyboard::Key::Named(Named::Backspace)
                if self.options.is_none()
                    && self.board.index(id).is_some_and(|i| {
                        self.board.tasks[i].text.is_empty() && self.board.end(i) == i + 1
                    }) =>
            {
                Some(Message::Remove(id))
            }
            keyboard::Key::Named(Named::ArrowUp | Named::ArrowDown) if press.modifiers.alt() => {
                Some(Message::Move(
                    id,
                    matches!(press.key.as_ref(), keyboard::Key::Named(Named::ArrowDown)),
                ))
            }
            keyboard::Key::Named(Named::Escape) if self.options.is_some() => {
                Some(Message::CloseOptions)
            }
            keyboard::Key::Named(Named::ContextMenu) => Some(Message::Context(target)),
            keyboard::Key::Named(Named::F10) if shift => Some(Message::Context(target)),
            _ => None,
        };
        if let Some(message) = message {
            let unfocus = matches!(message, Message::OptionsTab(_) | Message::Header(_));
            return Some(if unfocus {
                B::Sequence(vec![B::Unfocus, B::Custom(message)])
            } else {
                B::Custom(message)
            });
        }
        B::from_key_press(press)
    }

    pub fn view(&self, size: Size) -> Element<'_, Message> {
        let mut hidden = None;
        let mut visible = vec![];
        for (i, t) in self.board.tasks.iter().enumerate() {
            if hidden.is_some_and(|depth| t.depth > depth) {
                continue;
            }
            hidden = None;
            visible.push((i, t));
            if self.folded.contains(&t.id) {
                hidden = Some(t.depth);
            }
        }
        let last = visible.last().map(|(_, t)| t.id);
        let items = keyed_column(
            visible
                .into_iter()
                .map(|(i, t)| (t.id, self.task_view(i, t))),
        )
        .spacing(8);
        let mut body = column![items].spacing(4).padding([12, 11]);
        if self.board.tasks.is_empty() {
            body = body.push(
                container(
                    text("Add your first priority.")
                        .size(14)
                        .color(super::MUTED),
                )
                .padding([12, 6]),
            );
        }
        let add = button(text("+ Add task").size(13))
            .padding([7, 9])
            .on_press(Message::New(false))
            .style(super::ghost);
        let footer = mouse_area(container(add).width(Fill).padding([2, 0]))
            .on_enter(last.map_or(Message::CancelDrag, |id| Message::Over(id, true)))
            .on_exit(last.map_or(Message::CancelDrag, Message::Leave));
        body = body.push(footer);
        let list = container(
            scrollable(body)
                .id(LIST)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new().width(6).scroller_width(6),
                ))
                .on_scroll(Message::Scroll)
                .height(Fill),
        )
        .padding(iced::Padding {
            right: 8.,
            ..Default::default()
        })
        .width(Fill)
        .height(Fill);
        let Some(o) = &self.options else {
            return list.into();
        };
        let width = 270_f32.min(size.width - 20.);
        let height = match o.panel {
            Panel::Main => 260_f32,
            Panel::Date => 350.,
            Panel::Time => 170.,
            Panel::Color => 235.,
            Panel::Hours | Panel::Minutes => 260.,
        }
        .min((size.height - 60.).max(80.));
        let left = o.anchor.x.min((size.width - width - 10.).max(10.)).max(10.);
        let top = o.anchor.y.min((size.height - height - 50.).max(6.)).max(6.);
        let menu = container(self.options_view())
            .id("pin-menu")
            .width(width)
            .max_height(height)
            .style(|theme| container::Style {
                background: Some(bg(theme).into()),
                border: Border {
                    color: line(theme),
                    width: 1.,
                    radius: 6.into(),
                },
                ..Default::default()
            });
        stack![
            list,
            mouse_area(container(space()).width(Fill).height(Fill))
                .on_press(Message::DismissOptions),
            container(menu).padding(Padding {
                left,
                top,
                right: 0.,
                bottom: 0.
            })
        ]
        .into()
    }
    fn task_view<'a>(&'a self, i: usize, t: &'a Item) -> Element<'a, Message> {
        let id = t.id;
        let child = self.board.end(i) > i + 1;
        let chevron: Element<'_, Message> = if child {
            icon_button(
                "Fold / unfold (Ctrl+.)",
                if self.folded.contains(&id) {
                    "right"
                } else {
                    "down"
                },
                Some(super::Message::Board(Message::Fold(id))),
                false,
            )
            .map(move |m| match m {
                super::Message::Board(m) => m,
                _ => Message::Fold(id),
            })
        } else {
            space().width(26).into()
        };
        let editor = text_editor(&self.editors[&id])
            .id(Target::Item(id).id())
            .on_action(move |a| Message::Edit(Target::Item(id), a))
            .height(Length::Shrink)
            .padding([EDITOR_PADDING, 0.])
            .size(TEXT_SIZE)
            .line_height(iced::widget::text::LineHeight::Absolute(LINE_HEIGHT.into()))
            .wrapping(iced::widget::text::Wrapping::Word)
            .placeholder("New priority")
            .style(move |theme, _| text_editor::Style {
                background: Color::TRANSPARENT.into(),
                border: Border::default(),
                placeholder: muted(theme),
                value: if t.done { muted(theme) } else { fg(theme) },
                selection: Color::from_rgba8(161, 161, 170, 0.3),
            })
            .key_binding(move |p| self.binding(Target::Item(id), p));
        let mut content =
            column![mouse_area(editor).on_right_press(Message::Context(Target::Item(id)))];
        if t.date.is_some() || t.daily {
            let mut badges = row![].spacing(8).align_y(iced::Center);
            if let Some(date) = &t.date {
                let label = display_date(date);
                let today = self.today.clone();
                let date = date.clone();
                let done = t.done;
                badges = badges.push(text(label).size(11).color(
                    if !done && date.get(..10).is_some_and(|d| d < today.as_str()) {
                        Color::from_rgb8(194, 65, 65)
                    } else if !done && date.starts_with(&today) {
                        Color::from_rgb8(22, 145, 87)
                    } else {
                        super::MUTED
                    },
                ));
            }
            if t.daily {
                badges = badges.push(text("↻ Daily").size(11).color(super::MUTED));
            }
            content = content.push(badges);
        }
        let grip = mouse_area(
            container(
                svg(svg::Handle::from_memory(icon_data("grip")))
                    .width(14)
                    .height(14)
                    .style(|theme, _| svg::Style {
                        color: Some(muted(theme)),
                    }),
            )
            .center(24),
        )
        .on_press(Message::Grip(id))
        .interaction(iced::mouse::Interaction::Grab);
        let options = icon_button(
            "Task options (Ctrl+D)",
            "more",
            Some(super::Message::Board(Message::Options(id))),
            false,
        )
        .map(move |_| Message::Options(id));
        let check = checkbox(t.done)
            .size(17)
            .on_toggle(move |_| Message::Toggle(id))
            .style(|theme, status| {
                let checked = matches!(
                    status,
                    checkbox::Status::Active { is_checked: true }
                        | checkbox::Status::Hovered { is_checked: true }
                        | checkbox::Status::Disabled { is_checked: true }
                );
                checkbox::Style {
                    background: if checked {
                        Color::from_rgb8(45, 151, 148).into()
                    } else {
                        bg(theme).into()
                    },
                    icon_color: Color::WHITE,
                    border: Border {
                        color: if checked {
                            Color::from_rgb8(45, 151, 148)
                        } else {
                            muted(theme)
                        },
                        width: 2.,
                        radius: 4.into(),
                    },
                    text_color: None,
                }
            });
        let row = row![
            container(chevron)
                .height(CONTROL_HEIGHT)
                .align_y(iced::Center),
            container(check)
                .height(CONTROL_HEIGHT)
                .align_y(iced::Center)
                .padding(iced::Padding {
                    right: 5.,
                    ..Default::default()
                }),
            container(content).width(Fill),
            container(grip).height(CONTROL_HEIGHT).align_y(iced::Center),
            container(options)
                .height(CONTROL_HEIGHT)
                .align_y(iced::Center)
        ]
        .spacing(5)
        .padding(iced::Padding {
            top: 4.,
            bottom: 6.,
            left: (t.depth.min(6) * 16) as f32,
            right: 4.,
        })
        .align_y(iced::Top);
        let indicator = self.drop == Some((id, false));
        let accent = container(space())
            .width(Fill)
            .height(2)
            .style(move |theme| {
                container::Style::default().background(if indicator {
                    fg(theme)
                } else {
                    Color::TRANSPARENT
                })
            });
        let color = t.color.clone();
        let after = self.drop == Some((id, true));
        let bottom = container(space())
            .width(Fill)
            .height(2)
            .style(move |theme| {
                container::Style::default().background(if after {
                    fg(theme)
                } else {
                    Color::TRANSPARENT
                })
            });
        let body = container(column![accent, row, bottom])
            .width(Fill)
            .style(move |theme| container::Style {
                background: Some(tint(theme, color.as_deref()).into()),
                border: Border {
                    radius: 6.into(),
                    ..Border::default()
                },
                ..Default::default()
            });
        mouse_area(body)
            .on_enter(Message::Over(id, false))
            .on_exit(Message::Leave(id))
            .into()
    }
    fn field(&self, target: Target, placeholder: &'static str) -> Element<'_, Message> {
        let content = self.editor(target).expect("option field");
        let editor = text_editor(content)
            .id(target.id())
            .on_action(move |a| Message::Edit(target, a))
            .height(34)
            .padding([6, 8])
            .size(13)
            .placeholder(placeholder)
            .key_binding(move |p| self.binding(target, p))
            .style(|theme, status| text_editor::Style {
                background: bg(theme).into(),
                border: Border {
                    color: if matches!(status, text_editor::Status::Focused { .. }) {
                        muted(theme)
                    } else {
                        line(theme)
                    },
                    width: 1.,
                    radius: 5.into(),
                },
                placeholder: muted(theme),
                value: fg(theme),
                selection: Color::from_rgba8(161, 161, 170, 0.3),
            });
        mouse_area(editor)
            .on_right_press(Message::Context(target))
            .into()
    }
    fn option_button<'a>(&'a self, label: impl Into<String>, focus: u8) -> Element<'a, Message> {
        let focused = self.options.as_ref().is_some_and(|o| o.focus == focus);
        container(
            button(text(label.into()).size(13))
                .padding([6, 9])
                .on_press_maybe(self.option_action(focus))
                .style(move |theme, status| focus_style(theme, status, focused)),
        )
        .id(format!("option-{focus}"))
        .into()
    }
    fn options_view(&self) -> Element<'_, Message> {
        let o = self.options.as_ref().unwrap();
        let Some(i) = self.board.index(o.id) else {
            return space().into();
        };
        let item = &self.board.tasks[i];
        let mut body = column![].spacing(4).padding(8);
        match o.panel {
            Panel::Main => {
                body = body
                    .push(self.option_button(
                        if item.daily {
                            "✓ Repeat daily"
                        } else {
                            "Repeat daily"
                        },
                        0,
                    ))
                    .push(
                        self.option_button(
                            item.date
                                .as_ref()
                                .map_or_else(|| "Date…".into(), |d| display_date(d)),
                            1,
                        ),
                    )
                    .push(self.option_button("Time…", 2))
                    .push(self.option_button("Color…", 18))
                    .push(self.option_button("Add subtask", 20))
                    .push(self.option_button("Delete task", 21))
                    .push(self.option_button("Close", 22));
            }
            Panel::Date => {
                body = body.push(
                    row![
                        self.option_button("‹", 23),
                        container(text(o.calendar.format("%B %Y").to_string()).size(13))
                            .center_x(Fill),
                        self.option_button("›", 24),
                        self.option_button("Back", 22),
                    ]
                    .align_y(iced::Center),
                );
                let mut weekdays = row![].spacing(2);
                for day in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
                    weekdays = weekdays
                        .push(container(text(day).size(11).color(super::MUTED)).center_x(Fill));
                }
                body = body.push(weekdays);
                let first = o.calendar.with_day(1).unwrap();
                let offset = first.weekday().num_days_from_monday() as i32;
                for week in 0..6 {
                    let mut days = row![].spacing(2);
                    for weekday in 0..7 {
                        let day = week * 7 + weekday - offset + 1;
                        let date = (day > 0).then(|| first.with_day(day as u32)).flatten();
                        let cell: Element<'_, Message> = if let Some(date) = date {
                            let selected =
                                o.date.text().trim() == date.format("%Y-%m-%d").to_string();
                            let focused = o.focus == day as u8 + 31;
                            let today = date.format("%Y-%m-%d").to_string() == self.today;
                            container(
                                button(container(text(day.to_string()).size(12)).center_x(Fill))
                                    .height(28)
                                    .width(Fill)
                                    .padding(2)
                                    .on_press(Message::PickDate(date))
                                    .style(move |theme, status| {
                                        let mut style = focus_style(theme, status, focused);
                                        if selected {
                                            style.background =
                                                Some(Color::from_rgba8(45, 151, 148, 0.2).into());
                                        }
                                        if today || focused || selected {
                                            style.border = Border {
                                                color: if selected {
                                                    Color::from_rgb8(45, 151, 148)
                                                } else {
                                                    muted(theme)
                                                },
                                                width: 1.,
                                                radius: 5.into(),
                                            };
                                        }
                                        style
                                    }),
                            )
                            .id(format!("option-{}", day + 31))
                            .width(Fill)
                            .into()
                        } else {
                            space().width(Fill).height(28).into()
                        };
                        days = days.push(cell);
                    }
                    body = body.push(days);
                }
                body = body.push(self.option_button("Clear date", 4));
            }
            Panel::Time => {
                let value = o.time.text();
                let (h, m) = value.trim().split_once(':').unwrap_or(("09", "00"));
                body = body.push(
                    row![
                        text("Time").size(13),
                        space().width(Fill),
                        self.option_button("Back", 22)
                    ]
                    .align_y(iced::Center),
                );
                body = body.push(
                    row![
                        self.option_button(h, 25),
                        text(":").size(14),
                        self.option_button(m, 26),
                    ]
                    .spacing(8)
                    .align_y(iced::Center),
                );
                body = body.push(self.option_button("Clear time", 27));
            }
            Panel::Hours | Panel::Minutes => {
                body = body.push(self.option_button("Back", 22));
                for n in 0..if o.panel == Panel::Hours { 24 } else { 60 } {
                    body = body.push(self.option_button(format!("{n:02}"), n + 64));
                }
            }

            Panel::Color => {
                body = body.push(
                    row![
                        text("Color").size(13),
                        space().width(Fill),
                        self.option_button("Back", 22)
                    ]
                    .align_y(iced::Center),
                );
                for (n, colors) in COLORS.chunks(6).enumerate() {
                    let swatches = row(colors.iter().enumerate().map(|(j, (marker, hex))| {
                        let selected = item.color.as_deref() == Some(*marker);
                        let focus = 5 + (n * 6 + j) as u8;
                        let focused = o.focus == focus;
                        let color = hex_color(hex);
                        container(
                            button(
                                container(
                                    text(if selected { "✓" } else { "" })
                                        .size(14)
                                        .color(Color::from_rgb8(20, 20, 20)),
                                )
                                .center(26),
                            )
                            .on_press(Message::Color(Some((*marker).into())))
                            .padding(2)
                            .style(move |theme, status| {
                                let mut s = focus_style(theme, status, focused);
                                s.background = Some(color.into());
                                s.border = Border {
                                    color: if selected || focused {
                                        fg(theme)
                                    } else {
                                        Color::TRANSPARENT
                                    },
                                    width: if selected || focused { 2. } else { 0. },
                                    radius: 5.into(),
                                };
                                s
                            }),
                        )
                        .id(format!("option-{focus}"))
                        .into()
                    }))
                    .spacing(8);
                    body = body.push(swatches);
                }
                body = body.push(self.option_button("No color", 17)).push(
                    row![
                        container(self.field(Target::Color, "#rrggbb")).width(Fill),
                        self.option_button("Apply", 19)
                    ]
                    .spacing(8),
                );
            }
        }
        if let Some((error, _)) = &o.error {
            body = body.push(text(error).size(12).color(Color::from_rgb8(194, 65, 65)));
        }
        scrollable(body)
            .id(OPTIONS)
            .on_scroll(Message::OptionsScroll)
            .height(Length::Shrink)
            .into()
    }
}

fn fold_key(board: &Board, i: usize) -> String {
    let mut path = vec![board.tasks[i].text.clone()];
    let mut next = board.parent(i);
    while let Some(p) = next {
        path.push(board.tasks[p].text.clone());
        next = board.parent(p);
    }
    path.reverse();
    path.join("\u{1f}")
}
fn color_hex(value: &str) -> String {
    COLORS
        .iter()
        .find(|(marker, _)| *marker == value)
        .map_or_else(|| value.to_owned(), |(_, hex)| (*hex).to_owned())
}
fn hex_color(hex: &str) -> Color {
    u32::from_str_radix(hex.trim_start_matches('#'), 16).map_or(Color::TRANSPARENT, |v| {
        Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8)
    })
}
fn tint(theme: &Theme, color: Option<&str>) -> Color {
    let base = bg(theme);
    let Some(color) = color else { return base };
    let c = hex_color(&color_hex(color));
    let alpha = if super::is_dark(theme) { 0.2 } else { 0.11 };
    Color::from_rgb(
        base.r * (1. - alpha) + c.r * alpha,
        base.g * (1. - alpha) + c.g * alpha,
        base.b * (1. - alpha) + c.b * alpha,
    )
}
fn display_date(value: &str) -> String {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M")
        .map(|d| d.format("%a %-d %b, %H:%M").to_string())
        .or_else(|_| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d").map(|d| d.format("%a %-d %b").to_string())
        })
        .unwrap_or_else(|_| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtask_is_first_and_inherits_the_parent_color_including_none() {
        let mut s = State::new(
            "- [ ] ==🟢First==\n- [ ] Parent\n  - [ ] Older child\n",
            &[],
        );
        s.active = Target::Item(2);
        let _ = s.update(Message::New(true));
        assert_eq!(s.board.tasks[2].text, "");
        assert_eq!(s.board.tasks[2].depth, 1);
        assert_eq!(s.board.tasks[2].color, None);
        assert_eq!(s.board.tasks[3].text, "Older child");
    }

    #[test]
    fn unicode_split_uses_the_selected_byte_range_and_keeps_children() {
        let mut s = State::new("- [ ] سلام world\n  - [ ] Child\n- [ ] Last\n", &[]);
        s.editors.get_mut(&1).unwrap().move_to(text_editor::Cursor {
            position: text_editor::Position { line: 0, column: 9 },
            selection: None,
        });
        let _ = s.update(Message::Split(1));
        assert_eq!(
            s.board
                .tasks
                .iter()
                .map(|t| (&*t.text, t.depth))
                .collect::<Vec<_>>(),
            [("سلام ", 0), ("Child", 1), ("world", 0), ("Last", 0)]
        );
    }

    #[test]
    fn completion_snapshot_restores_branch_order_and_editor_selection() {
        let mut s = State::new(
            "- [ ] Parent\n  - [ ] Child\n- [x] Done\n- [ ] Other\n",
            &[],
        );
        s.editors
            .get_mut(&1)
            .unwrap()
            .perform(text_editor::Action::SelectAll);
        s.folded.insert(1);
        let before = s.snapshot();
        let original = s.markdown();
        let _ = s.update(Message::Toggle(1));
        assert_eq!(s.board.tasks.last().unwrap().text, "Child");
        let _ = s.restore(before);
        assert_eq!(s.markdown(), original);
        assert_eq!(s.selection().as_deref(), Some("Parent"));
        assert!(s.folded.contains(&1));
    }

    #[test]
    fn calendar_navigation_does_not_save_until_a_day_is_picked() {
        let mut s = State::new("- [ ] First 📅 2028-01-31 09:30\n", &[]);
        let _ = s.update(Message::Options(1));
        let _ = s.update(Message::Panel(Panel::Date));
        let original = s.markdown();
        let _ = s.update(Message::Month(true));
        assert_eq!(
            s.options.as_ref().unwrap().calendar.to_string(),
            "2028-02-29"
        );
        assert_eq!(s.markdown(), original);
        let _ = s.update(Message::Activate);
        assert_eq!(s.board.tasks[0].date.as_deref(), Some("2028-02-29 09:30"));
        assert_eq!(s.options.as_ref().unwrap().panel, Panel::Main);
        let _ = s.update(Message::Panel(Panel::Date));
        let _ = s.update(Message::CalendarMove(1));
        assert_eq!(
            s.options.as_ref().unwrap().calendar.to_string(),
            "2028-03-01"
        );
        let _ = s.update(Message::CloseOptions);
        assert_eq!(s.board.tasks[0].date.as_deref(), Some("2028-02-29 09:30"));
    }

    #[test]
    fn time_dropdown_preserves_date_and_an_undated_task_gets_today() {
        let mut s = State::new("- [ ] First\n", &[]);
        let _ = s.update(Message::Options(1));
        let _ = s.update(Message::TimePart(true, "14".into()));
        assert_eq!(
            s.board.tasks[0].date.as_deref(),
            Some(format!("{} 14:00", s.today).as_str())
        );
        let _ = s.update(Message::TimePart(false, "35".into()));
        assert_eq!(
            s.board.tasks[0].date.as_deref(),
            Some(format!("{} 14:35", s.today).as_str())
        );
        let _ = s.update(Message::ClearDate);
        assert!(s.board.tasks[0].date.is_none());
    }

    #[test]
    fn date_and_custom_color_drafts_validate_before_changing_the_board() {
        let mut s = State::new("- [ ] First\n", &[]);
        let _ = s.update(Message::Options(1));
        s.options.as_mut().unwrap().date = text_editor::Content::with_text("2026-02-29");
        let _ = s.update(Message::ApplyDate);
        assert!(s.board.tasks[0].date.is_none());
        s.options.as_mut().unwrap().date = text_editor::Content::with_text("2028-02-29");
        s.options.as_mut().unwrap().time = text_editor::Content::with_text("09:30");
        let _ = s.update(Message::ApplyDate);
        assert_eq!(s.board.tasks[0].date.as_deref(), Some("2028-02-29 09:30"));
        s.options.as_mut().unwrap().color = text_editor::Content::with_text("#eC4899");
        let _ = s.update(Message::ApplyColor);
        assert_eq!(s.board.tasks[0].color.as_deref(), Some("#ec4899"));
    }

    #[test]
    fn drag_requires_a_grip_and_cancels_outside_or_on_a_descendant() {
        let mut s = State::new("- [ ] Parent\n  - [ ] Child\n- [ ] Last\n", &[]);
        let original = s.markdown();
        let _ = s.update(Message::Over(3, true));
        let _ = s.update(Message::Drop);
        assert_eq!(s.markdown(), original);
        let _ = s.update(Message::Grip(1));
        let _ = s.update(Message::Over(2, false));
        let _ = s.update(Message::Drop);
        assert_eq!(s.markdown(), original);
        let _ = s.update(Message::Grip(1));
        let _ = s.update(Message::Over(3, true));
        let _ = s.update(Message::CancelDrag);
        let _ = s.update(Message::Drop);
        assert_eq!(s.markdown(), original);
        let _ = s.update(Message::Grip(1));
        let _ = s.update(Message::Over(3, true));
        let _ = s.update(Message::Drop);
        assert_eq!(s.board.tasks[0].text, "Last");
        assert_eq!(s.board.tasks[2].text, "Child");
    }

    #[test]
    fn renaming_a_folded_parent_preserves_its_fold_after_restart() {
        let mut s = State::new("- [ ] Parent\n  - [ ] Child\n", &["Parent".into()]);
        s.edit(Target::Item(1), text_editor::Action::SelectAll);
        s.edit(
            Target::Item(1),
            text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new("Renamed".into()))),
        );
        let reloaded = State::new(&s.markdown(), &s.folded_keys());
        assert!(reloaded.folded.contains(&1));
    }
}
