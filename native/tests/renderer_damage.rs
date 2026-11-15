#![cfg(feature = "iced-ui")]

use iced::advanced::renderer::Quad;
use iced::{Background, Color, Rectangle};
use iced_tiny_skia::Layer;

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
