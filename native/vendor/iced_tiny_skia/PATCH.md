# Local renderer fix

This is the published `iced_tiny_skia` 0.14.1 source, with its MIT license.
Only `src/engine.rs`, `src/layer.rs` and `src/window/compositor.rs` differ from
the published Rust source.

Applied upstream fix: [Always clip `Text::Editor`](https://github.com/iced-rs/iced/commit/23170119b048584a3cc659dd3b04d83bffd87c14).
It always masks editor glyphs to their viewport. Skipping the mask allows
partial lines to paint into the padding, beyond the region cleared on the
next redraw, leaving duplicated text after scrolling or editing.

The published 0.14.1 renderer also lacks this fix. Keep this patch until a
compatible renderer release includes it.

Quad damage comparison trims the shared prefix and suffix before comparing
the changed span. Inserting or removing a caret otherwise shifts every later
quad's index and repaints the remaining task cards on every blink. The
renderer regression checks in `../../tests/renderer_damage.rs` cover caret
insertion/removal, task color changes and resizing. Editor viewport masking
remains enabled.

Presentation sends the damaged rectangles to softbuffer instead of copying
the whole window to X11 after each small change. Rectangle edges round
outward at the physical render scale and stay inside the buffer. Empty
damage sends no image region; a new or resized buffer still presents in
full. Native X11 exposure notifications request one full presentation from
the retained client buffer after restoring or uncovering a window; until
the app registers that handler, full presentation remains the default.
Added and removed layers damage their content bounds instead of the entire
viewport, so a tooltip stays a small update. `../../test/presentation.py`
checks actual XDamage events for caret,
typing and hover updates at 1×, 1.25× and 2×, plus minimize/restore and resize.
