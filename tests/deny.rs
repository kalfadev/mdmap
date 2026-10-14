//! `deny` of `mdmap.yml`: nodes closed to reading, updating or removing, by
//! path or by id, and everything under them.

mod common;
use common::Md;
use common::{Project, SAMPLE_ARCH, SAMPLE_DESIGN};

/// The sample corpus with `docs/findings/2026` and `## Node` closed to
/// reading, `## Id` to updating and `design.md` to removing; an id of digits
/// only is read as an id.
fn project() -> Project {
    let p = Project::sample();
    p.md_config("deny:\n  read:\n    paths: [\"docs/findings/2026/**\"]\n    ids: [a00011]\n  update:\n    ids: [a00013, 123456]\n  remove:\n    paths: [docs/design.md]\n");
    p
}

#[test]
fn a_star_in_a_deny_path_stays_inside_one_folder() {
    let p = Project::sample();
    p.md_config("deny:\n  read:\n    paths: [\"docs/*.md\"]\n");
    p.md(&["show", "docs/arch.md"]).refused();
    p.md(&["show", "docs/findings/README.md"]).ok();
    p.md_config("deny:\n  read:\n    paths: [\"docs/**/README.md\"]\n");
    p.md(&["show", "docs/README.md"]).refused();
    p.md(&["show", "docs/findings/README.md"]).refused();
    p.md(&["show", "docs/arch.md"]).ok();
}

#[test]
fn ls_shows_a_read_denied_node_as_denied_and_nothing_under_it() {
    let p = project();
    let out = p.md(&["ls"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    docs/
       README.md  # Docs {a00001}
          ## How to read {a00002}
       arch.md  # Architecture {a00010}
          ## [denied]
          ## Id {a00013}
       design.md  # Design {a00020}
          ## Why random ids {a00021}
       docs/findings/
          B-013-language.md  # B-013 Language {a00031}
             ## Decision record {a00032}
          README.md  # Findings {a00030}
          docs/findings/2026/  [denied]
    ");
    let v = p.md(&["ls", "a00010", "--json"]).json();
    assert_eq!(v["nodes"][0], serde_json::json!({"level": 2, "kind": "section", "denied": true}));
    let out = p.md(&["show", "a00010", "--info"]).ok().to_string();
    assert!(out.contains("  children  ## [denied]"), "{out}");
    assert!(!out.contains("Node"), "{out}");
}

#[test]
fn every_verb_refuses_a_read_denied_node_and_what_is_under_it() {
    let p = project();
    let c = p.md(&["show", "a00041"]);
    assert_eq!(c.out, "refused: deny of mdmap.yml closes a00041 to show\ninstead: mdmap ls\n");
    let calls: &[&[&str]] = &[
        &["show", "docs/findings/2026"],
        &["show", "a00012", "--info"],
        &["ls", "docs/findings/2026/"],
        &["update", "a00011", "--body", "Changed."],
        &["add", "a00011", "--title", "New"],
        &["remove", "a00012"],
        &["move", "a00012", "docs/design.md"],
        &["update", "a00011", "--append", "More."],
        &["add", "docs/findings/2026", "--title", "New"],
        &["remove", "docs/findings/2026"],
        &["show", "a00013", "a00011"],
    ];
    for args in calls {
        let c = p.md(args);
        assert!(c.refused().contains("deny of mdmap.yml closes"), "{args:?}: {}", c.out);
    }
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH);
}

#[test]
fn update_and_remove_close_only_their_own_verbs() {
    let p = project();
    // `## Id` is closed to update, and so to an add under it or a move into it.
    let calls: &[&[&str]] = &[&["update", "a00013", "--body", "Changed."], &["add", "a00013", "--title", "New"], &["move", "a00002", "a00013"]];
    for args in calls {
        let c = p.md(args);
        assert!(c.refused().contains("closes a00013 to"), "{args:?}: {}", c.out);
    }
    // `design.md` is closed to remove, and so to a move out of it; a folder
    // that holds a read-denied node is closed to remove as well.
    let calls: &[&[&str]] = &[&["remove", "docs/design.md"], &["remove", "a00021"], &["move", "a00021", "a00002"], &["remove", "docs/findings"]];
    for args in calls {
        let c = p.md(args);
        assert!(c.refused().contains("or a node under it to"), "{args:?}: {}", c.out);
    }
    assert_eq!(p.read("docs/design.md"), SAMPLE_DESIGN);
    // What each list leaves open still works.
    p.md(&["update", "a00021", "--body", "Changed."]).ok();
    p.md(&["show", "a00013"]).ok();
    p.md(&["remove", "a00002"]).ok();
}

#[test]
fn search_stats_and_export_leave_out_what_is_closed_to_read() {
    let p = project();
    assert_eq!(p.md(&["search", "node"]).ok().to_string(), "no node holds \"node\"\n");
    assert_eq!(p.md(&["search", "year"]).ok().to_string(), "no node holds \"year\"\n");
    let out = p.md(&["stats"]).ok().to_string();
    for id in ["a00011", "a00012", "a00040", "a00041"] {
        assert!(!out.contains(id), "{id}: {out}");
    }
    assert!(out.contains("{a00013}"), "{out}");
    let v = p.md(&["export"]).json();
    let nodes = v["nodes"].as_array().unwrap();
    let keys: Vec<&str> = nodes.iter().map(|n| n["key"].as_str().unwrap()).collect();
    for k in ["a00011", "a00012", "docs/findings/2026/", "a00040", "a00041"] {
        assert!(!keys.contains(&k), "{k}: {keys:?}");
    }
    let arch = nodes.iter().find(|n| n["key"] == "a00010").unwrap();
    assert_eq!(arch["children"], serde_json::json!(["a00013"]));
}

#[test]
fn a_deny_id_that_is_not_an_id_is_refused() {
    let p = Project::sample();
    p.md_config("deny:\n  read:\n    ids: [\"{a00011}\"]\n");
    let c = p.md(&["ls"]);
    assert!(c.refused().contains("deny.read.ids: {a00011} is not an id"), "{}", c.out);
    p.md_config("deny:\n  read:\n    path: [docs/x]\n");
    p.md(&["ls"]).refused();
    p.md_config("deny:\n  update:\n    paths: [\"docs/[x\"]\n");
    let c = p.md(&["ls"]);
    assert!(c.refused().contains("bad deny.update pattern docs/[x"), "{}", c.out);
}

/// A second corpus: `arch.md` closed by its path, the 2026 folder by a path
/// written with `./` and a trailing slash, and the one heading of
/// `design.md` by its id.
fn closed_files() -> Project {
    let p = Project::sample();
    p.md_config("deny:\n  read:\n    paths: [\"./docs/findings/2026/\", docs/arch.md]\n    ids: [a00020]\n");
    p
}

#[test]
fn ls_prints_a_denied_file_and_a_denied_top_heading_on_the_file_line() {
    let p = closed_files();
    let out = p.md(&["ls"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    docs/
       README.md  # Docs {a00001}
          ## How to read {a00002}
       arch.md  [denied]
       design.md  # [denied]
       docs/findings/
          B-013-language.md  # B-013 Language {a00031}
             ## Decision record {a00032}
          README.md  # Findings {a00030}
          docs/findings/2026/  [denied]
    ");
    assert_eq!(p.md(&["ls", "docs/design.md"]).ok().to_string(), "# [denied]\n");
    for flag in ["--summary", "--body"] {
        let out = p.md(&["ls", flag]).ok().to_string();
        for text in ["Architecture", "The design in one", "A node is", "Why things", "This year", "Turns cost"] {
            assert!(!out.contains(text), "{flag} {text}: {out}");
        }
    }
    let v = p.md(&["ls", "docs/findings", "--json"]).json();
    let folder = v["nodes"].as_array().unwrap().iter().find(|n| n["key"] == "docs/findings/2026/").unwrap().clone();
    assert_eq!(folder, serde_json::json!({"key": "docs/findings/2026/", "level": 0, "kind": "folder", "denied": true}));
}

#[test]
fn a_link_to_a_denied_node_shows_as_denied_and_export_drops_every_edge_to_it() {
    let p = closed_files();
    let out = p.md(&["show", "a00002", "--info"]).ok().to_string();
    assert!(out.contains("  link      ## [denied]"), "{out}");
    let v = p.md(&["show", "a00002", "--info", "--json"]).json();
    assert_eq!(v["nodes"][0]["links"][0], serde_json::json!({"level": 2, "kind": "section", "denied": true}));
    for q in [project(), closed_files()] {
        let v = q.md(&["export"]).json();
        let nodes = v["nodes"].as_array().unwrap();
        let keys: Vec<&serde_json::Value> = nodes.iter().map(|n| &n["key"]).collect();
        for n in nodes {
            for edge in ["children", "links", "backlinks"] {
                for k in n[edge].as_array().unwrap() {
                    assert!(keys.contains(&k), "{} {edge} {k}", n["key"]);
                }
            }
        }
        for k in v["roots"].as_array().unwrap() {
            assert!(keys.contains(&k), "root {k}");
        }
    }
}

#[test]
fn a_denied_root_leaves_nothing_to_read() {
    let p = Project::sample();
    p.md_config("deny:\n  read:\n    paths: [\"docs/**\"]\n");
    assert_eq!(p.md(&["ls"]).ok().to_string(), "docs/  [denied]\n");
    let v = p.md(&["export"]).json();
    assert_eq!(v["roots"], serde_json::json!([]));
    assert_eq!(v["nodes"], serde_json::json!([]));
    assert_eq!(p.md(&["search", "the"]).ok().to_string(), "no node holds \"the\"\n");
}

#[test]
fn sizes_and_counts_tell_nothing_of_what_is_closed_to_read() {
    let before: usize = common::field(Project::sample().md(&["show", "a00010", "--info"]).ok(), "subtree");
    let p = project();
    let out = p.md(&["show", "a00010", "--info"]).ok().to_string();
    // `## Node` counts as one node with no body and nothing under it.
    assert!(common::field::<usize>(&out, "subtree") < before, "{out}");
    assert_eq!(common::field::<usize>(&out, "nodes"), 3, "{out}");
    let row = out.lines().find(|l| l.contains("## [denied]")).unwrap();
    assert!(row.trim_end().ends_with("-") && !row.chars().any(|c| c.is_ascii_digit()), "{row}");
    let out = p.md(&["ls", "--depth", "1"]).ok().to_string();
    assert!(out.contains("   arch.md  # Architecture {a00010}\n      … 2 more\n"), "{out}");
    assert!(out.contains("   docs/findings/\n      … 6 more\n"), "{out}");
}

#[test]
fn lint_reports_a_deny_entry_that_names_nothing() {
    let p = Project::sample();
    p.md_config("deny:\n  read:\n    paths: [\"docs/nothing/**\", docs/arch.md]\n  update:\n    ids: [a00013, 123456]\n");
    let c = p.md(&["lint"]);
    assert_eq!(c.code, 1, "{}", c.out);
    assert!(c.out.contains("deny.read.paths docs/nothing/**"), "{}", c.out);
    assert!(c.out.contains("deny.update.ids 123456"), "{}", c.out);
    assert!(c.out.contains("names no node, so it closes nothing"), "{}", c.out);
    assert!(!c.out.contains("docs/arch.md") && !c.out.contains("a00013"), "{}", c.out);
}
