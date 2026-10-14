//! `add`, `update`, `remove`: the only ways the agent writes.
//!
//! A write changes only the bytes it owns. Before anything reaches disk the
//! result is parsed in memory and compared with what the verb was supposed
//! to do; a mismatch (typically an unclosed code fence swallowing the
//! headings after it) refuses the whole write. Files are written through a
//! temp file and a rename.

use std::collections::BTreeSet;
use std::ops::Range;

use crate::id;
use crate::out::{atomic_write, dq, quote, Output, Refusal, R};
use serde_json::json;

use crate::check::local_problems;
use crate::config::{template_headings, MDMAP_PATH};
use crate::graph::{join, Graph, Kind, README};
use crate::markdown::{has_heading, heading_line, Doc};
use crate::slug::slug;

/// What a heading should look like after a write.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sig {
    level: u8,
    title: String,
    id: Option<String>,
    /// Trimmed body; None when the verb may change it.
    body: Option<String>,
}

fn sigs(doc: &Doc) -> Vec<Sig> {
    (0..doc.headings.len())
        .map(|i| {
            let h = &doc.headings[i];
            Sig {
                level: h.level,
                title: h.title.clone(),
                id: h.id.clone(),
                body: Some(doc.text[doc.body_of(i)].trim().to_string()),
            }
        })
        .collect()
}

/// Parse `text` and compare it with the expected headings.
fn expect(file: &str, text: &str, expected: &[Sig]) -> R<Doc> {
    let doc = Doc::parse(text.to_string());
    let got = sigs(&doc);
    if got.len() < expected.len() {
        let lost = expected.len() - got.len();
        return Err(Refusal::new(format!(
            "the write would turn {lost} heading{} in {file} into plain text — is a code fence left open?",
            if lost == 1 { "" } else { "s" }
        )));
    }
    let same = got.len() == expected.len()
        && got.iter().zip(expected).all(|(g, e)| {
            g.level == e.level
                && g.title == e.title
                && g.id == e.id
                && e.body.as_ref().is_none_or(|b| g.body.as_deref() == Some(b.as_str()))
        });
    if !same {
        return Err(Refusal::new(format!(
            "the write would change nodes in {file} it does not own"
        )));
    }
    if doc.open_fence_at_eof {
        return Err(Refusal::new(format!("{file} would end inside an open code fence")));
    }
    Ok(doc)
}

fn validate_title(t: &str) -> R<()> {
    if t.trim().is_empty() {
        return Err(Refusal::new("the title is empty"));
    }
    if t.contains('\n') || t.contains('\r') {
        return Err(Refusal::new("a title is one line"));
    }
    Ok(())
}

fn validate_body(b: &str) -> R<()> {
    if has_heading(b) {
        return Err(Refusal::new(
            "the body contains a heading; a heading is a new node — add it with its own --title",
        ));
    }
    Ok(())
}

/// A body as it sits under its heading: one blank line, the text, one
/// newline, and a blank line when another heading follows.
fn body_block(body: &str, followed: bool) -> String {
    let t = body.trim_end().trim_start_matches(['\n', '\r']);
    let mut s = String::new();
    if !t.is_empty() {
        s.push('\n');
        s.push_str(t);
        s.push('\n');
    }
    if followed {
        s.push('\n');
    }
    s
}

/// Text to put at `pos` so the inserted block is separated by blank lines.
fn separated(before: &str, block: &str, followed: bool) -> String {
    let mut s = String::new();
    if !before.is_empty() {
        if !before.ends_with('\n') {
            s.push_str("\n\n");
        } else if !before.ends_with("\n\n") {
            s.push('\n');
        }
    }
    s.push_str(block);
    if followed {
        s.push('\n');
    }
    s
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddMode {
    Default,
    /// Under a folder: a new directory whose README.md holds the heading.
    Dir,
}

pub struct Child {
    /// `None` only for `add <id> --template <name>`: the template goes under
    /// the node itself.
    pub title: Option<String>,
    pub body: String,
    pub template: Option<Applied>,
}

/// `--template <name>` and the `--fill <path> <text>` pairs after it.
pub struct Applied {
    pub name: String,
    pub fills: Vec<(String, String)>,
}

/// One heading to write: its depth below the first new level, title, body.
struct New {
    depth: u8,
    title: String,
    body: String,
}

enum Plan {
    /// `at`: index of the first new heading among the file's headings.
    Write { file: String, text: String, at: usize },
    /// `heads`: the new headings the file holds, as indexes into `born`.
    Create { file: String, text: String, heads: Range<usize> },
}

/// The headings the children stand for, templates expanded, in file order.
fn expand(g: &Graph, children: &[Child]) -> R<Vec<New>> {
    let mut news = Vec::new();
    for c in children {
        let base = match &c.title {
            Some(t) => {
                news.push(New { depth: 0, title: t.clone(), body: c.body.clone() });
                1
            }
            None => 0,
        };
        let Some(a) = &c.template else { continue };
        let Some(items) = g.config.templates.get(&a.name) else {
            let names: Vec<&str> = g.config.templates.keys().map(|s| s.as_str()).collect();
            return Err(Refusal::new(if names.is_empty() {
                format!("no template {}; {MDMAP_PATH} has none", quote(&a.name))
            } else {
                format!("no template {}; the templates are {}", quote(&a.name), names.join(", "))
            }));
        };
        let heads = template_headings(items);
        if let Some((path, _)) = a.fills.iter().find(|f| !heads.iter().any(|h| h.2 == f.0)) {
            let paths: Vec<&str> = heads.iter().map(|h| h.2.as_str()).collect();
            return Err(Refusal::new(format!(
                "template {} has no heading {}; its headings are {}",
                quote(&a.name),
                quote(path),
                paths.join(", ")
            )));
        }
        for (depth, title, path) in heads {
            let body = a.fills.iter().find(|f| f.0 == path).map(|f| f.1.clone()).unwrap_or_default();
            news.push(New { depth: base + depth, title, body });
        }
    }
    Ok(news)
}

/// New headings: under a folder each in a new file (or a new directory with
/// `--dir`), under a file at its end, under a heading at the end of what it
/// holds, one level deeper.
pub fn add(g: &Graph, p: usize, children: &[Child], mode: AddMode, json: bool) -> R<Output> {
    let parent = &g.nodes[p];
    let parent_key = g.key(p);
    if children.is_empty() {
        return Err(Refusal::new("add needs at least one --title"));
    }
    let news = expand(g, children)?;
    for n in &news {
        validate_title(&n.title)?;
        validate_body(&n.body)?;
    }
    let mut taken = g.all_ids();
    let mut plans: Vec<Plan> = Vec::new();
    let mut born: Vec<(String, String, u8)> = Vec::new(); // (id, title, level), one per entry of `news`

    if parent.kind != Kind::Folder && mode == AddMode::Dir {
        return Err(Refusal::new(format!("--dir applies only to a folder; {parent_key} is a {}", parent.kind.name()))
            .instead(add_command(&parent_key, children, None)));
    }

    match parent.kind {
        Kind::Folder => {
            if let Some(a) = children.iter().find(|c| c.title.is_none()).and_then(|c| c.template.as_ref()) {
                return Err(Refusal::new(format!("a template without --title goes under a file or a heading; {parent_key} is a folder"))
                    .instead(format!("mdmap add {parent_key} --title <title> --template {}", dq(&a.name))));
            }
            let dir = parent.dir.clone().unwrap_or_default();
            let mut names = BTreeSet::new();
            let mut k = 0;
            while k < news.len() {
                let c = &news[k];
                let end = k + 1 + news[k + 1..].iter().take_while(|n| n.depth > 0).count();
                let s = slug(&c.title);
                if s.is_empty() {
                    return Err(Refusal::new(format!("the title {:?} gives an empty file name", c.title)));
                }

                let file = if mode == AddMode::Dir {
                    join(&join(&dir, &s), README)
                } else {
                    join(&dir, &format!("{s}.md"))
                };
                let shown = if mode == AddMode::Dir { join(&dir, &format!("{s}/")) } else { file.clone() };
                let on_disk = if mode == AddMode::Dir { g.project.join(&dir).join(&s) } else { g.project.join(&file) };
                if on_disk.exists() || !names.insert(shown.clone()) {
                    return Err(Refusal::new(format!("{shown} already exists")));
                }
                let mut text = String::new();
                for (j, n) in news[k..end].iter().enumerate() {
                    let hid = id::generate(&mut taken);
                    if j > 0 {
                        text.push('\n');
                    }
                    text.push_str(&(heading_line(1 + n.depth, &n.title, Some(&hid)) + "\n"));
                    text.push_str(&body_block(&n.body, false));
                    born.push((hid, n.title.clone(), 1 + n.depth));
                }
                plans.push(Plan::Create { file, text, heads: k..end });
                k = end;
            }
        }
        _ => {
            let file = parent.file.clone();
            let pos = {
                let doc = &g.docs[&file];
                match (parent.kind, parent.heading) {
                    (Kind::Section, Some(h)) => doc.headings[h + 1..]
                        .iter()
                        .find(|x| x.level <= parent.level)
                        .map(|x| x.start)
                        .unwrap_or(doc.text.len()),
                    _ => doc.text.len(),
                }
            };
            let level = parent.level + 1;
            let deepest = news.iter().map(|n| n.depth).max().unwrap_or(0);
            if level + deepest > 6 {
                let reason = if deepest == 0 {
                    format!("{parent_key} is at level 6; markdown has no level 7")
                } else {
                    format!(
                        "{parent_key} is at level {}; the new headings need {} levels below it and markdown stops at 6",
                        parent.level,
                        deepest + 1
                    )
                };
                let mut r = Refusal::new(reason);
                if let Some(gp) = parent.parent.map(|x| g.key(x)) {
                    r = r.instead(add_command(&gp, children, None));
                }
                return Err(r);
            }
            let doc = &g.docs[&file];
            if let Some(prob) = local_problems(&file, doc).into_iter().find(|p| p.structural()) {
                return Err(Refusal::new(format!(
                    "{file} has a check problem ({} at {}); mdmap cannot tell where the new node goes",
                    prob.message(),
                    prob.at
                )));
            }
            let followed = pos < doc.text.len();
            let mut block = String::new();
            for (k, n) in news.iter().enumerate() {
                let hid = id::generate(&mut taken);
                if k > 0 {
                    block.push('\n');
                }
                block.push_str(&(heading_line(level + n.depth, &n.title, Some(&hid)) + "\n"));
                block.push_str(&body_block(&n.body, false));
                born.push((hid, n.title.clone(), level + n.depth));
            }
            let at = doc.headings.iter().filter(|h| h.start < pos).count();
            let mut text = doc.text.clone();
            let insert = separated(&text[..pos], &block, followed);
            text.insert_str(pos, &insert);
            plans.push(Plan::Write { file, text, at });
        }
    }

    // Verify every file before writing any. `born` and `news` line up one to one.
    let sig = |k: usize| Sig {
        level: born[k].2,
        title: born[k].1.clone(),
        id: Some(born[k].0.clone()),
        body: Some(news[k].body.trim().to_string()),
    };
    for plan in &plans {
        match plan {
            Plan::Create { file, text, heads } => {
                let expected: Vec<Sig> = heads.clone().map(sig).collect();
                expect(file, text, &expected)?;
            }
            Plan::Write { file, text, at } => {
                let mut expected = sigs(&g.docs[file]);
                for k in 0..born.len() {
                    expected.insert(*at + k, sig(k));
                }
                expect(file, text, &expected)?;
            }
        }
    }

    let mut out_lines = Vec::new();
    for plan in &plans {
        match plan {
            Plan::Write { file, text, .. } | Plan::Create { file, text, .. } => {
                atomic_write(&g.project.join(file), text)?;
            }
        }
    }
    // Report where each new node landed.
    let mut rows = Vec::new();
    for (hid, title, level) in &born {
        let (file, line) = plans
            .iter()
            .find_map(|p| match p {
                Plan::Write { file, text, .. } | Plan::Create { file, text, .. } => {
                    let d = Doc::parse(text.clone());
                    d.headings.iter().find(|h| h.id.as_deref() == Some(hid)).map(|h| (file.clone(), h.line))
                }
            })
            .unwrap_or_default();
        out_lines.push(format!("added {} · {file}:{line}", heading_line(*level, title, Some(hid))));
        rows.push(json!({"id": hid, "heading": heading_line(*level, title, Some(hid)), "path": format!("{file}:{line}")}));
    }
    let mut out = if json {
        Output::text(serde_json::to_string_pretty(&json!({"added": rows})).unwrap() + "\n")
    } else {
        Output::text(out_lines.join("\n") + "\n")
    };
    out.touched.insert(parent.file.clone());
    Ok(out)
}

fn add_command(parent: &str, children: &[Child], flag: Option<&str>) -> String {
    let mut s = format!("mdmap add {parent}");
    if let Some(f) = flag {
        s.push(' ');
        s.push_str(f);
    }
    for c in children {
        if let Some(t) = &c.title {
            s.push_str(&format!(" --title {}", dq(t)));
        }
        if !c.body.is_empty() {
            s.push_str(&format!(" --body {}", dq(&c.body)));
        }
        if let Some(a) = &c.template {
            s.push_str(&format!(" --template {}", dq(&a.name)));
            for (path, text) in &a.fills {
                s.push_str(&format!(" --fill {} {}", dq(path), dq(text)));
            }
        }
    }
    s
}

pub fn no_such(id: &str) -> Refusal {
    Refusal::new(format!("no node with id {id}")).instead("mdmap ls")
}

pub struct UpdateArgs {
    pub title: Option<String>,
    pub body: Option<String>,
    pub lines: Option<(usize, usize)>,
    pub replace: Vec<(String, String)>,
}

/// A heading's title and body, or a file's body: the text above its first
/// heading. A folder has no text of its own.
pub fn update(g: &Graph, i: usize, a: &UpdateArgs, json: bool) -> R<Output> {
    let n = &g.nodes[i];
    let key = g.key(i);
    if n.kind == Kind::Folder {
        return Err(Refusal::new(format!("{key} is a folder; it has no text of its own")).instead(format!("mdmap ls {key}")));
    }
    if a.lines.is_some() && a.body.is_none() {
        return Err(Refusal::new("--lines needs --body: the text that replaces those lines"));
    }
    if a.title.is_none() && a.body.is_none() && a.replace.is_empty() {
        return Err(Refusal::new("update needs --title, --body or --old with --new"));
    }
    if !a.replace.is_empty() && a.body.is_some() {
        return Err(Refusal::new("--old and --new take the place of --body and --lines").instead("mdmap help"));
    }
    if a.title.is_some() && n.kind == Kind::File {
        return Err(Refusal::new(format!("{key} is a file: its name is its path; --title is for a heading")));
    }
    let doc = g.docs.get(&n.file).ok_or_else(|| Refusal::new(format!("{} does not exist", n.file)))?;
    let mut text = doc.text.clone();
    let mut expected = sigs(doc);

    // Body first: its offsets sit after the heading line.
    if let Some(body) = &a.body {
        validate_body(body)?;
        let r = n.body.clone();
        match a.lines {
            Some((from, to)) => {
                let Some((first, last)) = doc.lines_of(&r) else {
                    return Err(Refusal::new(format!("{key} has an empty body; there are no lines to replace"))
                        .instead(format!("mdmap update {key} --body {}", dq(body))));
                };
                if from > to || from < first || to > last {
                    return Err(Refusal::new(format!(
                        "lines {from}-{to} are outside the body of {key}, which is lines {first}-{last}"
                    ))
                    .instead(format!("mdmap show {key}")));
                }
                let start = doc.line_start(from);
                let end = doc.line_start(to + 1).min(r.end);
                let old = &text[start..end];
                let mut new = body.clone();
                if !new.is_empty() && old.ends_with('\n') && !new.ends_with('\n') {
                    new.push('\n');
                }
                text.replace_range(start..end, &new);
            }
            None => {
                let followed = r.end < doc.text.len();
                let mut block = body_block(body, followed);
                if n.kind == Kind::File {
                    // The text at the top of a file needs no blank line above it.
                    block = block.trim_start_matches('\n').to_string();
                } else if !text[..r.start].ends_with('\n') && !block.is_empty() {
                    block.insert(0, '\n');
                }
                text.replace_range(r.clone(), &block);
            }
        }
        // A file's body belongs to no heading: every heading keeps its body.
        if let Some(h) = n.heading {
            expected[h].body = None;
        }
    }
    // Each --old is found once in the body as it stands; all pairs are replaced, or none.
    if !a.replace.is_empty() {
        let r = n.body.clone();
        let mut spans = Vec::new();
        for (old, new) in &a.replace {
            if old.is_empty() {
                return Err(Refusal::new("--old needs the text it replaces; it is empty"));
            }
            // Every place the text starts, overlapping ones too: "aa" is in "aaa" twice.
            let body = &text[r.clone()];
            let found: Vec<usize> =
                body.char_indices().filter(|(s, _)| body[*s..].starts_with(old.as_str())).map(|(s, _)| r.start + s).collect();
            if found.len() != 1 {
                let more = if found.is_empty() { "" } else { "; give more of the text around it" };
                return Err(Refusal::new(format!(
                    "--old {} is found {} times in the body of {key}, not once{more}",
                    dq(&crate::out::truncate(old, 60)),
                    found.len()
                ))
                .instead(format!("mdmap show {key}")));
            }
            spans.push((found[0], found[0] + old.len(), new.as_str()));
        }
        spans.sort_by_key(|s| s.0);
        if spans.windows(2).any(|w| w[1].0 < w[0].1) {
            return Err(Refusal::new(format!("two --old texts overlap in the body of {key}; join them into one")));
        }
        let mut end = r.end;
        for &(s, e, new) in spans.iter().rev() {
            text.replace_range(s..e, new);
            end = end + new.len() - (e - s);
        }
        validate_body(&text[r.start..end])?;
        if let Some(h) = n.heading {
            expected[h].body = None;
        }
    }
    if let Some(title) = &a.title {
        validate_title(title)?;
        let h = n.heading.ok_or_else(|| Refusal::new(format!("{key} has no heading line")))?;
        let hd = &doc.headings[h];
        if hd.setext {
            return Err(Refusal::new(format!("{key} is a setext heading; run mdmap check --fix")));
        }
        let nl = if text[hd.start..hd.end].ends_with("\r\n") {
            "\r\n"
        } else if text[hd.start..hd.end].ends_with('\n') {
            "\n"
        } else {
            ""
        };
        text.replace_range(hd.start..hd.end, &(heading_line(hd.level, title.trim(), n.id.as_deref()) + nl));
        expected[h].title = title.trim().to_string();
    }

    let new_doc = expect(&n.file, &text, &expected)?;
    atomic_write(&g.project.join(&n.file), &text)?;
    let range = match n.heading {
        Some(h) => new_doc.body_of(h),
        None => new_doc.content_start..new_doc.headings.first().map(|x| x.start).unwrap_or(new_doc.text.len()),
    };
    let lines = new_doc.lines_of(&range);
    let shown = match lines {
        Some((a, b)) => format!("lines {a}-{b}"),
        None => "no body lines".to_string(),
    };
    let mut out = if json {
        Output::text(
            serde_json::to_string_pretty(&json!({"updated": key, "lines": lines.map(|(a, b)| vec![a, b])})).unwrap()
                + "\n",
        )
    } else {
        Output::text(format!("updated {key} · {shown}\n"))
    };
    out.touched.insert(n.file.clone());
    Ok(out)
}

/// A file whose last bytes were taken out: one newline at its end, and
/// nothing at all when nothing is left of it.
fn ends_clean(text: &str) -> String {
    let t = text.trim_end();
    if t.is_empty() {
        String::new()
    } else {
        format!("{t}\n")
    }
}

/// Where the headings under heading `h` of `doc` end: at the next heading of
/// its level or higher, or at the end of the file.
fn subtree_end(doc: &Doc, h: usize) -> usize {
    let level = doc.headings[h].level;
    doc.headings[h + 1..].iter().find(|x| x.level <= level).map(|x| x.start).unwrap_or(doc.text.len())
}

/// `move`: a heading with the headings under it to the end of a file or of
/// another heading, one level below it, every id kept, so links follow.
pub fn move_node(g: &Graph, i: usize, p: usize, json: bool) -> R<Output> {
    let (n, to) = (&g.nodes[i], &g.nodes[p]);
    let (key, to_key) = (g.key(i), g.key(p));
    if n.kind != Kind::Section {
        return Err(Refusal::new(format!("{key} is a {}; move takes a heading", n.kind.name())));
    }
    if to.kind == Kind::Folder {
        return Err(Refusal::new(format!("{to_key} is a folder; a heading moves under a file or a heading")));
    }
    if p == i || g.descendants(i).contains(&p) {
        return Err(Refusal::new(format!("{to_key} is inside {key}; a heading cannot move under itself")));
    }
    for file in [&n.file, &to.file] {
        if let Some(prob) = local_problems(file, &g.docs[file]).into_iter().find(|p| p.structural()) {
            return Err(Refusal::new(format!("{file} has a check problem ({} at {})", prob.message(), prob.at)));
        }
    }
    let doc = &g.docs[&n.file];
    let h = n.heading.unwrap();
    let range = doc.headings[h].start..subtree_end(doc, h);
    let moved = doc.headings[h..].iter().take_while(|x| x.start < range.end).count();
    let level = to.level + 1;
    let deepest = doc.headings[h..h + moved].iter().map(|x| x.level).max().unwrap_or(n.level);
    if level + (deepest - n.level) > 6 {
        return Err(Refusal::new(format!(
            "{to_key} is at level {}; {key} and the headings under it need {} levels below it and markdown stops at 6",
            to.level,
            deepest - n.level + 1
        )));
    }
    // The block, each heading at its new level; the text between them as it was.
    let mut block = String::new();
    let mut last = range.start;
    let mut shifted = Vec::new();
    for (k, x) in doc.headings[h..h + moved].iter().enumerate() {
        let new_level = x.level - n.level + level;
        block.push_str(&doc.text[last..x.start]);
        block.push_str(&(heading_line(new_level, &x.title, x.id.as_deref()) + "\n"));
        last = x.end;
        shifted.push(Sig { level: new_level, title: x.title.clone(), id: x.id.clone(), body: Some(doc.text[doc.body_of(h + k)].trim().to_string()) });
    }
    block.push_str(&doc.text[last..range.end]);
    let block = block.trim_end().to_string() + "\n";

    // Where the block goes, in the target file as it stands.
    let pos = match to.heading {
        Some(t) => subtree_end(&g.docs[&to.file], t),
        None => g.docs[&to.file].text.len(),
    };
    let same = to.file == n.file;
    let mut writes: Vec<(String, String)> = Vec::new();
    let mut source = doc.text.clone();
    source.replace_range(range.clone(), "");
    if range.end == doc.text.len() {
        source = ends_clean(&source);
    }
    let mut source_sigs = sigs(doc);
    source_sigs.drain(h..h + moved);
    let (mut text, mut expected, at) = if same {
        let at = doc.headings.iter().enumerate().filter(|(k, x)| (*k < h || *k >= h + moved) && x.start < pos).count();
        (source.clone(), source_sigs.clone(), at)
    } else {
        writes.push((n.file.clone(), source.clone()));
        let tdoc = &g.docs[&to.file];
        (tdoc.text.clone(), sigs(tdoc), tdoc.headings.iter().filter(|x| x.start < pos).count())
    };
    let pos = if same && pos >= range.end { pos - range.len() } else { pos }.min(text.len());
    let followed = pos < text.len();
    let insert = separated(&text[..pos], &block, followed);
    text.insert_str(pos, &insert);
    for (k, s) in shifted.into_iter().enumerate() {
        expected.insert(at + k, s);
    }
    if !same {
        expect(&n.file, &source, &source_sigs)?;
    }
    let written = expect(&to.file, &text, &expected)?;
    writes.push((to.file.clone(), text));
    for (file, text) in &writes {
        atomic_write(&g.project.join(file), text)?;
    }
    let line = written.headings.iter().find(|x| x.id == n.id).map(|x| x.line).unwrap_or(0);
    let heading = heading_line(level, &n.title, n.id.as_deref());
    let mut out = if json {
        Output::text(serde_json::to_string_pretty(&json!({"moved": key, "to": to_key, "nodes": moved, "path": format!("{}:{line}", to.file)})).unwrap() + "\n")
    } else {
        Output::text(format!("moved {heading} · {moved} node{} · {}:{line}\n", if moved == 1 { "" } else { "s" }, to.file))
    };
    out.touched.insert(n.file.clone());
    out.touched.insert(to.file.clone());
    Ok(out)
}

/// A heading with the headings under it, a file, or a folder with all it
/// holds.
pub fn remove(g: &Graph, i: usize, json: bool) -> R<Output> {
    let n = &g.nodes[i];
    let key = g.key(i);
    let count = g.descendants(i).len();
    let heading = g.heading(i);
    match n.kind {
        Kind::Folder => {
            if n.parent.is_none() {
                return Err(Refusal::new(format!("{key} is a root; remove it from roots in {MDMAP_PATH}")));
            }
            let dir = n.dir.clone().unwrap_or_default();
            std::fs::remove_dir_all(g.project.join(&dir))?;
        }
        Kind::File => {
            std::fs::remove_file(g.project.join(&n.file))?;
        }
        Kind::Section => {
            let doc = &g.docs[&n.file];
            let h = n.heading.unwrap();
            let end = doc.headings[h + 1..]
                .iter()
                .find(|x| x.level <= n.level)
                .map(|x| x.start)
                .unwrap_or(doc.text.len());
            let start = doc.headings[h].start;
            let mut text = doc.text.clone();
            text.replace_range(start..end, "");
            // The blank line above a heading belongs to the heading: the last
            // section of a file would leave it behind at the end of the file.
            if end == doc.text.len() {
                text = ends_clean(&text);
            }
            let gone = doc.headings[h..].iter().take_while(|x| x.start < end).count();
            let mut expected = sigs(doc);
            expected.drain(h..h + gone);
            // The node before the removed range keeps its text; its trimmed
            // body is unchanged.
            expect(&n.file, &text, &expected)?;
            atomic_write(&g.project.join(&n.file), &text)?;
        }
    }
    let mut out = if json {
        Output::text(serde_json::to_string_pretty(&json!({"removed": key, "nodes": count})).unwrap() + "\n")
    } else {
        Output::text(format!("removed {heading} · {count} node{}\n", if count == 1 { "" } else { "s" }))
    };
    if !n.file.is_empty() {
        out.touched.insert(n.file.clone());
    }
    Ok(out)
}
