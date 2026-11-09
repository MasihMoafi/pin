//! Scroll keyboard targets into view using their actual laid-out bounds.
use super::pin::Message;
use iced::advanced::widget::{
    operation::{Focusable, Operation, Outcome, Scrollable},
    Id,
};
use iced::{Rectangle, Vector};

pub struct Reveal {
    scroll: &'static str,
    scroll_id: Id,
    target_id: Id,
    target: Option<Rectangle>,
    viewport: Option<(Rectangle, Vector)>,
}
impl Reveal {
    pub fn new(scroll: &'static str, target: String) -> Self {
        Self {
            scroll,
            scroll_id: Id::new(scroll),
            target_id: Id::new(target),
            target: None,
            viewport: None,
        }
    }
}
impl Operation<Message> for Reveal {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Message>)) {
        operate(self);
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if id == Some(&self.target_id) {
            self.target = Some(bounds);
        }
    }
    fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, _: &mut dyn Focusable) {
        if id == Some(&self.target_id) {
            self.target = Some(bounds);
        }
    }
    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _: Rectangle,
        translation: Vector,
        _: &mut dyn Scrollable,
    ) {
        if id == Some(&self.scroll_id) {
            self.viewport = Some((bounds, translation));
        }
    }
    fn finish(&self) -> Outcome<Message> {
        let (Some(target), Some((view, offset))) = (self.target, self.viewport) else {
            return Outcome::None;
        };
        let top = target.y - offset.y;
        let bottom = top + target.height;
        let next = if top < view.y + 6. {
            (target.y - view.y - 6.).max(0.)
        } else if bottom > view.y + view.height - 6. {
            (target.y + target.height - view.y - view.height + 6.).max(0.)
        } else {
            return Outcome::None;
        };
        if (next - offset.y).abs() < 1. {
            Outcome::None
        } else {
            Outcome::Some(Message::ScrollTo(self.scroll, next))
        }
    }
}
