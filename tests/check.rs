//! `mdmap check`: can the tree be read from these files, and `--fix`.

use std::collections::BTreeMap;

mod common;
use common::Md;
use common::Project;

/// Every file under the project except the log, with its contents.
fn snapshot(p: &Project) -> BTreeMap<String, String> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let path = e.path();
            // With `/` on every platform, as mdmap names a path.
            let rel = path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
            if rel.starts_with(".harno/log") || rel == ".gitignore" {
                continue;
            }
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                out.insert(rel, std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(p.root(), p.root(), &mut out);
    out
}

#[test]
fn a_clean_corpus_has_no_problems() {
    let p = Project::sample();
    let c = p.md(&["check"]);
    assert_eq!(c.out, "no check problems\n");
    assert_eq!(c.code, 0);
}

#[test]
fn check_reports_structure_problems_and_writes_nothing_for_them() {
    let p = Project::sample();
    // A skipped level and a second `#` are fine: the tree takes any levels.
    p.write("docs/skip.md", "# Skip {c00031}\n\n### Deep {c00032}\n");
    p.write("docs/two.md", "# One {c00033}\n\n# Two {c00034}\n");
    p.write("docs/setext.md", "Setext {c00035}\n===============\n\nText.\n");
    let before = snapshot(&p);
    let c = p.md(&["check"]);
    assert_eq!(c.code, 1);
    assert_eq!(c.out, "docs/setext.md:1   setext heading\n");
    assert_eq!(snapshot(&p), before);
}

#[test]
fn every_call_gives_every_node_an_id_before_it_reads() {
    let p = Project::sample();
    p.write("docs/noid.md", "# No id\n\nText.\n\n## Also none\n");
    p.write("docs/copy.md", "# Copy {c00030}\n\n## Edge {a00013}\n");
    p.write("docs/plain.md", "Just text.\n");
    p.write("docs/sub/leaf.md", "# Leaf {c00036}\n");
    let out = p.md(&["ls"]).ok().to_string();
    // A file without a heading and a folder without README need nothing.
    assert!(out.starts_with("autofix: 3 headings got an id — docs/copy.md, docs/noid.md\n"), "{out}");
    // Every heading in the tree now carries an id.
    for line in out.lines().skip(1).filter(|l| l.contains('#')) {
        assert!(line.trim_end().ends_with('}'), "no id: {line}\n{out}");
    }
    assert_eq!(p.id_of("docs/arch.md", "Id"), "a00013", "the first in path order keeps a duplicated id");
    assert_ne!(p.id_of("docs/copy.md", "Edge"), "a00013");
    // Ids are written once; the next call finds nothing to do.
    let before = snapshot(&p);
    let out = p.md(&["ls"]).ok().to_string();
    assert!(!out.contains("autofix"), "{out}");
    assert_eq!(snapshot(&p), before);
    assert_eq!(p.md(&["check"]).ok(), "no check problems\n");
}

#[test]
fn autofix_writes_ids_only_and_leaves_structure_to_check_fix() {
    let p = Project::sample();
    p.write("docs/skip.md", "# Skip\n\n### Deep\n");
    let out = p.md(&["show", "a00001", "--info"]).ok().to_string();
    assert!(out.starts_with("autofix: 2 headings got an id — docs/skip.md\n  # Docs"), "{out}");
    let a = p.id_of("docs/skip.md", "Skip");
    let b = p.id_of("docs/skip.md", "Deep");
    assert_eq!(p.read("docs/skip.md"), format!("# Skip {{{a}}}\n\n### Deep {{{b}}}\n"));
}

#[test]
fn the_id_pass_of_a_read_leaves_a_setext_heading_setext() {
    let p = Project::sample();
    p.write("docs/setext.md", "Setext\n======\n\nText.\n");
    let out = p.md(&["ls"]).ok().to_string();
    assert!(out.starts_with("autofix: 1 heading got an id — docs/setext.md\n"), "{out}");
    let text = p.read("docs/setext.md");
    assert!(text.starts_with("Setext {"), "the id goes on the heading line: {text}");
    assert!(text.contains("\n======\n"), "the underline is structure, left to check --fix: {text}");
    assert!(text.ends_with("\n\nText.\n"), "the body is untouched: {text}");
    // The structure problem is still there, and the id the read wrote stays.
    assert!(p.md(&["check"]).out.contains("docs/setext.md:1   setext heading"), "{}", p.md(&["check"]).out);
    p.md(&["check", "--fix"]).ok();
    let id = p.id_of("docs/setext.md", "Setext");
    assert_eq!(p.read("docs/setext.md"), format!("# Setext {{{id}}}\n\nText.\n"));
}

#[test]
fn fix_closes_every_problem_and_a_second_run_writes_nothing() {
    let p = Project::sample();
    p.write("docs/noid.md", "# No id\n\nText.\n\n## Also none\n");
    p.write("docs/copy.md", "# Copy {c00030}\n\n## Edge {a00013}\n");

    p.write("docs/setext.md", "Setext {c00035}\n===============\n\nText.\n\nSub\n---\n");
    p.write("docs/plain.md", "---\ntags: [x]\n---\nJust text.\n");
    p.write("docs/sub/leaf.md", "# Leaf {c00036}\n");
    p.md(&["check", "--fix"]).ok();
    assert_eq!(p.md(&["check"]).ok(), "no check problems\n");

    let after = snapshot(&p);
    p.md(&["check", "--fix"]).ok();
    assert_eq!(snapshot(&p), after, "second --fix changed files");

    // Duplicate: the first in path order keeps the id.
    assert_eq!(p.id_of("docs/arch.md", "Id"), "a00013");
    assert_ne!(p.id_of("docs/copy.md", "Edge"), "a00013");

    // Missing ids are written at the end of the heading line.
    let noid = p.read("docs/noid.md");
    let ids = p.ids_in("docs/noid.md");
    assert_eq!(noid, format!("# No id {{{}}}\n\nText.\n\n## Also none {{{}}}\n", ids[0], ids[1]));


    // Setext becomes ATX.
    let setext = p.read("docs/setext.md");
    let sub = p.id_of("docs/setext.md", "Sub");
    assert_eq!(setext, format!("# Setext {{c00035}}\n\nText.\n\n## Sub {{{sub}}}\n"));

    // A file without a heading and a folder without README stay as they are.
    assert_eq!(p.read("docs/plain.md"), "---\ntags: [x]\n---\nJust text.\n");
    assert!(!p.exists("docs/sub/README.md"));
}

#[test]
fn fix_does_not_touch_bodies_or_other_files() {
    let p = Project::sample();
    p.write("docs/noid.md", "# No id\n\nBody   with   spaces.\n```\n## fenced\n```\n");
    let before = snapshot(&p);
    p.md(&["check", "--fix"]).ok();
    let after = snapshot(&p);
    for (k, v) in &before {
        if k != "docs/noid.md" {
            assert_eq!(after.get(k), Some(v), "{k} changed");
        }
    }
    let id = p.id_of("docs/noid.md", "No id");
    assert_eq!(p.read("docs/noid.md"), format!("# No id {{{id}}}\n\nBody   with   spaces.\n```\n## fenced\n```\n"));
}

#[test]
fn fix_reports_what_it_fixed() {
    let p = Project::sample();
    p.write("docs/setext.md", "Setext {c00035}\n===============\n");
    let out = p.md(&["check", "--fix"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    docs/setext.md:1   setext heading — fixed

    fixed 1 problem in 1 file
    ");
}

#[test]
fn a_missing_root_is_an_empty_folder() {
    let p = Project::new();
    std::fs::remove_dir_all(p.path("docs")).unwrap();
    assert_eq!(p.md(&["ls"]).ok(), "docs/\n");
    p.md(&["check", "--fix"]).ok();
    assert!(!p.exists("docs"));
}

#[test]
fn a_call_that_reads_a_broken_file_starts_with_a_warning() {
    let p = Project::sample();
    p.write("docs/design.md", "# Design {a00020}\n\nWhy.\n\nUnderlined {c00050}\n----------\n");
    let out = p.md(&["show", "a00020", "--info"]).ok().to_string();
    assert!(out.starts_with("warning: 1 check problem — mdmap check\n"), "{out}");
    // A call that reads only healthy files carries no warning.
    let out = p.md(&["show", "a00012", "--info"]).ok().to_string();
    assert!(!out.contains("warning"), "{out}");
}

#[test]
fn check_json() {
    let p = Project::sample();
    p.write("docs/setext.md", "Setext {c00035}\n===============\n");
    let v = p.md(&["check", "--json"]).json();
    assert_eq!(v["problems"][0]["at"], "docs/setext.md:1");
    assert_eq!(v["problems"][0]["problem"], "setext heading");
}

#[test]
fn parallel_calls_agree_on_the_ids_they_write() {
    let p = Project::sample();
    let text: String = (0..30).map(|i| format!("## Section {i}\n\nText.\n\n")).collect();
    p.write("docs/many.md", &format!("# Many\n\n{text}"));
    let outs: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..6).map(|_| s.spawn(|| p.md(&["ls"]).ok().to_string())).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let ids = p.ids_in("docs/many.md");
    assert_eq!(ids.len(), 31);
    // Every call printed the ids that ended up on disk.
    for out in &outs {
        for id in &ids {
            assert!(out.contains(&format!("{{{id}}}")), "{id} missing from\n{out}");
        }
    }
    assert_eq!(outs.iter().filter(|o| o.starts_with("autofix:")).count(), 1);
}
