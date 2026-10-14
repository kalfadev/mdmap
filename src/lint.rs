//! `mdmap lint`: the tree reads, but something is broken: a link, an id in
//! inline code, a verify comment with nothing to compare, or an `ignore`
//! pattern that hides nothing.
//! It reports; it never fixes.

use crate::out::Output;
use serde_json::json;

use crate::graph::{Act, Graph, LinkTarget};
use crate::id::is_id;
use crate::markdown::CodeKind;

pub struct Finding {
    pub at: String,
    pub message: String,
}

pub fn findings(g: &Graph) -> Vec<Finding> {
    let mut out: Vec<(String, usize, Finding)> = Vec::new();
    for p in g.unused_ignores() {
        let message = "matches no path under the roots, so it hides nothing".to_string();
        out.push((String::new(), 0, Finding { at: format!("ignore {p}"), message }));
    }
    for at in g.unused_denies() {
        let message = "names no node, so it closes nothing".to_string();
        out.push((String::new(), 0, Finding { at, message }));
    }
    for l in &g.resolved {
        let message = match &l.target {
            LinkTarget::Broken(why) => format!("broken link → {} ({why})", l.dest),
            LinkTarget::Stale(t) => {
                let n = &g.nodes[*t];
                let place = if n.file.is_empty() { g.location(*t) } else { n.file.clone() };
                format!("stale path → {} (node is in {place})", l.dest)
            }
            _ => continue,
        };
        out.push((l.file.clone(), l.line, Finding { at: format!("{}:{}", l.file, l.line), message }));
    }

    for (file, doc) in &g.docs {
        for v in &doc.verifies {
            if crate::verify::span_after(doc, v.end).is_none() {
                out.push((
                    file.clone(),
                    v.line,
                    Finding { at: format!("{file}:{}", v.line), message: "verify without a span".into() },
                ));
            }
        }
        // An id in inline code names a heading; a bold title right before it names the same heading.
        for c in doc.code.iter().filter(|c| c.kind == CodeKind::Inline) {
            let Some(id) = c.content.strip_prefix("{").and_then(|s| s.strip_suffix("}")).filter(|s| is_id(s)) else { continue };
            let line = doc.line_of(c.range.start);
            let message = match g.node(id) {
                None => format!("broken id → {{{id}}} (no such id)"),
                Some(t) if !g.denied(t, Act::Read) => {
                    let title = g.nodes[t].title.as_str();
                    match bold_before(&doc.text[doc.line_starts[line - 1]..c.range.start]) {
                        Some(b) if b != title && !b.ends_with(&format!(": {title}")) => {
                            format!("stale title → **{b}** {{{id}}} (the heading is {title})")
                        }
                        _ => continue,
                    }
                }
                Some(_) => continue,
            };
            out.push((file.clone(), line, Finding { at: format!("{file}:{line}"), message }));
        }
    }
    out.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    out.into_iter().map(|x| x.2).collect()
}

/// The bold text that ends right before an inline code span, as in **Title** followed by its id;
/// it may lead with the titles above the heading, **File: Title**.
fn bold_before(before: &str) -> Option<&str> {
    let inner = before.trim_end().strip_suffix("**")?;
    Some(&inner[inner.rfind("**")? + 2..])
}

pub fn lint(g: &Graph, json_out: bool) -> Output {
    let f = findings(g);
    let mut out = if json_out {
        Output::text(
            serde_json::to_string_pretty(&json!({
                "findings": f.iter().map(|x| json!({"at": x.at, "finding": x.message})).collect::<Vec<_>>()
            }))
            .unwrap()
                + "\n",
        )
    } else if f.is_empty() {
        Output::text("no lint findings\n")
    } else {
        let w = f.iter().map(|x| x.at.chars().count()).max().unwrap_or(0);
        Output::text(f.iter().map(|x| format!("{:<w$}   {}\n", x.at, x.message)).collect::<String>())
    };
    out.code = if f.is_empty() { 0 } else { 1 };
    out.touched = g.docs.keys().cloned().collect();
    out
}
