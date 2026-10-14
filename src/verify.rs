//! `mdmap verify`: a sentence tied to the command that would prove it
//! wrong.
//!
//! `<!-- verify: <command> -->` in a body, then the first code span after
//! it, inline or fenced, within the same body. The stdout of the command is
//! compared with the content of the span. The line must start with the words
//! of an entry of `verify.run`, and what runs is the command of that entry,
//! with the other words of the line where `{args}` is.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::json;
use similar::{ChangeTag, TextDiff};

use crate::config;
use crate::exec::{allowed, exec, refusal_name, split, with_args};
use crate::graph::Graph;
use crate::markdown::{CodeKind, CodeSpan, Doc};
use crate::out::{atomic_write, Output, Refusal, R};

/// The span a `verify` comment ending at `end` is compared with.
pub fn span_after(doc: &Doc, end: usize) -> Option<&CodeSpan> {
    let limit = doc.headings.iter().find(|h| h.start >= end).map(|h| h.start).unwrap_or(doc.text.len());
    doc.code.iter().find(|c| c.range.start >= end && c.range.start < limit)
}

fn normalize(s: &str, inline: bool) -> String {
    if inline {
        s.trim().to_string()
    } else {
        s.trim_end_matches(['\n', '\r', ' ', '\t']).to_string()
    }
}

#[derive(Debug)]
struct Check {
    node: usize,
    status: String,
    detail: Vec<String>,
    failed: bool,
}

/// New bytes for a span, given the command's output.
fn replacement(doc: &Doc, span: &CodeSpan, output: &str) -> Option<(std::ops::Range<usize>, String)> {
    match span.kind {
        CodeKind::Inline => {
            let v = output.trim().replace('\n', " ");
            let longest = v.split(|c| c != '`').map(|r| r.len()).max().unwrap_or(0);
            let ticks = "`".repeat(longest + 1);
            let pad = if v.starts_with('`') || v.ends_with('`') { " " } else { "" };
            Some((span.range.clone(), format!("{ticks}{pad}{v}{pad}{ticks}")))
        }
        CodeKind::Fenced if span.closed => {
            let v = output.trim_end_matches(['\n', '\r']);
            let new = if v.is_empty() { String::new() } else { format!("{v}\n") };
            Some((span.inner.clone(), new))
        }
        CodeKind::Indented => {
            let raw = &doc.text[span.range.clone()];
            let indent: String = raw.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            let v = output.trim_end_matches(['\n', '\r']);
            let mut new: String = v.lines().map(|l| format!("{indent}{l}\n")).collect();
            if !raw.ends_with('\n') {
                new.pop();
            }
            Some((span.range.clone(), new))
        }
        _ => None,
    }
}

pub fn verify(g: &Graph, node: Option<usize>, fix: bool, json_out: bool) -> R<Output> {
    let scope: Vec<usize> = match node {
        Some(i) => vec![i],
        None => (0..g.nodes.len()).collect(),
    };
    let list = &g.config.verify.run;
    let path = config::path_of(&g.project).unwrap_or_else(|| config::MDMAP_PATH.to_string());
    let mut checks: Vec<Check> = Vec::new();
    let mut with_verify = 0;
    let mut edits: BTreeMap<String, Vec<(std::ops::Range<usize>, String)>> = BTreeMap::new();

    for &i in &scope {
        let n = &g.nodes[i];
        let Some(doc) = g.docs.get(&n.file) else { continue };
        let comments: Vec<_> = doc
            .verifies
            .iter()
            .filter(|v| v.start >= n.body.start && v.start < n.body.end)
            .collect();
        if comments.is_empty() {
            continue;
        }
        with_verify += 1;
        for v in comments {
            let Some(span) = span_after(doc, v.end) else {
                checks.push(Check { node: i, status: "no span".into(), detail: vec![], failed: false });
                continue;
            };
            let words = split(&v.cmd).unwrap_or_default();
            let Some(entry) = allowed(list, &words).filter(|_| !words.is_empty()) else {
                let why = format!("{} is not in verify.run of {path}", refusal_name(list, &words, &v.cmd));
                checks.push(Check { node: i, status: "refused".into(), detail: vec![why], failed: true });
                continue;
            };
            // The words of the entry name the command; the rest are arguments
            // and go where {args} is.
            let named = split(&entry.command).map(|w| w.len()).unwrap_or(1).min(words.len());
            let args: Vec<String> = words[named..].iter().map(|w| w.text.clone()).collect();
            let cmd = match &entry.run {
                Some(c) => with_args(c, &args),
                None => v.cmd.clone(),
            };
            let timeout = entry.timeout.unwrap_or(g.config.verify.timeout);
            let (code, status, output, _) = exec(&cmd, &g.project, Duration::from_secs(timeout), false);
            if status == "FAIL timeout" {
                checks.push(Check { node: i, status: "timeout".into(), detail: vec![v.cmd.clone()], failed: true });
                continue;
            }
            let inline = span.kind == CodeKind::Inline;
            let want = normalize(&span.content, inline);
            let got = normalize(&output, inline);
            let mut detail = Vec::new();
            if code != 0 {
                detail.push(format!("exit {code}: {}", v.cmd));
            }
            if want == got && code == 0 {
                checks.push(Check { node: i, status: "ok".into(), detail, failed: false });
                continue;
            }
            let diff = TextDiff::from_lines(&want, &got);
            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => continue,
                };
                detail.push(format!("{sign} {}", change.value().trim_end_matches('\n')));
            }
            if fix && code == 0 {
                if let Some(edit) = replacement(doc, span, &output) {
                    edits.entry(n.file.clone()).or_default().push(edit);
                    checks.push(Check { node: i, status: "fixed".into(), detail, failed: false });
                    continue;
                }
            }
            checks.push(Check { node: i, status: "FAIL".into(), detail, failed: true });
        }
    }

    for (file, mut list) in edits {
        let doc = &g.docs[&file];
        let mut text = doc.text.clone();
        list.sort_by_key(|e| std::cmp::Reverse(e.0.start));
        for (r, new) in list {
            text.replace_range(r, &new);
        }
        let new_doc = Doc::parse(text.clone());
        let same = new_doc.headings.len() == doc.headings.len()
            && new_doc
                .headings
                .iter()
                .zip(&doc.headings)
                .all(|(a, b)| a.level == b.level && a.title == b.title && a.id == b.id);
        if !same || new_doc.open_fence_at_eof {
            return Err(Refusal::new(format!("verify --fix would change the structure of {file}")));
        }
        atomic_write(&g.project.join(&file), &text)?;
    }

    let failed = checks.iter().filter(|c| c.failed && c.status == "FAIL").count();
    let refused = checks.iter().filter(|c| c.status == "refused").count();
    let timeouts = checks.iter().filter(|c| c.status == "timeout").count();
    let fixed = checks.iter().filter(|c| c.status == "fixed").count();
    let mut summary = format!("{} nodes, {with_verify} with verify, {failed} failed", scope.len());
    if refused > 0 {
        summary.push_str(&format!(", {refused} refused"));
    }
    if timeouts > 0 {
        summary.push_str(&format!(", {timeouts} timed out"));
    }
    if fix {
        summary.push_str(&format!(", {fixed} fixed"));
    }

    let mut out = Output::default();
    if json_out {
        out.text = serde_json::to_string_pretty(&json!({
            "checks": checks.iter().map(|c| json!({
                "id": g.nodes[c.node].id, "title": g.nodes[c.node].title,
                "status": c.status, "detail": c.detail,
            })).collect::<Vec<_>>(),
            "summary": summary,
        }))
        .unwrap()
            + "\n";
    } else {
        let tw = checks.iter().map(|c| g.nodes[c.node].title.chars().count()).max().unwrap_or(0);
        let mut s = String::new();
        for c in &checks {
            let n = &g.nodes[c.node];
            let id = n.id.clone().unwrap_or_else(|| "-     ".into());
            s.push_str(&format!("{id}  {:<tw$}    {}\n", n.title, c.status));
            for d in &c.detail {
                s.push_str(&format!("  {d}\n"));
            }
        }
        if !checks.is_empty() {
            s.push('\n');
        }
        s.push_str(&summary);
        s.push('\n');
        out.text = s;
    }
    out.code = if checks.iter().any(|c| c.failed) { 1 } else { 0 };
    for c in &checks {
        out.touched.insert(g.nodes[c.node].file.clone());
    }
    Ok(out)
}
