#![cfg(feature = "iced-ui")]

use iced::advanced::renderer::Quad;
use iced::advanced::text::editor::{Action, Edit, Editor as _, Motion};
use iced::advanced::text::highlighter::PlainText;
use iced::advanced::text::{LineHeight, Wrapping};
use iced::{Background, Color, Font, Pixels, Rectangle, Size};
use iced_tiny_skia::graphics::text::Editor;
use iced_tiny_skia::Layer;

fn layout_editor(editor: &mut Editor, bounds: Size) {
    editor.update(
        bounds,
        Font::default(),
        Pixels(15.),
        LineHeight::Absolute(Pixels(24.5)),
        Wrapping::WordOrGlyph,
        &mut PlainText,
    );
}

#[test]
fn unchanged_editor_layout_keeps_its_render_reference() {
    let mut editor = Editor::with_text("An unchanged task");
    let bounds = Size::new(300., 80.);
    layout_editor(&mut editor, bounds);
    let previous = editor.downgrade();
    layout_editor(&mut editor, bounds);
    assert!(previous.upgrade().is_some());
    assert_eq!(previous, editor.downgrade());
}

#[test]
fn editing_still_invalidates_and_lays_out_the_changed_editor() {
    let mut editor = Editor::with_text("alpha");
    let bounds = Size::new(300., 200.);
    layout_editor(&mut editor, bounds);
    let old_height = editor.min_bounds().height;
    let previous = editor.downgrade();
    editor.perform(Action::Move(Motion::DocumentEnd));
    editor.perform(Action::Edit(Edit::Enter));
    editor.perform(Action::Edit(Edit::Insert('B')));
    layout_editor(&mut editor, bounds);
    assert!(previous.upgrade().is_none());
    assert_eq!(editor.line(1).unwrap().text, "B");
    assert!(editor.min_bounds().height > old_height);
}

#[test]
fn resizing_still_rewraps_unchanged_editor_text() {
    let mut editor =
        Editor::with_text("A task with enough words to wrap when its window becomes narrow");
    layout_editor(&mut editor, Size::new(600., 300.));
    let previous = editor.downgrade();
    let old_height = editor.min_bounds().height;
    layout_editor(&mut editor, Size::new(100., 300.));
    assert!(previous.upgrade().is_none());
    assert!(editor.min_bounds().height > old_height);
}

fn scene() -> Layer {
    Layer {
        bounds: Rectangle::new(iced::Point::ORIGIN, iced::Size::new(1213., 884.)),
        quads: (0..3)
            .map(|i| {
                (
                    Quad {
                        bounds: Rectangle {
                            x: 10.,
                            y: 50. * i as f32,
                            width: 1180.,
                            height: 40.,
                        },
                        ..Quad::default()
                    },
                    Background::Color(Color::from_rgb(0.8, 0.9, 0.7)),
                )
            })
            .collect(),
        primitives: vec![],
        images: vec![],
        text: vec![],
    }
}

#[test]
fn blinking_caret_damages_only_its_own_rectangle() {
    let before = scene();
    let mut after = before.clone();
    let caret = Quad {
        bounds: Rectangle {
            x: 30.,
            y: 10.,
            width: 1.,
            height: 24.,
        },
        ..Quad::default()
    };
    after.quads.insert(1, (caret, Color::BLACK.into()));
    let expected = vec![caret.bounds.expand(1.)];
    assert_eq!(Layer::damage(&before, &after), expected);
    assert_eq!(Layer::damage(&after, &before), expected);
}

#[test]
fn changing_a_task_color_still_invalidates_that_task() {
    let before = scene();
    let mut after = before.clone();
    after.quads[1].1 = Color::BLACK.into();
    let damage = Layer::damage(&before, &after);
    assert!(!damage.is_empty());
    assert!(damage
        .iter()
        .all(|r| *r == before.quads[1].0.bounds.expand(1.)));
}

#[test]
fn resizing_still_invalidates_both_window_bounds() {
    let before = scene();
    let mut after = before.clone();
    after.bounds.width = 600.;
    assert_eq!(
        Layer::damage(&before, &after),
        vec![before.bounds, after.bounds]
    );
}
