//! Apply CommonMark emphasis to the editable source without changing its bytes.
use iced::advanced::text::highlighter::{Format, Highlighter};
use iced::{font, Font, Theme};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    bold: bool,
    italic: bool,
    code: bool,
}

pub struct Markdown {
    lines: Vec<Vec<(Range<usize>, Style)>>,
    current: usize,
}

impl Highlighter for Markdown {
    type Settings = String;
    type Highlight = Style;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Style)>;

    fn new(source: &String) -> Self {
        let mut offsets = vec![0];
        offsets.extend(source.match_indices('\n').map(|(i, _)| i + 1));
        let mut lines = vec![vec![]; offsets.len()];
        let (mut bold, mut italic, mut code) = (0, 0, false);
        for (event, range) in Parser::new(source).into_offset_iter() {
            match event {
                Event::Start(Tag::Strong | Tag::Heading { .. }) => bold += 1,
                Event::Start(Tag::Emphasis) => italic += 1,
                Event::Start(Tag::CodeBlock(_)) => code = true,
                Event::End(TagEnd::Strong | TagEnd::Heading(_)) => bold -= 1,
                Event::End(TagEnd::Emphasis) => italic -= 1,
                Event::End(TagEnd::CodeBlock) => code = false,
                Event::Text(_) | Event::Code(_) => {
                    let style = Style {
                        bold: bold > 0,
                        italic: italic > 0,
                        code: code || matches!(event, Event::Code(_)),
                    };
                    if style == Style::default() {
                        continue;
                    }
                    let mut line = offsets.partition_point(|&start| start <= range.start) - 1;
                    while line < offsets.len() && offsets[line] < range.end {
                        let start = offsets[line];
                        let end = offsets.get(line + 1).copied().unwrap_or(source.len());
                        lines[line].push((
                            range.start.max(start) - start..range.end.min(end) - start,
                            style,
                        ));
                        line += 1;
                    }
                }
                _ => (),
            }
        }
        Self { lines, current: 0 }
    }

    fn update(&mut self, source: &String) {
        *self = Self::new(source);
    }

    fn change_line(&mut self, line: usize) {
        self.current = self.current.min(line);
    }

    fn highlight_line(&mut self, _: &str) -> Self::Iterator<'_> {
        let spans = self.lines.get(self.current).cloned().unwrap_or_default();
        #[cfg(debug_assertions)]
        eprintln!("Markdown line {}: {:?}", self.current, spans);
        self.current += 1;
        spans.into_iter()
    }

    fn current_line(&self) -> usize {
        self.current
    }
}

pub fn format(style: &Style, _: &Theme) -> Format<Font> {
    #[cfg(debug_assertions)]
    eprintln!("Markdown format {style:?}");
    Format {
        color: None,
        font: Some(Font {
            weight: if style.bold {
                font::Weight::Bold
            } else {
                font::Weight::Normal
            },
            style: if style.italic {
                font::Style::Italic
            } else {
                font::Style::Normal
            },
            ..if style.code {
                Font::MONOSPACE
            } else {
                Font::with_name("Inter")
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styles(source: &str, needle: &str) -> Vec<Style> {
        let markdown = Markdown::new(&source.to_owned());
        let at = source.find(needle).unwrap();
        let line = source[..at].bytes().filter(|&b| b == b'\n').count();
        let column = at - source[..at].rfind('\n').map_or(0, |i| i + 1);
        markdown.lines[line]
            .iter()
            .filter(|(range, _)| range.contains(&column))
            .map(|(_, style)| *style)
            .collect()
    }

    #[test]
    fn nested_emphasis_and_unicode_use_the_right_byte_ranges() {
        let source = "سلام **bold *italic* end** and _emphasis_\n***هر دو***";
        assert!(styles(source, "bold")[0].bold);
        let nested = styles(source, "italic")[0];
        assert!(nested.bold && nested.italic);
        assert!(styles(source, "emphasis")[0].italic);
        let persian = styles(source, "هر دو")[0];
        assert!(persian.bold && persian.italic);
    }

    #[test]
    fn literal_markers_and_code_are_not_emphasis() {
        let source =
            "\\*literal\\* and unmatched * and snake_case_name\n`**inline**`\n\n```\n*block*\n```";
        for plain in ["literal", "unmatched", "snake_case_name"] {
            assert!(styles(source, plain).is_empty());
        }
        for code in ["inline", "block"] {
            let style = styles(source, code)[0];
            assert!(style.code && !style.bold && !style.italic);
        }
    }

    #[test]
    fn multiline_emphasis_and_removing_a_delimiter_refresh_later_lines() {
        let source = "**first\nsecond**\nplain";
        assert!(styles(source, "second")[0].bold);
        assert!(styles(source, "plain").is_empty());
        let mut markdown = Markdown::new(&source.to_owned());
        assert!(!markdown
            .highlight_line("**first")
            .collect::<Vec<_>>()
            .is_empty());
        markdown.update(&"*first\nsecond**\nplain".to_owned());
        markdown.change_line(1);
        assert_eq!(markdown.current_line(), 0);
        assert!(markdown
            .highlight_line("*first")
            .all(|(_, style)| !style.bold));
    }
}
