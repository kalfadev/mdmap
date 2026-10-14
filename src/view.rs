//! `show`, `ls` and `search`: the ways to read the tree.

use crate::out::{Output, R};
use serde_json::{json, Map, Value};

use crate::graph::{Act, Graph, Kind};

#[derive(Clone)]
pub struct ShowArgs {
    pub body: bool,
    pub path: bool,
    /// Print a verify block instead of counting its lines.
    pub outline: bool,
}

/// The body without blank lines at either end, and the file lines it takes;
/// None when the body has no text.
fn body_trimmed(g: &Graph, i: usize) -> Option<((usize, usize), String)> {
    let (a, _) = g.body_lines(i)?;
    let lines: Vec<&str> = g.body_text(i).lines().collect();
    let first = lines.iter().position(|l| !l.trim().is_empty())?;
    let last = lines.iter().rposition(|l| !l.trim().is_empty()).unwrap();
    Some(((a + first, a + last), lines[first..=last].join("\n") + "\n"))
}

/// The body, each line after its file line number. The fenced block under a
/// `<!-- verify: … -->` comment is the output of that command, which the
/// reader can run itself: its lines are counted, not printed, unless
/// `outline` asks for them. It is the longer half of an area of the code
/// map, and prose is what a body is read for.
fn body_block(g: &Graph, i: usize, outline: bool) -> String {
    let Some(((a, b), text)) = body_trimmed(g, i) else {
        return String::new();
    };
    let w = b.to_string().len();
    let num = |l: &str, n: usize| if l.is_empty() { format!("{n:>w$}\n") } else { format!("{n:>w$}  {l}\n") };
    let mut s = String::new();
    let mut lines = text.lines().zip(a..);
    let mut after_verify = false;
    while let Some((l, n)) = lines.next() {
        s.push_str(&num(l, n));
        let head = l.trim_start();
        if head.starts_with("<!-- verify:") {
            after_verify = !outline;
        } else if after_verify && head.starts_with("```") {
            after_verify = false;
            // The lines are the file: what the fences hold is the count between them.
            let mut inside: Vec<(&str, usize)> = Vec::new();
            let mut close = None;
            for (inner, m) in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    close = Some((inner, m));
                    break;
                }
                inside.push((inner, m));
            }
            match close {
                Some((inner, m)) => {
                    if m > n + 1 {
                        s.push_str(&format!("{:>w$}  … {} lines: mdmap show {} --outline\n", "", m - n - 1, g.key(i)));
                    }
                    s.push_str(&num(inner, m));
                }
                // A fence that never closes holds no block to count, and the
                // lines are the body: the reader gets them.
                None => {
                    for (inner, m) in inside {
                        s.push_str(&num(inner, m));
                    }
                }
            }
        } else if after_verify && !head.is_empty() {
            // Only the block right under the comment, blank lines apart, is
            // the output of that command; a later block is prose and is printed.
            after_verify = false;
        }
    }
    s
}

fn insert_body(g: &Graph, i: usize, m: &mut Map<String, Value>) {
    let t = body_trimmed(g, i);
    m.insert("lines".into(), json!(t.as_ref().map(|((a, b), _)| vec![*a, *b])));
    m.insert("body".into(), json!(t.map(|(_, s)| s).unwrap_or_default()));
}

/// Wrap `text` to `width` columns, continuation lines indented by `indent`.
fn wrap(text: &str, width: usize, indent: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines.join(&format!("\n{indent}"))
}

pub fn node_json(g: &Graph, i: usize) -> Map<String, Value> {
    let n = &g.nodes[i];
    let mut m = Map::new();
    // A read-denied node: that it is there, never its id, title or body.
    if g.denied(i, Act::Read) {
        if n.kind != Kind::Section {
            m.insert("key".into(), json!(g.key(i)));
        }
        m.insert("level".into(), json!(n.level));
        m.insert("kind".into(), json!(n.kind.name()));
        m.insert("denied".into(), json!(true));
        return m;
    }
    m.insert("key".into(), json!(g.key(i)));
    m.insert("id".into(), json!(n.id));
    m.insert("heading".into(), json!(g.heading(i)));
    m.insert("level".into(), json!(n.level));
    m.insert("kind".into(), json!(n.kind.name()));
    m.insert("title".into(), json!(n.title));
    m.insert("body_chars".into(), json!(n.body_chars));
    m.insert("subtree".into(), json!(n.subtree));
    m
}

/// The files a call read, for the check warning; a folder has none.
fn touch(g: &Graph, i: usize, out: &mut Output) {
    let f = &g.nodes[i].file;
    if !f.is_empty() && !g.denied(i, Act::Read) {
        out.touched.insert(f.clone());
    }
}

/// Advice for a node whose prose is past `warn.body`.
fn split_advice(g: &Graph, i: usize, out: &mut Output) {
    let limit = g.config.warn.body;
    let chars = g.prose_chars(i);
    if limit > 0 && chars > limit {
        let who = g.key(i);
        let line = format!("warning: {who} body {chars} > {limit} — mdmap add {who} --title …");
        if !out.advice.contains(&line) {
            out.advice.push(line);
        }
    }
}

pub fn show(g: &Graph, nodes: &[usize], a: &ShowArgs, json_out: bool) -> R<Output> {
    let mut out = Output::default();
    let mut blocks = Vec::new();
    let mut jnodes = Vec::new();
    for &i in nodes {
        let n = &g.nodes[i];
        let mut touched = vec![i];
        split_advice(g, i, &mut out);
        if a.body {
            // Reading: the heading, where the body sits in its file, the body.
            touch(g, i, &mut out);
            if json_out {
                let mut m = Map::new();
                m.insert("key".into(), json!(g.key(i)));
                m.insert("id".into(), json!(n.id));
                m.insert("heading".into(), json!(g.heading(i)));
                if a.path {
                    m.insert("path".into(), json!(g.location(i)));
                }
                insert_body(g, i, &mut m);
                jnodes.push(Value::Object(m));
            } else {
                let mut s = format!("{}\n", g.heading(i));
                if a.path && n.kind == Kind::Section {
                    s.push_str(&format!("path {}\n", g.location(i)));
                }
                s.push_str(&body_block(g, i, a.outline));
                blocks.push(s);
            }
            continue;
        }

        let mut rows: Vec<(&str, usize)> = Vec::new();
        if let Some(p) = n.parent {
            rows.push(("parent", p));
        }
        for &c in &n.children {
            rows.push(("children", c));
        }
        for &l in &n.links {
            rows.push(("link", l));
        }
        for &b in &n.backlinks {
            rows.push(("backlink", b));
        }
        touched.extend(rows.iter().map(|r| r.1));

        if json_out {
            let mut m = node_json(g, i);
            m.insert("path".into(), json!(g.location(i)));
            m.insert("summary".into(), json!(g.summary(i)));
            m.insert("nodes".into(), json!(n.nodes));
            m.insert("max_depth".into(), json!(n.max_depth));
            let neighbour = |x: usize| Value::Object(node_json(g, x));
            m.insert("parent".into(), json!(n.parent.map(neighbour)));
            m.insert("children".into(), json!(n.children.iter().map(|&x| neighbour(x)).collect::<Vec<_>>()));
            m.insert("links".into(), json!(n.links.iter().map(|&x| neighbour(x)).collect::<Vec<_>>()));
            m.insert("backlinks".into(), json!(n.backlinks.iter().map(|&x| neighbour(x)).collect::<Vec<_>>()));
            jnodes.push(Value::Object(m));
        } else {
            let mut s = format!("  {}\n", g.heading(i));
            if a.path && n.kind == Kind::Section {
                s.push_str(&format!("  path       {}\n", g.location(i)));
            }
            s.push('\n');
            let summary = g.summary(i).unwrap_or_else(|| "-".into());
            s.push_str(&format!("  summary    {}\n", wrap(&summary, 64, "             ")));
            s.push_str(&format!("  body       {}\n", n.body_chars));
            s.push_str(&format!("  subtree    {}\n", n.subtree));
            s.push_str(&format!("  nodes      {}\n", n.nodes));
            s.push_str(&format!("  max_depth  {}\n", n.max_depth));
            if !rows.is_empty() {
                let hw = rows.iter().map(|r| masked(g, r.1, g.heading(r.1)).chars().count()).max().unwrap();
                // The size of a read-denied neighbour is not told.
                let size = |x: usize, v: usize| if g.denied(x, Act::Read) { "-".to_string() } else { v.to_string() };
                let bw = rows.iter().map(|r| size(r.1, g.nodes[r.1].body_chars).len()).max().unwrap().max(4);
                let sw = rows.iter().map(|r| size(r.1, g.nodes[r.1].subtree).len()).max().unwrap().max(7);
                s.push('\n');
                s.push_str(&format!("  {:<8}  {:<hw$}  {:>bw$}  {:>sw$}\n", "", "", "body", "subtree"));
                let mut last = "";
                for (role, x) in &rows {
                    let label = if *role == last { "" } else { role };
                    last = role;
                    s.push_str(&format!(
                        "  {:<8}  {:<hw$}  {:>bw$}  {:>sw$}\n",
                        label,
                        masked(g, *x, g.heading(*x)),
                        size(*x, g.nodes[*x].body_chars),
                        size(*x, g.nodes[*x].subtree)
                    ));
                }
            }
            blocks.push(s);
        }
        for t in touched {
            touch(g, t, &mut out);
        }
    }
    out.text = if json_out {
        serde_json::to_string_pretty(&json!({ "nodes": jnodes })).unwrap() + "\n"
    } else {
        blocks.join("\n")
    };
    Ok(out)
}

#[derive(Clone)]
pub struct LsArgs {
    /// Levels below the listed node; None prints the whole tree.
    pub depth: Option<usize>,
    pub summary: bool,
    pub body: bool,
}

/// One line of `ls`: a node at a depth, and for a file that holds a single
/// heading at its top, that heading, printed on the same line. `hidden` is
/// how many nodes under it the depth left out.
struct Row {
    node: usize,
    depth: usize,
    heading: Option<usize>,
    hidden: usize,
}

/// The one heading at the top of a file, when there is exactly one.
fn only_heading(g: &Graph, i: usize) -> Option<usize> {
    let n = &g.nodes[i];
    match n.children.as_slice() {
        [c] if n.kind == Kind::File => Some(*c),
        _ => None,
    }
}

fn collect(g: &Graph, i: usize, depth: usize, max: usize, rows: &mut Vec<Row>) {
    let heading = only_heading(g, i);
    let under = heading.unwrap_or(i);
    // A read-denied node is one line, and nothing under it.
    if g.denied(under, Act::Read) {
        let heading = heading.filter(|_| !g.denied(i, Act::Read));
        rows.push(Row { node: i, depth, heading, hidden: 0 });
        return;
    }
    let hidden = if depth < max { 0 } else { g.nodes[under].nodes.saturating_sub(1) };
    rows.push(Row { node: i, depth, heading, hidden });
    if depth < max {
        for &c in &g.nodes[under].children {
            collect(g, c, depth + 1, max, rows);
        }
    }
}

fn tree_json(g: &Graph, i: usize, depth: usize, max: usize, a: &LsArgs) -> Value {
    let mut m = node_json(g, i);
    if g.denied(i, Act::Read) {
        return Value::Object(m);
    }
    if a.summary {
        m.insert("summary".into(), json!(g.summary(i)));
    }
    if a.body {
        insert_body(g, i, &mut m);
    }
    if depth < max {
        m.insert(
            "children".into(),
            json!(g.nodes[i].children.iter().map(|&c| tree_json(g, c, depth + 1, max, a)).collect::<Vec<_>>()),
        );
    } else if !g.nodes[i].children.is_empty() {
        // The depth stopped here: how many nodes are under it, unread.
        m.insert("hidden".into(), json!(g.nodes[i].nodes.saturating_sub(1)));
    }
    Value::Object(m)
}

/// A node as `ls` prints it: a folder by its path, a file by its name, a
/// heading as it reads in the file.
fn label(g: &Graph, i: usize) -> String {
    let n = &g.nodes[i];
    let name = match n.kind {
        Kind::Folder => g.key(i),
        Kind::File => n.title.clone(),
        Kind::Section => g.heading(i),
    };
    masked(g, i, name)
}

/// A read-denied node in a list: the `#` of a heading, or the path of a
/// folder or a file, then `[denied]`; never its id or its title.
fn masked(g: &Graph, i: usize, name: String) -> String {
    let n = &g.nodes[i];
    if !g.denied(i, Act::Read) {
        name
    } else if n.kind == Kind::Section {
        format!("{} [denied]", "#".repeat(n.level as usize))
    } else {
        format!("{name}  [denied]")
    }
}

pub fn ls(g: &Graph, node: Option<usize>, a: &LsArgs, json_out: bool) -> R<Output> {
    let depth = a.depth.unwrap_or(usize::MAX).max(1);
    let mut rows = Vec::new();
    let mut jtrees = Vec::new();
    match node {
        Some(i) => {
            for &c in &g.nodes[only_heading(g, i).filter(|&h| !g.denied(h, Act::Read)).unwrap_or(i)].children {
                collect(g, c, 0, depth - 1, &mut rows);
                jtrees.push(tree_json(g, c, 0, depth - 1, a));
            }
        }
        None => {
            // No node: every root, as its own tree.
            for &r in &g.roots {
                collect(g, r, 0, depth, &mut rows);
                jtrees.push(tree_json(g, r, 0, depth, a));
            }
        }
    }
    let mut out = Output::default();
    for r in &rows {
        for x in std::iter::once(r.node).chain(r.heading).filter(|&x| !g.denied(x, Act::Read)) {
            if a.body {
                split_advice(g, x, &mut out);
            }
            touch(g, x, &mut out);
        }
    }
    if json_out {
        out.text = serde_json::to_string_pretty(&json!({ "nodes": jtrees })).unwrap() + "\n";
        return Ok(out);
    }
    let mut s = String::new();
    let mut any_hidden = false;
    for r in &rows {
        let pad = "   ".repeat(r.depth);
        let mut line = format!("{pad}{}", label(g, r.node));
        if let Some(h) = r.heading {
            line.push_str(&format!("  {}", label(g, h)));
        }
        s.push_str(&line);
        s.push('\n');
        let shown: Vec<usize> = std::iter::once(r.node).chain(r.heading).filter(|&x| !g.denied(x, Act::Read)).collect();
        if a.summary {
            if let Some(sum) = shown.iter().rev().find_map(|&x| g.summary(x)) {
                s.push_str(&format!("{pad}  {sum}\n"));
            }
        }
        if a.body {
            for x in shown {
                s.push_str(&body_block(g, x, false));
            }
        }
        // Where the depth cut the tree, the line says how much is left unread.
        if r.hidden > 0 {
            any_hidden = true;
            s.push_str(&format!("{pad}   … {} more\n", r.hidden));
        }
    }
    if any_hidden {
        s.push_str("\n… = nodes not shown; open one: mdmap ls <id>|<path>\n");
    }
    out.text = s;
    Ok(out)
}

#[derive(Clone)]
pub struct SearchArgs {
    pub summary: bool,
    pub body: bool,
}

/// Lowercase, one char for one char, so a position in the folded text is a position in the original.
fn fold(s: &str) -> Vec<char> {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

/// Whether `needle` occurs in `hay` as whole words: where the needle starts or ends with a letter, a digit or `_`,
/// the character next to it in `hay` is none of these, so `search` does not find `WebSearch`.
fn has_words(hay: &[char], needle: &[char]) -> bool {
    let word = |c: &char| c.is_alphanumeric() || *c == '_';
    let n = needle.len();
    hay.windows(n).enumerate().any(|(at, w)| {
        w == needle
            && (at == 0 || !word(&needle[0]) || !word(&hay[at - 1]))
            && (at + n == hay.len() || !word(&needle[n - 1]) || !word(&hay[at + n]))
    })
}

/// Every node a read may reach, in tree order.
fn walk(g: &Graph, i: usize, out: &mut Vec<usize>) {
    if g.denied(i, Act::Read) {
        return;
    }
    out.push(i);
    for &c in &g.nodes[i].children {
        walk(g, c, out);
    }
}

/// Every node whose title or body holds `text` as whole words, case ignored, in tree order: the heading as written,
/// or the path of a folder or a file; `--summary` adds the summary, `--body` the lines and the body.
pub fn search(g: &Graph, text: &str, a: &SearchArgs, json_out: bool) -> R<Output> {
    let needle = fold(text.trim());
    if needle.is_empty() {
        return Err(crate::out::Refusal::new("search takes the text to find").instead("mdmap search \"<text>\""));
    }
    let mut order = Vec::new();
    for &r in &g.roots {
        walk(g, r, &mut order);
    }
    let mut out = Output::default();
    let mut blocks = Vec::new();
    let mut jnodes = Vec::new();
    for i in order {
        if !has_words(&fold(&g.nodes[i].title), &needle) && !has_words(&fold(g.body_text(i)), &needle) {
            continue;
        }
        touch(g, i, &mut out);
        if a.body {
            split_advice(g, i, &mut out);
        }
        if json_out {
            let mut m = node_json(g, i);
            // The body starts with the summary: with --body, --summary adds nothing.
            if a.summary && !a.body {
                m.insert("summary".into(), json!(g.summary(i)));
            }
            if a.body {
                insert_body(g, i, &mut m);
            }
            jnodes.push(Value::Object(m));
            continue;
        }
        let mut s = g.heading(i) + "\n";
        if a.summary && !a.body {
            if let Some(sum) = g.summary(i) {
                s.push_str(&format!("  {sum}\n"));
            }
        }
        if a.body {
            s.push_str(&body_block(g, i, false));
        }
        blocks.push(s);
    }
    if json_out {
        out.text = serde_json::to_string_pretty(&json!({ "nodes": jnodes })).unwrap() + "\n";
    } else if blocks.is_empty() {
        out.text = format!("no node holds \"{}\"\n", text.trim());
    } else {
        out.text = blocks.join(if a.body { "\n" } else { "" });
    }
    Ok(out)
}
