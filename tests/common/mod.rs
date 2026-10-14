//! What every mdmap test needs: a temporary project with its settings and a
//! sample corpus, and mdmap called in-process the way its binary calls it.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub struct Project {
    pub dir: TempDir,
}

#[derive(Debug)]
pub struct Call {
    pub out: String,
    pub code: i32,
}

impl Call {
    /// The output, asserting the call succeeded.
    #[track_caller]
    pub fn ok(&self) -> &str {
        assert_eq!(self.code, 0, "call failed:\n{}", self.out);
        &self.out
    }

    /// The output, asserting the call was refused.
    #[track_caller]
    pub fn refused(&self) -> &str {
        assert_ne!(self.code, 0, "call succeeded:\n{}", self.out);
        assert!(self.out.contains("refused: "), "not a refusal:\n{}", self.out);
        &self.out
    }

    pub fn instead(&self) -> Option<String> {
        self.out.lines().find_map(|l| l.strip_prefix("instead: ").map(|s| s.to_string()))
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.out).unwrap_or_else(|e| panic!("not JSON ({e}):\n{}", self.out))
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

impl Project {
    /// A project of mdmap: `mdmap.yml` with the default corpus, and an empty
    /// `docs/`.
    pub fn new() -> Project {
        let p = Project::bare();
        p.write("mdmap.yml", "roots: [docs/]\n");
        std::fs::create_dir_all(p.path("docs")).unwrap();
        p
    }

    /// The corpus most tests read: fixed ids, every node kind, links in both
    /// directions.
    ///
    /// ```text
    /// docs/                         # Docs {a00001}
    /// ├─ ## How to read {a00002}    links to a00013
    /// ├─ arch.md                    # Architecture {a00010} (with frontmatter)
    /// │  ├─ ## Node {a00011}
    /// │  │  └─ ### Fields {a00012}
    /// │  └─ ## Id {a00013}          links to README.md
    /// ├─ design.md                  # Design {a00020}
    /// │  └─ ## Why random ids {a00021}  links to arch.md#a00013
    /// └─ findings/                  # Findings {a00030}
    ///    ├─ 2026/                   # 2026 {a00040}
    ///    │  └─ B-043-turns.md       # B-043 Turns {a00041}
    ///    └─ B-013-language.md       # B-013 Language {a00031}
    ///       └─ ## Decision record {a00032}
    /// ```
    pub fn sample() -> Project {
        let p = Project::new();
        p.write("docs/README.md", SAMPLE_README)
            .write("docs/arch.md", SAMPLE_ARCH)
            .write("docs/design.md", SAMPLE_DESIGN)
            .write("docs/findings/README.md", "# Findings {a00030}\n\nRecords of decisions.\n")
            .write("docs/findings/2026/README.md", "# 2026 {a00040}\n\nThis year.\n")
            .write("docs/findings/2026/B-043-turns.md", "# B-043 Turns {a00041}\n\nTurns cost money.\n")
            .write(
                "docs/findings/B-013-language.md",
                "# B-013 Language {a00031}\n\nWhich language.\n\n## Decision record {a00032}\n\nEnglish.\n",
            );
        p
    }

    /// A temporary directory with nothing in it: no settings, no corpus.
    pub fn bare() -> Project {
        Project { dir: tempfile::tempdir().unwrap() }
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    // The settings are written by md_config of the Md trait.

    pub fn write(&self, rel: &str, text: &str) -> &Self {
        let p = self.path(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
        self
    }

    #[track_caller]
    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.path(rel).exists()
    }

    // A call of mdmap is the Md trait at the end of this file.

    /// The id written on the heading with this title in `rel`.
    #[track_caller]
    pub fn id_of(&self, rel: &str, title: &str) -> String {
        let text = self.read(rel);
        for line in text.lines() {
            let t = line.trim_start_matches('#').trim();
            if line.starts_with('#') && t.starts_with(title) {
                let rest = t[title.len()..].trim();
                if let Some(id) = rest.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
                    return id.to_string();
                }
            }
        }
        panic!("no heading {title:?} with an id in {rel}:\n{text}");
    }

    /// Every `{id}` on a heading line in a file.
    pub fn ids_in(&self, rel: &str) -> Vec<String> {
        self.read(rel)
            .lines()
            .filter(|l| l.starts_with('#'))
            .filter_map(|l| {
                let l = l.trim_end();
                let open = l.rfind('{')?;
                l.ends_with('}').then(|| l[open + 1..l.len() - 1].to_string())
            })
            .collect()
    }

    // Nothing else: an mdmap test reads files and calls mdmap.
}

pub const SAMPLE_README: &str = "# Docs {a00001}

The corpus root.

## How to read {a00002}

Read the [Id section](arch.md#a00013) first.
";

pub const SAMPLE_ARCH: &str = "---
title: Architecture
---
# Architecture {a00010}

The design in one file.

## Node {a00011}

A node is a heading.

### Fields {a00012}

Title, summary, body.

## Id {a00013}

Ids are random. See [the root](README.md).
";

pub const SAMPLE_DESIGN: &str = "# Design {a00020}

Why things are the way they are.

## Why random ids {a00021}

The rule is [in the Id section](arch.md#a00013).
";

/// A number from `show`'s field block: `  body       2840`.
#[track_caller]
pub fn field<T: std::str::FromStr>(out: &str, name: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    let prefix = format!("  {name} ");
    let line = out
        .lines()
        .find(|l| l.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no {name} in\n{out}"));
    line[prefix.len()..].trim().parse().unwrap()
}

/// A call of mdmap in a project, in this process.
pub trait Md {
    fn md(&self, args: &[&str]) -> Call;
    fn md_in(&self, cwd: &Path, args: &[&str]) -> Call;
    /// The settings of mdmap: mdmap.yml at the root of the project.
    fn md_config(&self, yaml: &str) -> &Self;
}

impl Md for Project {
    fn md(&self, args: &[&str]) -> Call {
        self.md_in(self.root(), args)
    }

    fn md_in(&self, cwd: &Path, args: &[&str]) -> Call {
        let env = mdmap::Env { cwd: cwd.to_path_buf() };
        let mut buf: Vec<u8> = Vec::new();
        let code = mdmap::main(args.iter().map(|s| s.to_string()).collect(), env, &mut buf);
        Call { out: String::from_utf8(buf).unwrap(), code }
    }

    fn md_config(&self, yaml: &str) -> &Self {
        self.write("mdmap.yml", yaml)
    }
}
