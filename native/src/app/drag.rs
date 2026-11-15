//! Track drop targets and redraw their indicator without rebuilding task editors.
use iced::advanced::{layout, mouse, overlay, renderer, widget, Clipboard, Layout, Shell, Widget};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Copy, Default)]
struct Position {
    source: Option<u64>,
    target: Option<(u64, bool)>,
}

#[derive(Clone, Default)]
pub struct Drag(Rc<Cell<Position>>);

impl Drag {
    pub fn source(&self) -> Option<u64> {
        self.0.get().source
    }
    pub fn target(&self) -> Option<(u64, bool)> {
        self.0.get().target
    }
    pub fn start(&self, source: u64) {
        self.0.set(Position {
            source: Some(source),
            target: None,
        });
    }
    pub fn over(&self, target: u64, after: bool) {
        self.0.set(Position {
            target: Some((target, after)),
            ..self.0.get()
        });
    }
    fn leave(&self, target: u64, after: bool) {
        if self.target() == Some((target, after)) {
            self.0.set(Position {
                target: None,
                ..self.0.get()
            });
        }
    }
    pub fn take(&self) -> (Option<u64>, Option<(u64, bool)>) {
        let value = self.0.replace(Position::default());
        (value.source, value.target)
    }
    pub fn cancel(&self) {
        self.0.set(Position::default());
    }
}

pub fn target<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    drag: Drag,
    id: u64,
    after: bool,
    enabled: bool,
) -> Element<'a, Message> {
    Element::new(Target {
        content: content.into(),
        drag,
        id,
        after,
        enabled,
    })
}

struct Target<'a, Message> {
    content: Element<'a, Message>,
    drag: Drag,
    id: u64,
    after: bool,
    enabled: bool,
}

impl<Message> Widget<Message, Theme, Renderer> for Target<'_, Message> {
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if self.drag.source().is_some()
            && matches!(event, Event::Mouse(mouse::Event::CursorMoved { .. }))
        {
            let before = self.drag.target();
            if self.enabled && cursor.is_over(layout.bounds()) && cursor.is_over(*viewport) {
                self.drag.over(self.id, self.after);
            } else {
                self.drag.leave(self.id, self.after);
            }
            if before != self.drag.target() {
                shell.request_redraw();
            }
        }
    }
    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
