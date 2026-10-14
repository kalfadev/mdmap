//! How files become nodes: folders, files, headings, bodies, summaries.

mod common;
use common::Md;
use common::{field, Project};

#[test]
fn a_folder_is_its_path_with_no_body_and_no_id() {
    let p = Project::sample();
    assert_eq!(p.md(&["show", "docs/"]).ok(), "docs/\n");
    assert_eq!(p.md(&["show", "docs"]).ok(), "docs/\n", "the slash is optional");
    let out = p.md(&["show", "docs/findings", "--info"]).ok().to_string();
    assert!(out.starts_with("  docs/findings/\n\n  summary    -\n  body       0\n"), "{out}");
    let v = p.md(&["show", "docs/findings", "--json", "--info"]).json();
    assert_eq!(v["nodes"][0]["id"], serde_json::Value::Null);
    assert_eq!(v["nodes"][0]["key"], "docs/findings/");
}

#[test]
fn the_readme_is_a_file_like_any_other() {
    let p = Project::sample();
    let out = p.md(&["show", "a00001"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    # Docs {a00001}
    3  The corpus root.
    ");
    let v = p.md(&["show", "a00001", "--json", "--info"]).json();
    assert_eq!(v["nodes"][0]["parent"]["key"], "docs/README.md");
}

#[test]
fn children_are_files_then_folders() {
    let p = Project::sample();
    let out = p.md(&["ls", "docs/", "--depth", "1"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    README.md  # Docs {a00001}
       … 1 more
    arch.md  # Architecture {a00010}
       … 3 more
    design.md  # Design {a00020}
       … 1 more
    docs/findings/
       … 10 more

    … = nodes not shown; open one: mdmap ls <id>|<path>
    ");
    let out = p.md(&["ls", "docs/findings", "--depth", "1"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    B-013-language.md  # B-013 Language {a00031}
       … 1 more
    README.md  # Findings {a00030}
    docs/findings/2026/
       … 4 more

    … = nodes not shown; open one: mdmap ls <id>|<path>
    ");
}

#[test]
fn every_heading_level_reads_the_same_way() {
    let p = Project::sample();
    // `## Node`'s body ends where `### Fields` starts.
    let out = p.md(&["show", "a00011"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    ## Node {a00011}
    10  A node is a heading.
    ");
    let out = p.md(&["show", "a00011", "--info"]).ok().to_string();
    assert!(out.contains("summary    A node is a heading.\n"), "{out}");
    assert!(out.contains("children  ### Fields {a00012}"), "{out}");
}

#[test]
fn a_heading_may_skip_levels_and_a_file_may_hold_several_tops() {
    let p = Project::sample();
    p.write("docs/free.md", "# One {c00020}\n\n### Deep {c00021}\n\nDeep.\n\n## Mid {c00022}\n\n# Two {c00023}\n\n#### Far {c00024}\n");
    let out = p.md(&["ls", "docs/free.md"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    # One {c00020}
       ### Deep {c00021}
       ## Mid {c00022}
    # Two {c00023}
       #### Far {c00024}
    ");
    assert_eq!(p.md(&["check"]).ok(), "no check problems\n");
    let v = p.md(&["show", "c00021", "--json", "--info"]).json();
    assert_eq!(v["nodes"][0]["parent"]["key"], "c00020");
}

#[test]
fn a_file_node_body_is_the_text_before_its_first_heading() {
    let p = Project::sample();
    p.write("docs/pre.md", "Intro line.\n\n# Title {c00010}\n\nBody.\n");
    let out = p.md(&["show", "docs/pre.md"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    docs/pre.md
    1  Intro line.
    ");
    // Frontmatter is no one's body.
    assert_eq!(p.md(&["show", "docs/arch.md"]).ok(), "docs/arch.md\n");
    let out = p.md(&["show", "docs/arch.md", "--info"]).ok().to_string();
    assert_eq!(field::<usize>(&out, "body"), 0);
    assert!(field::<usize>(&out, "subtree") > 50, "{out}");
}

#[test]
fn text_before_the_first_heading_belongs_to_the_file() {
    let p = Project::sample();
    let out = p.md(&["ls", "docs/", "--depth", "3", "--body"]).ok().to_string();
    assert!(!out.contains("title: Architecture"), "{out}");
}

#[test]
fn sizes_nodes_and_max_depth() {
    let p = Project::sample();
    let out = p.md(&["show", "a00012", "--info"]).ok().to_string();
    assert_eq!(field::<usize>(&out, "body"), "\nTitle, summary, body.\n\n".chars().count());
    assert_eq!(field::<usize>(&out, "subtree"), field::<usize>(&out, "body"));
    assert_eq!(field::<usize>(&out, "nodes"), 1);
    assert_eq!(field::<usize>(&out, "max_depth"), 0);

    let out = p.md(&["show", "a00011", "--info"]).ok().to_string();
    let node_body = "\nA node is a heading.\n\n".chars().count();
    let fields_body = "\nTitle, summary, body.\n\n".chars().count();
    assert_eq!(field::<usize>(&out, "subtree"), node_body + fields_body);
    assert_eq!(field::<usize>(&out, "nodes"), 2);
    assert_eq!(field::<usize>(&out, "max_depth"), 1);

    // 3 folders, 7 files, 13 headings; docs → findings → 2026 → B-043-turns.md → # B-043 Turns.
    let out = p.md(&["show", "docs/", "--info"]).ok().to_string();
    assert_eq!(field::<usize>(&out, "nodes"), 23);
    assert_eq!(field::<usize>(&out, "max_depth"), 4);
}

#[test]
fn nothing_inside_code_is_structure() {
    let p = Project::sample();
    p.write(
        "docs/code.md",
        "# Code {c00001}\n\nIntro.\n\n```markdown\n## not a heading\n[x](#a00001)\n<!-- verify: ls -->\n```\n\nInline `## no` and `[x](#a00001)` too.\n",
    );
    let out = p.md(&["show", "c00001", "--info"]).ok().to_string();
    assert_eq!(field::<usize>(&out, "nodes"), 1, "{out}");
    assert!(!out.contains("link "), "{out}");
    let out = p.md(&["show", "a00001", "--info"]).ok().to_string();
    assert!(!out.contains("backlink  # Code"), "{out}");
    let out = p.md(&["verify", "c00001"]).ok().to_string();
    assert!(out.contains("0 with verify"), "{out}");
}

#[test]
fn a_verify_comment_outside_code_is_part_of_the_body() {
    let p = Project::sample();
    p.write("docs/v.md", "# V {c00002}\n\nText.\n\n<!-- verify: echo hi -->\n\n`hi`\n");
    let out = p.md(&["show", "c00002"]).ok().to_string();
    assert!(out.contains("<!-- verify: echo hi -->"), "{out}");
}

#[test]
fn a_long_summary_is_cut_at_280_characters_with_an_ellipsis() {
    let p = Project::sample();
    let long = "word ".repeat(100);
    p.write("docs/long.md", &format!("# Long {{c00003}}\n\n{long}\n"));
    let out = p.md(&["show", "c00003", "--json", "--info"]).ok().to_string();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let summary = v["nodes"][0]["summary"].as_str().unwrap();
    assert!(summary.chars().count() <= 280, "{summary}");
    assert!(summary.ends_with('…'), "{summary}");
}

#[test]
fn the_summary_is_the_first_paragraph_joined_to_one_line() {
    let p = Project::sample();
    p.write("docs/wrap.md", "# Wrap {c00004}\n\nFirst line\nsecond line.\n\nSecond paragraph.\n");
    let v = p.md(&["show", "c00004", "--json", "--info"]).json();
    assert_eq!(v["nodes"][0]["summary"], "First line second line.");
}

#[test]
fn directories_without_markdown_are_not_nodes() {
    let p = Project::sample();
    p.write("docs/images/logo.png", "png");
    let out = p.md(&["ls"]).ok().to_string();
    assert!(!out.contains("images"), "{out}");
}

#[test]
fn ignored_paths_are_not_in_the_graph() {
    let p = Project::sample();
    p.md_config("roots: [docs/]\nignore: ['drafts/**', '**/INDEX.md']");
    p.write("docs/drafts/wip.md", "# WIP {c00005}\n\nDraft.\n");
    p.write("docs/findings/INDEX.md", "# Index\n\nGenerated.\n");
    let out = p.md(&["ls", "--depth", "9"]).ok().to_string();
    assert!(!out.contains("WIP") && !out.contains("Index"), "{out}");
    // An ignored file has no id to fix.
    let out = p.md(&["check"]).ok().to_string();
    insta::assert_snapshot!(out, @"no check problems");
}

#[test]
fn a_star_in_ignore_stays_inside_one_folder_and_two_stars_cross_folders() {
    let p = Project::sample();
    p.md_config("roots: [docs/]\nignore: [\"*.md\"]");
    let out = p.md(&["ls", "--depth", "9"]).ok().to_string();
    assert!(!out.contains("arch.md") && !out.contains("design.md"), "{out}");
    assert!(out.contains("B-013-language.md"), "{out}");
    p.md_config("roots: [docs/]\nignore: [\"**/README.md\"]");
    let out = p.md(&["ls", "--depth", "9"]).ok().to_string();
    assert!(!out.contains("README.md") && out.contains("arch.md"), "{out}");
}

#[test]
fn outside_the_roots_nothing_exists() {
    let p = Project::sample();
    p.write("README.md", "# Repo\n\nNot in the corpus.\n");
    p.write("target/gen.md", "# Generated\n");
    let out = p.md(&["ls", "--depth", "9"]).ok().to_string();
    assert!(!out.contains("Repo") && !out.contains("Generated"), "{out}");
    assert_eq!(p.md(&["check"]).ok(), "no check problems\n");
}

#[test]
fn each_root_is_its_own_tree() {
    let p = Project::sample();
    p.md_config("roots: [docs/, notes/]");
    p.write("notes/README.md", "# Notes {b00001}\n\nScratch.\n");
    p.write("notes/today.md", "# Today {b00002}\n\nToday.\n");
    let out = p.md(&["ls", "--depth", "1"]).ok().to_string();
    let tops: Vec<&str> = out.lines().filter(|l| !l.is_empty() && !l.starts_with(' ') && !l.contains("…")).collect();
    assert_eq!(tops, ["docs/", "notes/"], "{out}");
    assert!(out.contains("\n   today.md  # Today {b00002}\n"), "{out}");
    // A root has no parent.
    let out = p.md(&["show", "notes/", "--info"]).ok().to_string();
    assert!(!out.contains("parent"), "{out}");
}

#[test]
fn kind_comes_from_structure() {
    let p = Project::sample();
    let kind = |key: &str| p.md(&["show", key, "--json", "--info"]).json()["nodes"][0]["kind"].as_str().unwrap().to_string();
    assert_eq!(kind("docs/"), "folder");
    assert_eq!(kind("docs/findings"), "folder");
    assert_eq!(kind("docs/arch.md"), "file");
    assert_eq!(kind("docs/README.md"), "file");
    assert_eq!(kind("a00001"), "section");
    assert_eq!(kind("a00012"), "section");
}
