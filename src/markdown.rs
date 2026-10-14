//! One markdown file, parsed into the pieces mdmap needs: headings, links,
//! paragraphs, code spans and `verify` comments, all with byte offsets.
//!
//! Anything inside a code fence or an inline code span is not structure:
//! pulldown-cmark already reports those as code, so a `## x` line or a
//! `[x](#id)` link inside them never shows up here.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::id;
use crate::out::truncate;

#[derive(Debug, Clone)]
pub struct Heading {
    pub level: u8,
    pub title: String,
    pub id: Option<String>,
    /// Underlined with `===` or `---` instead of `#`.
    pub setext: bool,
    /// Byte offset of the first heading line.
    pub start: usize,
    /// Byte offset just past the heading (after its last newline).
    pub end: usize,
    /// 1-based line of the heading.
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub dest: String,
    pub start: usize,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeKind {
    Inline,
    Fenced,
    Indented,
}

#[derive(Debug, Clone)]
pub struct CodeSpan {
    pub kind: CodeKind,
    /// The whole span: backticks, or fence lines included.
    pub range: Range<usize>,
    /// The content as markdown reads it.
    pub content: String,
    /// For a fenced block, the bytes between the fence lines.
    pub inner: Range<usize>,
    pub closed: bool,
}

#[derive(Debug, Clone)]
pub struct VerifyComment {
    pub cmd: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Doc {
    pub text: String,
    pub line_starts: Vec<usize>,
    /// Where the markdown starts, after frontmatter.
    pub content_start: usize,
    /// Top-level headings, in document order.
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
    /// Top-level paragraphs.
    pub paragraphs: Vec<Range<usize>>,
    pub code: Vec<CodeSpan>,
    pub verifies: Vec<VerifyComment>,
    /// The file ends inside a code fence that never closes.
    pub open_fence_at_eof: bool,
}

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
}

impl Doc {
    pub fn parse(text: impl Into<String>) -> Doc {
        let text: String = text.into();
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        let mut doc = Doc {
            text: String::new(),
            line_starts,
            content_start: 0,
            headings: Vec::new(),
            links: Vec::new(),
            paragraphs: Vec::new(),
            code: Vec::new(),
            verifies: Vec::new(),
            open_fence_at_eof: false,
        };

        let mut depth = 0usize;
        let mut html_ranges: Vec<Range<usize>> = Vec::new();
        let mut in_block: Option<(CodeKind, Range<usize>, String)> = None;

        for (event, range) in Parser::new_ext(&text, options()).into_offset_iter() {
            if let Some((_, _, content)) = in_block.as_mut() {
                match &event {
                    Event::Text(t) => {
                        content.push_str(t);
                        continue;
                    }
                    Event::End(TagEnd::CodeBlock) => {}
                    _ => continue,
                }
            }
            match event {
                Event::Start(Tag::MetadataBlock(_)) => {
                    doc.content_start = line_end_after(&text, range.end.saturating_sub(1));
                }
                Event::Start(
                    Tag::BlockQuote(_)
                    | Tag::List(_)
                    | Tag::Item
                    | Tag::FootnoteDefinition(_)
                    | Tag::Table(_),
                ) => depth += 1,
                Event::End(
                    TagEnd::BlockQuote(_)
                    | TagEnd::List(_)
                    | TagEnd::Item
                    | TagEnd::FootnoteDefinition
                    | TagEnd::Table,
                ) => depth = depth.saturating_sub(1),
                Event::Start(Tag::Heading { level, .. }) if depth == 0 => {
                    let start = doc_line_start(&doc.line_starts, range.start);
                    let end = line_end_after(&text, range.end.saturating_sub(1).max(range.start));
                    let raw = &text[start..end];
                    let first = raw.trim_start();
                    let setext = !first.starts_with('#');
                    let (title, hid) = if setext {
                        let lines: Vec<&str> = raw.lines().collect();
                        let body = lines[..lines.len().saturating_sub(1).max(1)]
                            .iter()
                            .map(|l| l.trim())
                            .collect::<Vec<_>>()
                            .join(" ");
                        split_id(&body)
                    } else {
                        split_id(&atx_text(raw.lines().next().unwrap_or("")))
                    };
                    doc.headings.push(Heading {
                        level: level as u8,
                        title,
                        id: hid,
                        setext,
                        start,
                        end,
                        line: line_of(&doc.line_starts, start),
                    });
                }
                Event::Start(Tag::Paragraph) if depth == 0 => doc.paragraphs.push(range),
                Event::Start(Tag::Link { dest_url, .. }) => doc.links.push(Link {
                    dest: dest_url.to_string(),
                    start: range.start,
                    line: line_of(&doc.line_starts, range.start),
                }),
                Event::Code(content) => {
                    let raw = &text[range.clone()];
                    let ticks = raw.bytes().take_while(|&b| b == b'`').count();
                    let inner = (range.start + ticks)..(range.end - ticks).max(range.start + ticks);
                    doc.code.push(CodeSpan {
                        kind: CodeKind::Inline,
                        range: range.clone(),
                        content: content.to_string(),
                        inner,
                        closed: true,
                    });
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    let kind = match kind {
                        CodeBlockKind::Fenced(_) => CodeKind::Fenced,
                        CodeBlockKind::Indented => CodeKind::Indented,
                    };
                    in_block = Some((kind, range, String::new()));
                }
                Event::End(TagEnd::CodeBlock) => {
                    if let Some((kind, range, content)) = in_block.take() {
                        let (inner, closed) = if kind == CodeKind::Fenced {
                            fence_inner(&text, range.clone())
                        } else {
                            (range.clone(), true)
                        };
                        if kind == CodeKind::Fenced && !closed && depth == 0 {
                            doc.open_fence_at_eof = true;
                        }
                        doc.code.push(CodeSpan { kind, range, content, inner, closed });
                    }
                }
                Event::Start(Tag::HtmlBlock) => html_ranges.push(range),
                Event::InlineHtml(_) => html_ranges.push(range),
                _ => {}
            }
        }

        for r in html_ranges {
            let chunk = &text[r.clone()];
            let mut from = 0;
            while let Some(open) = chunk[from..].find("<!--") {
                let open = from + open;
                let Some(close) = chunk[open..].find("-->") else { break };
                let close = open + close + 3;
                let inside = chunk[open + 4..close - 3].trim();
                if let Some(cmd) = inside.strip_prefix("verify:") {
                    let start = r.start + open;
                    doc.verifies.push(VerifyComment {
                        cmd: cmd.trim().to_string(),
                        start,
                        end: r.start + close,
                        line: line_of(&doc.line_starts, start),
                    });
                }
                from = close;
            }
        }
        doc.code.sort_by_key(|c| c.range.start);
        doc.verifies.sort_by_key(|v| v.start);
        doc.text = text;
        doc
    }

    pub fn line_of(&self, offset: usize) -> usize {
        line_of(&self.line_starts, offset)
    }

    /// Byte offset of the start of a 1-based line; one past the last line
    /// gives the end of the text.
    pub fn line_start(&self, line: usize) -> usize {
        if line == 0 {
            return 0;
        }
        self.line_starts.get(line - 1).copied().unwrap_or(self.text.len())
    }

    /// The body of heading `i`: everything up to the next heading.
    pub fn body_of(&self, i: usize) -> Range<usize> {
        let start = self.headings[i].end;
        let end = self.headings.get(i + 1).map(|h| h.start).unwrap_or(self.text.len());
        start..end.max(start)
    }

    /// First and last line of a body range, or None when it is empty.
    pub fn lines_of(&self, r: &Range<usize>) -> Option<(usize, usize)> {
        if r.start >= r.end {
            return None;
        }
        Some((self.line_of(r.start), self.line_of(r.end - 1)))
    }

    /// The first paragraph inside `r`, one line, at most 280 characters.
    pub fn summary_in(&self, r: &Range<usize>) -> Option<String> {
        let p = self.paragraphs.iter().find(|p| p.start >= r.start && p.start < r.end)?;
        let joined = self.text[p.clone()]
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        Some(truncate(&joined, 280))
    }
}


fn line_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|&s| s <= offset).max(1)
}

fn doc_line_start(starts: &[usize], offset: usize) -> usize {
    starts[line_of(starts, offset) - 1]
}

/// Offset just past the newline of the line containing `offset`.
fn line_end_after(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    match text[offset..].find('\n') {
        Some(i) => offset + i + 1,
        None => text.len(),
    }
}

/// The text of an ATX heading line, without the marker and closing hashes.
fn atx_text(line: &str) -> String {
    let t = line.trim().trim_start_matches('#');
    let t = t.trim();
    let stripped = t.trim_end_matches('#');
    if stripped.len() != t.len() && (stripped.is_empty() || stripped.ends_with([' ', '\t'])) {
        return stripped.trim().to_string();
    }
    t.to_string()
}

/// Split `Title {a3f9c1}` into its title and id.
pub fn split_id(text: &str) -> (String, Option<String>) {
    let t = text.trim();
    if let Some(open) = t.rfind('{') {
        if t.ends_with('}') {
            let inside = &t[open + 1..t.len() - 1];
            if id::is_id(inside) {
                return (t[..open].trim().to_string(), Some(inside.to_string()));
            }
        }
    }
    (t.to_string(), None)
}

/// For a fenced block: the bytes between the fence lines, and whether a
/// closing fence exists.
fn fence_inner(text: &str, r: Range<usize>) -> (Range<usize>, bool) {
    let raw = &text[r.clone()];
    let first_end = raw.find('\n').map(|i| i + 1).unwrap_or(raw.len());
    let opening = raw[..first_end].trim();
    let fc = opening.chars().next().unwrap_or('`');
    let n = opening.chars().take_while(|&c| c == fc).count();
    let body = raw.trim_end_matches(['\n', '\r']);
    let last_start = body.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let last = body[last_start..].trim();
    let closed = last_start >= first_end
        && !last.is_empty()
        && last.chars().all(|c| c == fc)
        && last.chars().count() >= n;
    if closed {
        (r.start + first_end..r.start + last_start, true)
    } else {
        (r.start + first_end.min(raw.len())..r.end, false)
    }
}

/// Top-level headings in a standalone piece of markdown, e.g. a body.
pub fn has_heading(text: &str) -> bool {
    !Doc::parse(text).headings.is_empty()
}

/// Render a heading line.
pub fn heading_line(level: u8, title: &str, id: Option<&str>) -> String {
    let mut s = "#".repeat(level as usize);
    s.push(' ');
    s.push_str(title);
    if let Some(id) = id {
        s.push_str(" {");
        s.push_str(id);
        s.push('}');
    }
    s
}
