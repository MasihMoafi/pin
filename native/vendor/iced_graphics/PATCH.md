# Preserve unchanged editor layout

This is the published `iced_graphics` 0.14.0 source, under Iced's MIT license.
The crate archive matches its original lockfile checksum. Only
`src/text/editor.rs` differs from the published Rust source. The application
uses the root native lockfile instead of the library's bundled lockfile.

`Editor::update` previously rebuilt its internal Arc on every layout call,
even when all layout inputs were unchanged. This expired the previous
frame's weak references, so editing or hovering one task invalidated every
visible task editor.

The update now returns early when the text/interaction state, bounds, font,
metrics, wrapping and font-system version are unchanged. Editing and cursor
operations mark layout dirty; changes to layout inputs still shape normally.
The renderer checks in `../../tests/renderer_damage.rs` verify unchanged
references, edits, and wrapping after a resize. Real-window checks cover
selection, history, external reload and Markdown highlighting.
