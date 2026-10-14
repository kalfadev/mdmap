//! The corpus as one tree: folders, files and headings.
//!
//! A folder is a directory: its name, no body, no id; its children are its
//! `.md` files, then its subdirectories. A file is a node with no id either:
//! its body is the text before its first heading, and its children are the
//! headings under no other heading. A heading is a node with an id, and its
//! children are the headings under it: a heading belongs to the nearest
//! heading above it with fewer `#`, so a `###` may stand right under a `#`.
//! Folders and files are named by their path, headings by their id.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use crate::id;
use crate::out::{Refusal, R};

use crate::config::Config;
use crate::markdown::{heading_line, Doc};

pub const README: &str = "README.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Folder,
    File,
    Section,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Folder => "folder",
            Kind::File => "file",
            Kind::Section => "section",
        }
    }
}

/// What `deny` of `mdmap.yml` closes a node to; `Read` closes it to every verb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Read,
    Update,
    Remove,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: Kind,
    /// The number of `#` of a heading; 0 for a folder or a file.
    pub level: u8,
    /// A heading's title; the name of a folder or a file.
    pub title: String,
    /// A heading's id; folders and files have none.
    pub id: Option<String>,
    /// The markdown file the node lives in, relative to the project; empty
    /// for a folder.
    pub file: String,
    /// For a folder, its directory relative to the project.
    pub dir: Option<String>,
    /// Index into the file's headings; None for a folder or a file.
    pub heading: Option<usize>,
    /// Byte range of the body inside the file.
    pub body: Range<usize>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub links: Vec<usize>,
    pub backlinks: Vec<usize>,
    pub body_chars: usize,
    pub subtree: usize,
    pub nodes: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone)]
pub enum LinkTarget {
    Node(usize),
    /// Resolves, but the path in the link is not the node's file.
    Stale(usize),
    Broken(String),
    /// Outside the graph: external, outside the roots, not markdown.
    Outside,
}

#[derive(Debug, Clone)]
pub struct ResolvedLink {
    pub from: usize,
    pub file: String,
    pub line: usize,
    pub dest: String,
    pub target: LinkTarget,
}

#[derive(Debug)]
pub struct Graph {
    pub project: PathBuf,
    pub config: Config,
    pub nodes: Vec<Node>,
    pub roots: Vec<usize>,
    /// Parsed markdown files, by path relative to the project.
    pub docs: BTreeMap<String, Doc>,
    pub by_id: HashMap<String, usize>,
    pub resolved: Vec<ResolvedLink>,
    ignore: GlobSet,
    /// The pattern of `ignore` each glob of the set came from, and whether a
    /// path of the walk matched that pattern.
    ignore_of: Vec<usize>,
    ignore_hit: Vec<Cell<bool>>,
    /// For each node, whether `deny` closes it to read, update and remove, in
    /// the order of [`Act`]; a node under a closed one is closed as well.
    deny: Vec<[bool; 3]>,
}

impl Graph {
    pub fn load(project: &Path, config: Config) -> R<Graph> {
        let (ignore, ignore_of) = build_ignore(&config.ignore)?;
        let ignore_hit = config.ignore.iter().map(|_| Cell::new(false)).collect();
        let mut g = Graph {
            project: project.to_path_buf(),
            config,
            nodes: Vec::new(),
            roots: Vec::new(),
            docs: BTreeMap::new(),
            by_id: HashMap::new(),
            resolved: Vec::new(),
            ignore,
            ignore_of,
            ignore_hit,
            deny: Vec::new(),
        };
        for root in g.config.root_dirs() {
            if g.nodes.iter().any(|n| n.dir.as_deref() == Some(root.as_str())) {
                continue;
            }
            let idx = g.build_dir(&root, &root, None)?;
            g.roots.push(idx);
        }
        // Before finish: the sizes it counts leave out what `deny.read` closes.
        g.deny = g.denials()?;
        g.finish();
        Ok(g)
    }

    fn root_of<'a>(&self, rel: &'a str, root: &str) -> &'a str {
        if root == "." {
            return rel;
        }
        rel.strip_prefix(root).map(|r| r.trim_start_matches('/')).unwrap_or(rel)
    }

    fn ignored(&self, rel: &str, root: &str) -> bool {
        let inside = self.root_of(rel, root);
        if inside.is_empty() {
            return false;
        }
        let hits = self.ignore.matches(inside);
        for &g in &hits {
            self.ignore_hit[self.ignore_of[g]].set(true);
        }
        !hits.is_empty()
    }

    /// The patterns of `ignore` no path of the walk matched: after a load,
    /// those that hide nothing.
    pub fn unused_ignores(&self) -> Vec<&str> {
        self.config.ignore.iter().zip(&self.ignore_hit).filter(|(_, hit)| !hit.get()).map(|(p, _)| p.as_str()).collect()
    }

    /// Whether `deny` of `mdmap.yml` closes node `i` to `act`; a read-denied
    /// node is closed to every verb.
    pub fn denied(&self, i: usize, act: Act) -> bool {
        let d = self.deny[i];
        d[Act::Read as usize] || d[act as usize]
    }

    /// Whether `deny` closes any node of the subtree of `i` to `act`.
    pub fn denied_under(&self, i: usize, act: Act) -> bool {
        self.descendants(i).into_iter().any(|x| self.denied(x, act))
    }

    /// The entries of `deny` that name no node, and so close nothing: an id no
    /// heading has, a path no folder or file matches.
    pub fn unused_denies(&self) -> Vec<String> {
        let paths: Vec<&str> = self.nodes.iter().filter_map(node_path).collect();
        let mut out = Vec::new();
        for (verb, list) in self.config.deny.lists() {
            for p in &list.paths {
                let hit = build_deny(verb, std::slice::from_ref(p)).is_ok_and(|set| paths.iter().any(|x| set.is_match(x)));
                if !hit {
                    out.push(format!("deny.{verb}.paths {p}"));
                }
            }
            for id in list.ids.iter().filter(|id| !self.by_id.contains_key(*id)) {
                out.push(format!("deny.{verb}.ids {id}"));
            }
        }
        out
    }

    /// What `deny` closes each node to: a folder or a file matched on its
    /// path, a heading on its id, and every node under it the same.
    fn denials(&self) -> R<Vec<[bool; 3]>> {
        let lists = self.config.deny.lists();
        let mut sets = Vec::new();
        for (verb, list) in &lists {
            sets.push(build_deny(verb, &list.paths)?);
        }
        let mut deny = vec![[false; 3]; self.nodes.len()];
        let mut stack: Vec<(usize, [bool; 3])> = self.roots.iter().map(|&r| (r, [false; 3])).collect();
        while let Some((i, mut d)) = stack.pop() {
            let n = &self.nodes[i];
            let path = node_path(n);
            for (k, (_, list)) in lists.iter().enumerate() {
                d[k] |= match path {
                    Some(p) => sets[k].is_match(p),
                    None => n.id.as_ref().is_some_and(|id| list.ids.contains(id)),
                };
            }
            deny[i] = d;
            stack.extend(n.children.iter().map(|&c| (c, d)));
        }
        Ok(deny)
    }

    fn read_doc(&mut self, rel: &str) -> R<()> {
        if !self.docs.contains_key(rel) {
            let text = std::fs::read_to_string(self.project.join(rel))
                .map_err(|e| Refusal::new(format!("cannot read {rel}: {e}")))?;
            self.docs.insert(rel.to_string(), Doc::parse(text));
        }
        Ok(())
    }

    /// Markdown files and subdirectories of a directory that belong to the
    /// graph: files first, then directories.
    fn entries(&self, dir: &str, root: &str) -> R<Vec<(String, bool)>> {
        let abs = self.project.join(dir);
        let mut out = Vec::new();
        let Ok(read) = std::fs::read_dir(&abs) else { return Ok(out) };
        for entry in read {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let rel = join(dir, &name);
            if self.ignored(&rel, root) {
                continue;
            }
            let ft = entry.file_type()?;
            let ft_dir = ft.is_dir() || (ft.is_symlink() && entry.path().is_dir());
            if ft_dir {
                if self.has_markdown(&rel, root) {
                    out.push((rel, true));
                }
            } else if name.ends_with(".md") {
                out.push((rel, false));
            }
        }
        // Files first, then directories, each in path order.
        out.sort_by(|a, b| (a.1, &a.0).cmp(&(b.1, &b.0)));
        Ok(out)
    }

    fn has_markdown(&self, dir: &str, root: &str) -> bool {
        let Ok(read) = std::fs::read_dir(self.project.join(dir)) else { return false };
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let rel = join(dir, &name);
            if self.ignored(&rel, root) {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                if self.has_markdown(&rel, root) {
                    return true;
                }
            } else if name.ends_with(".md") {
                return true;
            }
        }
        false
    }

    fn build_dir(&mut self, dir: &str, root: &str, parent: Option<usize>) -> R<usize> {
        let idx = self.nodes.len();
        let dir_name = Path::new(dir)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| dir.to_string());
        self.nodes.push(Node {
            kind: Kind::Folder,
            level: 0,
            title: dir_name,
            id: None,
            file: String::new(),
            dir: Some(dir.to_string()),
            heading: None,
            body: 0..0,
            parent,
            children: Vec::new(),
            links: Vec::new(),
            backlinks: Vec::new(),
            body_chars: 0,
            subtree: 0,
            nodes: 0,
            max_depth: 0,
        });
        for (rel, is_dir) in self.entries(dir, root)? {
            let child = if is_dir {
                self.build_dir(&rel, root, Some(idx))?
            } else {
                self.read_doc(&rel)?;
                let c = self.push_file_node(&rel, Some(idx));
                self.build_sections(&rel, c);
                c
            };
            self.nodes[idx].children.push(child);
        }
        Ok(idx)
    }

    /// The node for a whole file: its name, and the text before its first
    /// heading as its body.
    fn push_file_node(&mut self, file: &str, parent: Option<usize>) -> usize {
        let doc = &self.docs[file];
        let end = doc.headings.first().map(|h| h.start).unwrap_or(doc.text.len());
        let body = doc.content_start..end.max(doc.content_start);
        let title = Path::new(file).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let idx = self.nodes.len();
        self.nodes.push(Node {
            kind: Kind::File,
            level: 0,
            title,
            id: None,
            file: file.to_string(),
            dir: None,
            heading: None,
            body,
            parent,
            children: Vec::new(),
            links: Vec::new(),
            backlinks: Vec::new(),
            body_chars: 0,
            subtree: 0,
            nodes: 0,
            max_depth: 0,
        });
        idx
    }

    fn build_sections(&mut self, file: &str, file_node: usize) {
        let doc = &self.docs[file];
        let mut stack: Vec<(u8, usize)> = Vec::new();
        let mut new_nodes = Vec::new();
        for i in 0..doc.headings.len() {
            let h = &doc.headings[i];
            while stack.last().is_some_and(|&(l, _)| l >= h.level) {
                stack.pop();
            }
            let parent = stack.last().map(|&(_, n)| n).unwrap_or(file_node);
            let idx = self.nodes.len() + new_nodes.len();
            new_nodes.push(Node {
                kind: Kind::Section,
                level: h.level,
                title: h.title.clone(),
                id: h.id.clone(),
                file: file.to_string(),
                dir: None,
                heading: Some(i),
                body: doc.body_of(i),
                parent: Some(parent),
                children: Vec::new(),
                links: Vec::new(),
                backlinks: Vec::new(),
                body_chars: 0,
                subtree: 0,
                nodes: 0,
                max_depth: 0,
            });
            stack.push((h.level, idx));
        }
        let base = self.nodes.len();
        for (k, n) in new_nodes.into_iter().enumerate() {
            let p = n.parent.unwrap();
            self.nodes.push(n);
            self.nodes[p].children.push(base + k);
        }
    }

    fn finish(&mut self) {
        // Ids: when two headings share one, the first in path order owns it.
        let mut order: Vec<usize> = (0..self.nodes.len()).collect();
        order.sort_by_key(|&i| (self.nodes[i].file.clone(), self.line(i).unwrap_or(0)));
        for i in order {
            if let Some(id) = &self.nodes[i].id {
                self.by_id.entry(id.clone()).or_insert(i);
            }
        }

        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            if let Some(doc) = self.docs.get(&n.file) {
                self.nodes[i].body_chars = doc.text[n.body.clone()].chars().count();
            }
        }

        // Links, in body order.
        let mut resolved = Vec::new();
        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            let Some(doc) = self.docs.get(&n.file) else { continue };
            for l in doc.links.iter().filter(|l| l.start >= n.body.start && l.start < n.body.end) {
                let target = self.resolve(&n.file, &l.dest);
                resolved.push(ResolvedLink {
                    from: i,
                    file: n.file.clone(),
                    line: l.line,
                    dest: l.dest.clone(),
                    target,
                });
            }
        }
        for r in &resolved {
            if let LinkTarget::Node(t) | LinkTarget::Stale(t) = r.target {
                if !self.nodes[r.from].links.contains(&t) {
                    self.nodes[r.from].links.push(t);
                }
                if !self.nodes[t].backlinks.contains(&r.from) {
                    self.nodes[t].backlinks.push(r.from);
                }
            }
        }
        self.resolved = resolved;

        for &r in &self.roots.clone() {
            self.sizes(r);
        }
    }

    fn sizes(&mut self, i: usize) {
        let mut subtree = self.nodes[i].body_chars;
        let mut nodes = 1;
        let mut depth = 0;
        for c in self.nodes[i].children.clone() {
            self.sizes(c);
            // A read-denied child is one node with no body and nothing under it:
            // a size would tell what is there.
            if self.denied(c, Act::Read) {
                nodes += 1;
                depth = depth.max(1);
                continue;
            }
            subtree += self.nodes[c].subtree;
            nodes += self.nodes[c].nodes;
            depth = depth.max(self.nodes[c].max_depth + 1);
        }
        let n = &mut self.nodes[i];
        n.subtree = subtree;
        n.nodes = nodes;
        n.max_depth = depth;
    }

    /// Where a link written in `from_file` goes.
    pub fn resolve(&self, from_file: &str, dest: &str) -> LinkTarget {
        let dest = dest.trim();
        if dest.is_empty() || dest.contains("://") || dest.starts_with("mailto:") {
            return LinkTarget::Outside;
        }
        let (path, fragment) = match dest.split_once('#') {
            Some((p, f)) => (p, Some(f)),
            None => (dest, None),
        };
        let path = path.split('?').next().unwrap_or("");
        let path = percent_decode(path);
        let target_path = if path.is_empty() {
            None
        } else {
            let base = if let Some(abs) = path.strip_prefix('/') {
                abs.to_string()
            } else {
                let dir = parent_dir(from_file);
                join(&dir, &path)
            };
            match normalize(&base) {
                Some(p) => Some(p),
                None => return LinkTarget::Outside,
            }
        };
        if let Some(p) = &target_path {
            if !self.in_roots(p) {
                return LinkTarget::Outside;
            }
        }
        if let Some(f) = fragment {
            if !id::is_id(f) {
                if target_path.as_deref().is_some_and(|p| !p.ends_with(".md") && !self.is_dir_node(p)) {
                    return LinkTarget::Outside;
                }
                return LinkTarget::Broken("fragment is not an id".into());
            }
            let Some(&t) = self.by_id.get(f) else {
                return LinkTarget::Broken("no such id".into());
            };
            if let Some(p) = &target_path {
                if !self.node_is_at(t, p) {
                    return LinkTarget::Stale(t);
                }
            }
            return LinkTarget::Node(t);
        }
        let Some(p) = target_path else { return LinkTarget::Outside };
        if let Some(t) = self.node_at_path(&p) {
            return LinkTarget::Node(t);
        }
        if p.ends_with(".md") || self.project.join(&p).is_dir() {
            if self.project.join(&p).exists() {
                // Present but ignored.
                return LinkTarget::Outside;
            }
            return LinkTarget::Broken("no such file".into());
        }
        LinkTarget::Outside
    }

    fn is_dir_node(&self, p: &str) -> bool {
        self.nodes.iter().any(|n| n.dir.as_deref() == Some(p))
    }

    fn node_is_at(&self, t: usize, p: &str) -> bool {
        let n = &self.nodes[t];
        n.file == p || n.dir.as_deref() == Some(p)
    }

    /// The file or folder node for a path.
    pub fn node_at_path(&self, p: &str) -> Option<usize> {
        let p = p.trim_start_matches("./").trim_end_matches('/');
        self.nodes.iter().position(|n| (n.kind == Kind::File && n.file == p) || n.dir.as_deref() == Some(p))
    }

    /// The node an argument names: a heading by its id, a folder or a file
    /// by its path.
    pub fn find(&self, key: &str) -> Option<usize> {
        if id::is_id(key) {
            return self.node(key);
        }
        self.node_at_path(key)
    }

    /// What names a node on the command line: its id, or its path.
    pub fn key(&self, i: usize) -> String {
        let n = &self.nodes[i];
        match n.kind {
            Kind::Section => n.id.clone().unwrap_or_else(|| self.location(i)),
            Kind::File => n.file.clone(),
            Kind::Folder => format!("{}/", n.dir.clone().unwrap_or_default()),
        }
    }

    pub fn in_roots(&self, p: &str) -> bool {
        self.config.root_dirs().iter().any(|r| {
            let inside = r == "." || p == r || p.starts_with(&format!("{r}/"));
            inside && !self.ignored(p, r)
        })
    }

    pub fn node(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    pub fn doc(&self, i: usize) -> Option<&Doc> {
        self.docs.get(&self.nodes[i].file)
    }

    /// 1-based line of the node's heading.
    pub fn line(&self, i: usize) -> Option<usize> {
        let n = &self.nodes[i];
        let h = n.heading?;
        Some(self.docs.get(&n.file)?.headings[h].line)
    }

    pub fn body_text(&self, i: usize) -> &str {
        match self.doc(i) {
            Some(d) => &d.text[self.nodes[i].body.clone()],
            None => "",
        }
    }

    pub fn body_lines(&self, i: usize) -> Option<(usize, usize)> {
        self.doc(i)?.lines_of(&self.nodes[i].body)
    }

    /// The characters of the body a reader is given: the fenced block under a
    /// `<!-- verify: … -->` comment is the output of that command, which `show`
    /// counts instead of printing (`view::body_block`), so the size warnings
    /// answer for the prose and leave that block out.
    pub fn prose_chars(&self, i: usize) -> usize {
        let text = self.body_text(i);
        let mut hidden = 0;
        let mut after_verify = false;
        let mut lines = text.lines();
        while let Some(l) = lines.next() {
            let head = l.trim_start();
            if head.starts_with("<!-- verify:") {
                after_verify = true;
            } else if after_verify && head.starts_with("```") {
                after_verify = false;
                let (mut inside, mut closed) = (0, false);
                for inner in lines.by_ref() {
                    if inner.trim_start().starts_with("```") {
                        closed = true;
                        break;
                    }
                    inside += inner.chars().count() + 1;
                }
                // A fence that never closes holds no block to count: show gives those lines to the reader.
                if closed {
                    hidden += inside;
                }
            } else if after_verify && !head.is_empty() {
                after_verify = false;
            }
        }
        self.nodes[i].body_chars.saturating_sub(hidden)
    }

    pub fn summary(&self, i: usize) -> Option<String> {
        self.doc(i)?.summary_in(&self.nodes[i].body)
    }

    /// A heading as it reads on disk (level, title, id); a file's path; a
    /// folder's path with a slash.
    pub fn heading(&self, i: usize) -> String {
        let n = &self.nodes[i];
        match n.kind {
            Kind::Section => heading_line(n.level, &n.title, n.id.as_deref()),
            _ => self.key(i),
        }
    }

    /// `docs/arch.md:34` for a heading, the path for a file or a folder.
    pub fn location(&self, i: usize) -> String {
        let n = &self.nodes[i];
        match (n.kind, self.line(i)) {
            (Kind::Section, Some(l)) => format!("{}:{l}", n.file),
            (Kind::Folder, _) => format!("{}/", n.dir.clone().unwrap_or_default()),
            _ => n.file.clone(),
        }
    }

    /// Every id in the corpus.
    pub fn all_ids(&self) -> HashSet<String> {
        let mut ids = HashSet::new();
        for d in self.docs.values() {
            for h in &d.headings {
                if let Some(id) = &h.id {
                    ids.insert(id.clone());
                }
            }
        }
        ids
    }

    /// The node and everything below it, in tree order.
    pub fn descendants(&self, i: usize) -> Vec<usize> {
        let mut out = vec![i];
        let mut k = 0;
        while k < out.len() {
            let c = self.nodes[out[k]].children.clone();
            out.extend(c);
            k += 1;
        }
        out
    }

    /// Files a set of nodes live in, for the check warning.
    pub fn files_of(&self, nodes: &[usize]) -> Vec<String> {
        nodes.iter().map(|&i| self.nodes[i].file.clone()).collect()
    }
}

/// The globs of `ignore`, and for each glob the index of its pattern.
fn build_ignore(patterns: &[String]) -> R<(GlobSet, Vec<usize>)> {
    let mut b = GlobSetBuilder::new();
    let mut of = Vec::new();
    for (i, p) in patterns.iter().enumerate() {
        let p = p.trim_start_matches("./");
        let glob = path_glob(p).map_err(|e| Refusal::new(format!("bad ignore pattern {p}: {e}")))?;
        b.add(glob);
        of.push(i);
        // `drafts/**` should also hide the directory itself.
        if let Some(dir) = p.strip_suffix("/**") {
            if let Ok(g) = path_glob(dir) {
                b.add(g);
                of.push(i);
            }
        }
    }
    let set = b.build().map_err(|e| Refusal::new(format!("bad ignore patterns: {e}")))?;
    Ok((set, of))
}

/// The globs of one `deny` list, relative to the project; `docs/x/**` also
/// matches the folder `docs/x`.
fn build_deny(verb: &str, patterns: &[String]) -> R<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        let p = p.trim_start_matches("./").trim_end_matches("/");
        let glob = path_glob(p).map_err(|e| Refusal::new(format!("bad deny.{verb} pattern {p}: {e}")))?;
        b.add(glob);
        if let Some(dir) = p.strip_suffix("/**") {
            if let Ok(g) = path_glob(dir) {
                b.add(g);
            }
        }
    }
    b.build().map_err(|e| Refusal::new(format!("bad deny.{verb} patterns: {e}")))
}

/// A glob of `ignore` or `deny`: `*` stays inside one folder and `**` crosses
/// folders, as in `.gitignore`.
fn path_glob(p: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(p).literal_separator(true).build()
}

/// The path a `deny` glob is matched on: of a folder or a file; a heading has none.
fn node_path(n: &Node) -> Option<&str> {
    match n.kind {
        Kind::Section => None,
        Kind::File => Some(n.file.as_str()),
        Kind::Folder => n.dir.as_deref(),
    }
}

pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() || dir == "." {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

pub fn parent_dir(file: &str) -> String {
    match file.rfind('/') {
        Some(i) => file[..i].to_string(),
        None => ".".to_string(),
    }
}

/// Resolve `.` and `..`; None when the path climbs above the project.
pub fn normalize(p: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    if parts.is_empty() {
        return Some(".".into());
    }
    Some(parts.join("/"))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
