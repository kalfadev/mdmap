//! `mdmap check`: can the tree be read correctly from these files?
//! `--fix` brings the files into the shape the tree expects, touching
//! structure only.
//!
//! The tree takes any heading levels: a `###` right under a `#` is its
//! child, and a file may hold several `#`. What it needs is a heading on
//! one line (`#`, not an underline) and one id per heading.

use std::collections::{BTreeMap, HashSet};

use crate::id;
use crate::out::{atomic_write, Output, R};
use serde_json::json;

use crate::graph::Graph;
use crate::markdown::{heading_line, Doc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemKind {
    Setext,
    MissingId,
    DuplicateId { id: String, first: String },
}

#[derive(Debug, Clone)]
pub struct Problem {
    /// The markdown file the problem is in.
    pub file: String,
    /// What the report prints: `docs/arch.md:88`.
    pub at: String,
    pub kind: ProblemKind,
}

impl Problem {
    pub fn message(&self) -> String {
        match &self.kind {
            ProblemKind::Setext => "setext heading".into(),
            ProblemKind::MissingId => "heading without id".into(),
            ProblemKind::DuplicateId { id, first } => {
                format!("duplicate id {{{id}}} — same as {first}")
            }
        }
    }

    /// Missing or duplicated ids; every call fixes these before it reads.
    pub fn id_problem(&self) -> bool {
        matches!(self.kind, ProblemKind::MissingId | ProblemKind::DuplicateId { .. })
    }

    /// Problems that stop the tree from being read correctly, as opposed to
    /// missing or duplicated ids.
    pub fn structural(&self) -> bool {
        matches!(self.kind, ProblemKind::Setext)
    }
}

/// Problems inside one file, ids excepted from duplication.
pub fn local_problems(file: &str, doc: &Doc) -> Vec<Problem> {
    let mut out = Vec::new();
    let at = |line: usize| format!("{file}:{line}");
    for h in &doc.headings {
        if h.setext {
            out.push(Problem { file: file.into(), at: at(h.line), kind: ProblemKind::Setext });
        }
        if h.id.is_none() {
            out.push(Problem { file: file.into(), at: at(h.line), kind: ProblemKind::MissingId });
        }
    }
    out
}

/// Every problem in the corpus, in path order.
pub fn problems(g: &Graph) -> Vec<Problem> {
    let mut out = Vec::new();
    for (file, doc) in &g.docs {
        out.extend(local_problems(file, doc));
    }
    out.extend(duplicates(g.docs.iter().map(|(f, d)| (f.as_str(), d))));
    out.sort_by(|a, b| (&a.file, line_num(&a.at)).cmp(&(&b.file, line_num(&b.at))));
    out
}

fn line_num(at: &str) -> usize {
    at.rsplit(':').next().and_then(|n| n.parse().ok()).unwrap_or(0)
}

fn duplicates<'a>(docs: impl Iterator<Item = (&'a str, &'a Doc)>) -> Vec<Problem> {
    let mut first: BTreeMap<String, String> = BTreeMap::new();
    let mut out = Vec::new();
    for (file, doc) in docs {
        for h in &doc.headings {
            let Some(hid) = &h.id else { continue };
            let here = format!("{file}:{}", h.line);
            match first.get(hid) {
                Some(f) => out.push(Problem {
                    file: file.into(),
                    at: here,
                    kind: ProblemKind::DuplicateId { id: hid.clone(), first: f.clone() },
                }),
                None => {
                    first.insert(hid.clone(), here);
                }
            }
        }
    }
    out
}

pub fn render(problems: &[Problem]) -> String {
    let width = problems.iter().map(|p| p.at.chars().count()).max().unwrap_or(0);
    let mut s = String::new();
    for p in problems {
        s.push_str(&format!("{:<width$}   {}\n", p.at, p.message()));
    }
    s
}

/// The report of `mdmap check`: the problems of the corpus.
pub fn check(g: &Graph, json: bool) -> Output {
    let problems = problems(g);
    let mut out = if json {
        Output::text(
            serde_json::to_string_pretty(&json!({
                "problems": problems.iter().map(|p| json!({
                    "at": p.at, "problem": p.message()
                })).collect::<Vec<_>>(),
            }))
            .unwrap()
                + "\n",
        )
    } else if problems.is_empty() {
        Output::text("no check problems\n")
    } else {
        Output::text(render(&problems))
    };
    out.code = if problems.is_empty() { 0 } else { 1 };
    out
}

/// A fix applied, for the report.
pub struct Fixed {
    pub at: String,
    pub message: String,
}

/// Which problems a pass fixes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Everything `check --fix` fixes.
    All,
    /// Only what gives every heading one id: an id for a heading, a new id
    /// for a duplicate. Every call runs this before it reads.
    Ids,
}

/// Apply fixes to the corpus; returns what was fixed and the files written.
pub fn apply_fixes(g: &Graph, scope: Scope) -> R<(Vec<Fixed>, Vec<String>)> {
    let mut taken = g.all_ids();
    let mut texts: BTreeMap<String, String> =
        g.docs.iter().map(|(f, d)| (f.clone(), d.text.clone())).collect();
    let originals = texts.clone();
    let mut fixed: Vec<Fixed> = Vec::new();

    let files: Vec<String> = texts.keys().cloned().collect();
    for file in &files {
        let mut text = texts[file].clone();
        for _ in 0..10_000 {
            let doc = Doc::parse(text.clone());
            let problems = local_problems(file, &doc);
            let Some(p) = pick(&problems, scope) else { break };
            text = apply(&doc, p, &mut taken);
            fixed.push(Fixed { at: p.at.clone(), message: p.message() });
        }
        texts.insert(file.clone(), text);
    }

    // Duplicates last: the first heading in path order keeps its id.
    let parsed: Vec<(String, Doc)> =
        texts.iter().map(|(f, t)| (f.clone(), Doc::parse(t.clone()))).collect();
    let dups = duplicates(parsed.iter().map(|(f, d)| (f.as_str(), d)));
    let mut by_file: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for p in &dups {
        by_file.entry(p.file.clone()).or_default().push(line_num(&p.at));
        fixed.push(Fixed { at: p.at.clone(), message: p.message() });
    }
    for (file, lines) in by_file {
        let doc = &parsed.iter().find(|(f, _)| *f == file).unwrap().1;
        let mut text = doc.text.clone();
        for h in doc.headings.iter().rev() {
            if lines.contains(&h.line) {
                let hid = id::generate(&mut taken);
                let nl = line_ending(&text[h.start..h.end]);
                text.replace_range(h.start..h.end, &(heading_line(h.level, &h.title, Some(&hid)) + nl));
            }
        }
        texts.insert(file, text);
    }

    let mut written = Vec::new();
    for (file, text) in &texts {
        if originals.get(file) != Some(text) {
            atomic_write(&g.project.join(file), text)?;
            written.push(file.clone());
        }
    }
    Ok((fixed, written))
}

pub fn fix(g: &Graph, json: bool) -> R<Output> {
    let (fixed, written) = apply_fixes(g, Scope::All)?;
    let written = written.len();
    let mut out = if json {
        Output::text(
            serde_json::to_string_pretty(&json!({
                "fixed": fixed.iter().map(|f| json!({"at": f.at, "problem": f.message})).collect::<Vec<_>>(),
                "files": written,
            }))
            .unwrap()
                + "\n",
        )
    } else if fixed.is_empty() {
        Output::text("no check problems\n")
    } else {
        let width = fixed.iter().map(|f| f.at.chars().count()).max().unwrap_or(0);
        let mut s = String::new();
        for f in &fixed {
            s.push_str(&format!("{:<width$}   {} — fixed\n", f.at, f.message));
        }
        s.push_str(&format!(
            "\nfixed {} problem{} in {} file{}\n",
            fixed.len(),
            if fixed.len() == 1 { "" } else { "s" },
            written,
            if written == 1 { "" } else { "s" }
        ));
        Output::text(s)
    };
    out.code = 0;
    Ok(out)
}

/// A setext heading first: its id goes on the `#` line the fix writes.
fn pick(problems: &[Problem], scope: Scope) -> Option<&Problem> {
    let rank = |k: &ProblemKind| match (k, scope) {
        (ProblemKind::Setext, Scope::All) => 1,
        (ProblemKind::MissingId, _) => 2,
        _ => 9,
    };
    problems.iter().filter(|p| rank(&p.kind) < 9).min_by_key(|p| rank(&p.kind))
}

fn line_ending(line: &str) -> &'static str {
    if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

fn apply(doc: &Doc, p: &Problem, taken: &mut HashSet<String>) -> String {
    let mut text = doc.text.clone();
    let line = line_num(&p.at);
    let Some(h) = doc.headings.iter().find(|h| h.line == line) else { return text };
    match &p.kind {
        ProblemKind::Setext => {
            let new = heading_line(h.level, &h.title, h.id.as_deref());
            text.replace_range(h.start..h.end, &(new + "\n"));
        }
        ProblemKind::MissingId if h.setext => {
            // A setext heading keeps its form: the id goes at the end of its
            // text, and the underline is structure, which check --fix alone
            // turns into a # line. Every call runs this pass, reads included.
            let hid = id::generate(taken);
            let raw = text[h.start..h.end].trim_end_matches(['\n', '\r']);
            let mut at = h.start + raw.rfind('\n').unwrap_or(raw.len());
            if text[..at].ends_with('\r') {
                at -= 1;
            }
            text.insert_str(at, &format!(" {{{hid}}}"));
        }
        ProblemKind::MissingId => {
            let hid = id::generate(taken);
            let nl = line_ending(&text[h.start..h.end]);
            text.replace_range(h.start..h.end, &(heading_line(h.level, &h.title, Some(&hid)) + nl));
        }
        ProblemKind::DuplicateId { .. } => {}
    }
    text
}
