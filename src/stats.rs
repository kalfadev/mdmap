//! `stats`: which headings the log shows the agent opened, and which it
//! never did.
//!
//! The log of the gate keeps the words of every call, so the ids a call
//! named are in its arguments: the log says what the docs cost, a heading
//! opened again and again, and a heading no call ever opened.

use std::collections::BTreeMap;

use crate::out::{Output, R};
use serde_json::{json, Value};

use crate::graph::{Act, Graph, Kind};

/// Where a harno project keeps the calls that went through it. mdmap only
/// reads them, and a project without a log has no opened headings.
const GATE_DIR: &str = ".harno/log/harno";

/// An id as a heading carries it: six hex characters.
fn is_id(s: &str) -> bool {
    s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit())
}

pub struct StatsArgs {
    /// Only the headings no call ever named.
    pub unread: bool,
    /// The opened headings by size times calls, the costliest first.
    pub cost: bool,
}

/// How often each id was named in a call, over every session of the log.
/// The gate keeps no word of a tool, so the ids are read out of the
/// arguments of the call, where the agent wrote them.
fn opened(g: &Graph) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    let Ok(read) = std::fs::read_dir(g.project.join(GATE_DIR)) else { return out };
    for e in read.flatten() {
        if !e.file_name().to_string_lossy().ends_with(".jsonl") {
            continue;
        }
        let text = std::fs::read_to_string(e.path()).unwrap_or_default();
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let Some(args) = v.get("args").and_then(|a| a.as_array()) else { continue };
            for id in args
                .iter()
                .filter_map(|a| a.as_str())
                .flat_map(|a| a.split_whitespace())
                .map(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
                .filter(|w| is_id(w))
            {
                *out.entry(id.to_string()).or_default() += 1;
            }
        }
    }
    out
}

/// What the corpus itself adds up to: its files and folders, the size of every
/// body, the nodes over `warn.body` and the ones with no body at all.
struct Corpus {
    files: usize,
    folders: usize,
    chars: usize,
    long: Vec<usize>,
    empty: Vec<usize>,
}

fn corpus(g: &Graph) -> Corpus {
    let mut c = Corpus { files: 0, folders: 0, chars: 0, long: Vec::new(), empty: Vec::new() };
    for (i, n) in g.nodes.iter().enumerate() {
        if g.denied(i, Act::Read) {
            continue;
        }
        c.chars += n.body_chars;
        match n.kind {
            Kind::File => c.files += 1,
            Kind::Folder => c.folders += 1,
            Kind::Section => {
                if g.config.warn.body > 0 && g.prose_chars(i) > g.config.warn.body {
                    c.long.push(i);
                }
                if n.body_chars == 0 {
                    c.empty.push(i);
                }
            }
        }
    }
    c.long.sort_by_key(|b| std::cmp::Reverse(g.prose_chars(*b)));
    c
}

/// The corpus in numbers, then the headings most opened first and the ones
/// never opened, the biggest of them first: what the agent reads, and what
/// nobody reads.
pub fn stats(g: &Graph, a: &StatsArgs, json_out: bool) -> R<Output> {
    let counts = opened(g);
    let c = corpus(g);
    let mut open: Vec<(usize, usize)> = Vec::new();
    let mut never: Vec<usize> = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        if n.kind != Kind::Section || g.denied(i, Act::Read) {
            continue;
        }
        let Some(id) = n.id.as_deref() else { continue };
        match counts.get(id) {
            Some(c) => open.push((*c, i)),
            None => never.push(i),
        }
    }
    open.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    if a.cost {
        open.sort_by(|x, y| (g.prose_chars(y.1) * y.0).cmp(&(g.prose_chars(x.1) * x.0)).then(x.1.cmp(&y.1)));
    }
    never.sort_by(|a, b| g.prose_chars(*b).cmp(&g.prose_chars(*a)).then(a.cmp(b)));
    let headings = open.len() + never.len();
    if json_out {
        let one = |i: usize, calls: usize| {
            json!({"id": g.nodes[i].id, "title": g.nodes[i].title, "file": g.nodes[i].file, "chars": g.prose_chars(i), "calls": calls, "cost": g.prose_chars(i) * calls})
        };
        let mut rows: Vec<Value> = Vec::new();
        if !a.unread {
            rows.extend(open.iter().map(|(c, i)| one(*i, *c)));
        }
        rows.extend(never.iter().map(|i| one(*i, 0)));
        let v = json!({"files": c.files, "folders": c.folders, "headings": headings, "chars": c.chars, "long": c.long.len(), "empty": c.empty.len(), "opened": open.len(), "never": never.len(), "nodes": rows});
        return Ok(Output::text(serde_json::to_string_pretty(&v).unwrap() + "\n"));
    }
    let row = |i: usize, calls: usize| format!("  {:>4}  {}  {} chars\n", calls, g.heading(i), g.prose_chars(i));
    let mut t = format!("corpus: {} files, {} folders, {headings} headings, {} chars\n", c.files, c.folders, c.chars);
    if !c.long.is_empty() || !c.empty.is_empty() {
        t.push_str(&format!("{} over warn.body {}, {} with no body\n", c.long.len(), g.config.warn.body, c.empty.len()));
    }
    t.push_str(&format!("{headings} headings  opened {}  never opened {}\n", open.len(), never.len()));
    if !a.unread && !open.is_empty() && a.cost {
        t.push_str("\nopened, most costly first (chars × calls)\n");
        for (c, i) in &open {
            t.push_str(&format!("  {:>7}  {}  {} chars × {} calls\n", g.prose_chars(*i) * c, g.heading(*i), g.prose_chars(*i), c));
        }
    } else if !a.unread && !open.is_empty() {
        t.push_str("\nopened, most called first\n");
        for (c, i) in &open {
            t.push_str(&row(*i, *c));
        }
    }
    if !never.is_empty() {
        t.push_str("\nnever opened, biggest first\n");
        for i in &never {
            t.push_str(&row(*i, 0));
        }
    }
    Ok(Output::text(t))
}
