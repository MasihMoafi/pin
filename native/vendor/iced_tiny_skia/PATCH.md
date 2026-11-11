# Local renderer fix

This is the published `iced_tiny_skia` 0.14.1 source, with its MIT license.
Only `src/engine.rs` differs from the published Rust source.

Applied upstream fix: [Always clip `Text::Editor`](https://github.com/iced-rs/iced/commit/23170119b048584a3cc659dd3b04d83bffd87c14).
It always masks editor glyphs to their viewport. Skipping the mask allows
partial lines to paint into the padding, beyond the region cleared on the
next redraw, leaving duplicated text after scrolling or editing.

The published 0.14.1 renderer also lacks this fix. Keep this patch until a
compatible renderer release includes it.
