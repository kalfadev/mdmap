//! `mdmap.yml` at the root of the project: the roots of the corpus, what to
//! leave out of it, the heading trees `add --template` writes, when a body is
//! too long, and what a verify line may run. A project without the file reads
//! the defaults.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::out::{Refusal, R};
use serde::Deserialize;

/// The settings of a project: the fields of [`Config`] at the top level of the
/// file.
pub const MDMAP_PATH: &str = "mdmap.yml";
/// Where the other words of a verify line go in the command of its entry.
pub const ARGS: &str = "{args}";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_roots")]
    pub roots: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Heading trees `add --template` writes, by name.
    #[serde(default)]
    pub templates: BTreeMap<String, Vec<TemplateItem>>,
    #[serde(default)]
    pub warn: WarnConfig,
    #[serde(default)]
    pub verify: VerifyConfig,
    /// The nodes a verb may not reach.
    #[serde(default)]
    pub deny: DenyConfig,
}

/// The nodes each verb may not reach, and all under them. `read` closes a
/// node to every verb, since nothing is written blind.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DenyConfig {
    /// Closed to `show`, `ls`, `search`, `stats`, `export` and every write.
    #[serde(default)]
    pub read: DenyList,
    /// Closed to `update`, and to `add` or `move` into it.
    #[serde(default)]
    pub update: DenyList,
    /// Closed to `remove`, and to `move` out of its place.
    #[serde(default)]
    pub remove: DenyList,
}

/// One list of `deny`: globs relative to the project, matched on the path of
/// a folder or a file, and heading ids, each in a list of its own.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DenyList {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub ids: Vec<String>,
}

impl DenyConfig {
    /// The lists by name, in the order of `graph::Act`.
    pub fn lists(&self) -> [(&str, &DenyList); 3] {
        [("read", &self.read), ("update", &self.update), ("remove", &self.remove)]
    }
}

/// What a verify line may run.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyConfig {
    /// Matched on the first words of a line: an entry is the words alone, and
    /// the line runs as written, or `{command, run, timeout}`, and `run` runs
    /// with the other words of the line where `{args}` is.
    #[serde(default)]
    pub run: Vec<Allow>,
    /// Seconds a verify command may run, unless its own entry says otherwise.
    #[serde(default = "verify_timeout")]
    pub timeout: u64,
}

fn verify_timeout() -> u64 {
    120
}

impl Default for VerifyConfig {
    fn default() -> Self {
        VerifyConfig { run: Vec::new(), timeout: verify_timeout() }
    }
}

/// One entry of `verify.run`: the words a line must start with, what runs for
/// them, and how long it may take.
#[derive(Debug, Clone, Deserialize)]
#[serde(from = "Entry")]
pub struct Allow {
    /// Matched word by word against the start of the line.
    pub command: String,
    /// What runs, with `{args}` taking the other words of the line; without a
    /// command of its own the line runs as it is written.
    pub run: Option<String>,
    pub timeout: Option<u64>,
}

/// An entry is the words themselves, or a map of `command`, `run` and
/// `timeout`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Entry {
    Words(String),
    Full(Full),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Full {
    command: String,
    #[serde(default)]
    run: Option<String>,
    #[serde(default)]
    timeout: Option<u64>,
}

impl From<Entry> for Allow {
    fn from(e: Entry) -> Allow {
        match e {
            Entry::Words(command) => Allow { command, run: None, timeout: None },
            Entry::Full(f) => Allow { command: f.command, run: f.run, timeout: f.timeout },
        }
    }
}

/// Sizes above which a call starts with advice.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarnConfig {
    /// Characters in one node's body; 0 turns the warning off.
    #[serde(default = "warn_body")]
    pub body: usize,
}

fn default_roots() -> Vec<String> {
    vec!["docs/".to_string()]
}

fn warn_body() -> usize {
    8_000
}

impl Default for WarnConfig {
    fn default() -> Self {
        WarnConfig { body: warn_body() }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config { roots: default_roots(), ignore: Vec::new(), templates: BTreeMap::new(), warn: WarnConfig::default(), verify: VerifyConfig::default(), deny: DenyConfig::default() }
    }
}


impl Config {
    /// Root directories relative to the project, without a trailing slash.
    pub fn root_dirs(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for r in &self.roots {
            let r = r.trim().trim_start_matches("./").trim_end_matches('/').to_string();
            let r = if r.is_empty() { ".".to_string() } else { r };
            if !out.contains(&r) {
                out.push(r);
            }
        }
        out
    }
}

/// The settings file of a project: `mdmap.yml` at its root. A project may have
/// none and run on the defaults.
pub fn path_of(project: &Path) -> Option<String> {
    project.join(MDMAP_PATH).is_file().then(|| MDMAP_PATH.to_string())
}

/// The nearest directory upward that holds a settings file, else the nearest
/// git checkout, where the defaults apply.
pub fn find_project(cwd: &Path) -> Option<PathBuf> {
    let mut dir = Some(cwd);
    while let Some(d) = dir {
        if path_of(d).is_some() || d.join(".git").exists() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

pub fn load(project: &Path) -> R<Config> {
    let Some(path) = path_of(project) else { return Ok(Config::default()) };
    // serde-saphyr puts a snippet of the file below the message; a refusal is one line.
    let bad = |e: serde_saphyr::Error| Refusal::new(format!("{path}: {}", e.to_string().lines().next().unwrap_or_default()));
    let Ok(text) = std::fs::read_to_string(project.join(&path)) else { return Ok(Config::default()) };
    let config = serde_saphyr::from_str::<Config>(&text).map_err(bad)?;
    for (name, items) in &config.templates {
        check_template(&path, name, items, 0)?;
    }
    for (verb, list) in config.deny.lists() {
        if let Some(bad) = list.ids.iter().find(|x| !crate::id::is_id(x)) {
            return Err(Refusal::new(format!("{path}: deny.{verb}.ids: {bad} is not an id, six lowercase hex characters")));
        }
    }
    Ok(config)
}

/// One heading of a template: a title, or a title with the headings under it.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TemplateItem {
    Title(String),
    Under(BTreeMap<String, Vec<TemplateItem>>),
}

impl TemplateItem {
    fn parts(&self) -> (&str, &[TemplateItem]) {
        match self {
            TemplateItem::Title(t) => (t.trim(), Default::default()),
            TemplateItem::Under(m) => m.iter().next().map(|(t, u)| (t.trim(), u.as_slice())).unwrap_or(("", Default::default())),
        }
    }
}

/// A template's headings in order: depth from 0, title, and the path
/// `--fill` names it by (`Repro/Steps`).
pub fn template_headings(items: &[TemplateItem]) -> Vec<(u8, String, String)> {
    fn walk(items: &[TemplateItem], depth: u8, prefix: &str, out: &mut Vec<(u8, String, String)>) {
        for it in items {
            let (title, under) = it.parts();
            let path = if prefix.is_empty() { title.to_string() } else { format!("{prefix}/{title}") };
            out.push((depth, title.to_string(), path.clone()));
            walk(under, depth + 1, &path, out);
        }
    }
    let mut out = Vec::new();
    walk(items, 0, "", &mut out);
    out
}

/// A template holds headings mdmap can write: one title per item, one line,
/// no `/` (it separates a `--fill` path), no title twice under one heading,
/// at most 5 levels (a node at level 1 leaves 5).
fn check_template(path: &str, name: &str, items: &[TemplateItem], depth: usize) -> R<()> {
    let bad = |why: String| Refusal::new(format!("{path}: template {name}: {why}"));
    if items.is_empty() {
        return Err(bad("a heading list is empty".to_string()));
    }
    if depth > 4 {
        return Err(bad("more than 5 levels; markdown has 6 and the node it goes under takes one".to_string()));
    }
    let mut seen = BTreeSet::new();
    for it in items {
        if matches!(it, TemplateItem::Under(m) if m.len() != 1) {
            return Err(bad("an item with headings under it has exactly one title".to_string()));
        }
        let (title, under) = it.parts();
        if title.is_empty() || title.contains(['\n', '\r', '/']) {
            return Err(bad(format!("{title:?} is not a title: one line, no /")));
        }
        if !seen.insert(title) {
            return Err(bad(format!("{title:?} twice under one heading")));
        }
        if matches!(it, TemplateItem::Under(_)) {
            check_template(path, name, under, depth + 1)?;
        }
    }
    Ok(())
}
