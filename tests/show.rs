//! `mdmap show` and `mdmap ls`: reading the tree.

mod common;
use common::Md;
use common::{field, Project};

#[test]
fn show_starts_with_the_heading_as_written_and_lists_the_numbers() {
    let p = Project::sample();
    let out = p.md(&["show", "a00013", "--info"]).ok().to_string();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "  ## Id {a00013}");
    assert_eq!(lines[1], "");
    assert!(lines[2].starts_with("  summary    Ids are random."), "{out}");
    for f in ["body", "subtree", "nodes", "max_depth"] {
        let _: usize = field(&out, f);
    }
}

#[test]
fn a_verify_block_is_counted_not_printed() {
    let p = Project::sample();
    p.write("docs/files.md", "# Files {f00001}\n\nThe source files.\n\n<!-- verify: ls src/*.rs -->\n\n```\nsrc/graph.rs\nsrc/id.rs\n```\n\nProse after the span stays.\n");
    let out = p.md(&["show", "f00001"]).ok().to_string();
    assert!(out.contains("<!-- verify: ls src/*.rs -->"), "the line that names the command stays: {out}");
    assert!(out.contains("… 2 lines: mdmap show f00001 --outline"), "{out}");
    assert!(!out.contains("src/graph.rs"), "the block itself stays out: {out}");
    assert!(out.contains("Prose after the span stays."), "{out}");
    let whole = p.md(&["show", "f00001", "--outline"]).ok().to_string();
    assert!(whole.contains("src/graph.rs") && whole.contains("src/id.rs"), "{whole}");
}

#[test]
fn a_fenced_block_the_verify_comment_does_not_own_is_printed_whole() {
    let p = Project::sample();
    p.write("docs/inline.md", "# Inline {f00002}\n\n<!-- verify: ls src -->\n\nThe span of that command is the inline `src/graph.rs`.\n\n```\nan example\nthe reader needs\n```\n");
    let out = p.md(&["show", "f00002"]).ok().to_string();
    assert!(out.contains("an example") && out.contains("the reader needs"), "a block the comment does not own stays: {out}");
    assert!(!out.contains("lines: mdmap show"), "{out}");
}

#[test]
fn a_verify_fence_that_never_closes_keeps_its_lines() {
    let p = Project::sample();
    p.write("docs/open.md", "# Open {f00003}\n\n<!-- verify: ls src -->\n\n```\nsrc/graph.rs\nsrc/id.rs\n");
    let out = p.md(&["show", "f00003"]).ok().to_string();
    assert!(out.contains("src/graph.rs") && out.contains("src/id.rs"), "no line of the body is dropped: {out}");
}

#[test]
fn show_lists_every_neighbour_with_body_and_subtree() {
    let p = Project::sample();
    let out = p.md(&["show", "a00013", "--info"]).ok().to_string();
    let row = |label: &str, heading: &str| {
        out.lines()
            .find(|l| l.trim_start().starts_with(label) && l.contains(heading))
            .unwrap_or_else(|| panic!("no {label} {heading} in\n{out}"))
            .to_string()
    };
    let parent = row("parent", "# Architecture {a00010}");
    // A link to a path goes to the file.
    let link = row("link", "docs/README.md");
    let back1 = row("backlink", "## How to read {a00002}");
    let back2 = out.lines().find(|l| l.contains("## Why random ids {a00021}")).unwrap();
    assert!(back2.trim_start().starts_with("##"), "second backlink row has no repeated label: {back2}");
    assert!(out.contains("body  subtree"), "{out}");
    // Numbers on each row: the parent's body and subtree.
    let nums: Vec<usize> = parent.split_whitespace().rev().take(2).map(|n| n.parse().unwrap()).collect();
    let arch = p.md(&["show", "a00010", "--info"]).ok().to_string();
    assert_eq!(nums[0], field::<usize>(&arch, "subtree"));
    assert_eq!(nums[1], field::<usize>(&arch, "body"));
    let _ = (link, back1);
}

#[test]
fn every_neighbour_row_is_callable() {
    let p = Project::sample();
    let v = p.md(&["show", "a00013", "--json", "--info"]).json();
    let n = &v["nodes"][0];
    let mut keys: Vec<String> = vec![n["parent"]["key"].as_str().unwrap().to_string()];
    for role in ["children", "links", "backlinks"] {
        keys.extend(n[role].as_array().unwrap().iter().map(|x| x["key"].as_str().unwrap().to_string()));
    }
    assert_eq!(keys, ["a00010", "docs/README.md", "a00002", "a00021"]);
    let all: Vec<&str> = std::iter::once("show").chain(keys.iter().map(|s| s.as_str())).collect();
    p.md(&all).ok();
}

#[test]
fn body_prints_only_the_heading_the_lines_and_the_body_verbatim() {
    let p = Project::sample();
    let out = p.md(&["show", "a00012", "--info"]).ok().to_string();
    assert!(!out.contains("lines "), "{out}");
    let out = p.md(&["show", "a00012"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    ### Fields {a00012}
    14  Title, summary, body.
    ");
    p.write("docs/two.md", "# Two {c00020}\n\nOne.\n\n  Two.\n\n");
    assert_eq!(p.md(&["show", "c00020"]).ok(), "# Two {c00020}\n3  One.\n4\n5    Two.\n");
}

#[test]
fn path_adds_the_file_and_heading_line() {
    let p = Project::sample();
    let out = p.md(&["show", "a00012", "--path", "--info"]).ok().to_string();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "  ### Fields {a00012}");
    assert_eq!(lines[1], "  path       docs/arch.md:12");
    let out = p.md(&["show", "a00030", "--path", "--info"]).ok().to_string();
    assert!(out.contains("  path       docs/findings/README.md:1\n"), "{out}");
}

#[test]
fn several_ids_in_one_call_each_in_its_own_block() {
    let p = Project::sample();
    let out = p.md(&["show", "a00011", "a00012"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    ## Node {a00011}
    10  A node is a heading.

    ### Fields {a00012}
    14  Title, summary, body.
    ");
}

#[test]
fn ids_are_written_without_braces() {
    let p = Project::sample();
    let c = p.md(&["show", "{a00013}"]);
    c.refused();
    assert_eq!(c.out.lines().next().unwrap(), "refused: ids are written without braces on the command line");
    assert_eq!(c.instead().unwrap(), "mdmap show a00013");
}

#[test]
fn an_unknown_id_is_refused_with_the_known_ones_as_the_next_call() {
    let p = Project::sample();
    let c = p.md(&["show", "a00013", "ffffff", "--info"]);
    assert!(c.refused().starts_with("refused: no node with id ffffff\n"));
    assert_eq!(c.instead().unwrap(), "mdmap show a00013");
    let c = p.md(&["show", "ffffff", "--info"]);
    assert_eq!(c.instead().unwrap(), "mdmap ls");
    let c = p.md(&["show", "hello", "--info"]);
    assert!(c.refused().contains("nothing at 'hello': a node is the id of a heading, or the path of a folder or a file"), "{}", c.out);
}

#[test]
fn a_refusal_is_two_lines_and_a_nonzero_exit() {
    let p = Project::sample();
    let c = p.md(&["show", "ffffff", "--info"]);
    assert_ne!(c.code, 0);
    assert_eq!(c.out, "refused: no node with id ffffff\ninstead: mdmap ls\n");
}

#[test]
fn json_carries_the_same_content_with_body_in_its_own_field() {
    let p = Project::sample();
    let v = p.md(&["show", "a00012", "--json"]).json();
    let n = &v["nodes"][0];
    assert_eq!(n["id"], "a00012");
    assert_eq!(n["heading"], "### Fields {a00012}");
    assert_eq!(n["body"], "Title, summary, body.\n");
    assert_eq!(n["lines"], serde_json::json!([14, 14]));
    assert!(n.get("parent").is_none() && n.get("summary").is_none(), "{v}");
    let v = p.md(&["show", "ffffff", "--json", "--info"]).json();
    assert_eq!(v["refused"], "no node with id ffffff");
    assert_eq!(v["instead"], "mdmap ls");
}

#[test]
fn ls_prints_the_whole_tree_with_titles_and_ids_only() {
    let p = Project::sample();
    let out = p.md(&["ls", "a00010"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    ## Node {a00011}
       ### Fields {a00012}
    ## Id {a00013}
    ");
}

#[test]
fn ls_depth_limits_the_levels() {
    let p = Project::sample();
    let out = p.md(&["ls", "a00010", "--depth", "1"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    ## Node {a00011}
       … 1 more
    ## Id {a00013}

    … = nodes not shown; open one: mdmap ls <id>|<path>
    ");
    let out = p.md(&["ls", "a00010", "--depth", "2"]).ok().to_string();
    let titles: Vec<String> = out.lines().map(|l| l.split('{').next().unwrap().trim_end().to_string()).collect();
    assert_eq!(titles, ["## Node", "   ### Fields", "## Id"], "{out}");
    assert!(p.md(&["ls", "--depth", "0"]).refused().contains("--depth takes a number of levels"));
}

#[test]
fn ls_without_an_id_prints_every_root_and_everything_under_it() {
    let p = Project::sample();
    let out = p.md(&["ls"]).ok().to_string();
    // A file with one heading at its top shares a line with it.
    insta::assert_snapshot!(out, @"
    docs/
       README.md  # Docs {a00001}
          ## How to read {a00002}
       arch.md  # Architecture {a00010}
          ## Node {a00011}
             ### Fields {a00012}
          ## Id {a00013}
       design.md  # Design {a00020}
          ## Why random ids {a00021}
       docs/findings/
          B-013-language.md  # B-013 Language {a00031}
             ## Decision record {a00032}
          README.md  # Findings {a00030}
          docs/findings/2026/
             B-043-turns.md  # B-043 Turns {a00041}
             README.md  # 2026 {a00040}
    ");
    // A line the depth cut says how many nodes it hides; the legend how to open one.
    let out = p.md(&["ls", "--depth", "1"]).ok().to_string();
    let shown: Vec<&str> = out.lines().filter(|l| !l.is_empty() && !l.contains("…")).collect();
    assert_eq!(shown.len(), 5, "{out}");
    assert!(out.contains("   README.md  # Docs {a00001}\n      … 1 more\n"), "{out}");
    assert!(out.ends_with("\n… = nodes not shown; open one: mdmap ls <id>|<path>\n"), "{out}");
}

#[test]
fn ls_summary_and_body_add_to_each_line() {
    let p = Project::sample();
    let out = p.md(&["ls", "a00010", "--summary"]).ok().to_string();
    assert!(out.contains("\n  A node is a heading.\n"), "{out}");
    assert!(out.contains("\n  Ids are random. See [the root](README.md).\n"), "{out}");
    let out = p.md(&["ls", "a00010", "--body"]).ok().to_string();
    assert!(out.contains("{a00011}\n10  A node is a heading.\n"), "{out}");
}

#[test]
fn ls_json_keeps_the_heading_the_text_line_folds_into_a_file() {
    let p = Project::sample();
    // A file with one top heading shares its line with that heading, so what
    // the depth hides there is that heading fewer than in the tree of --json,
    // which keeps every node of its own.
    let out = p.md(&["ls", "docs/findings", "--depth", "1"]).ok().to_string();
    assert!(out.contains("B-013-language.md  # B-013 Language {a00031}\n   … 1 more\n"), "{out}");
    assert!(out.contains("README.md  # Findings {a00030}\n"), "{out}");
    assert!(!out.contains("# Findings {a00030}\n   …"), "nothing is hidden under it: {out}");
    let v = p.md(&["ls", "docs/findings", "--depth", "1", "--json"]).json();
    let nodes = v["nodes"].as_array().unwrap().clone();
    let of = |key: &str| nodes.iter().find(|n| n["key"] == key).unwrap_or_else(|| panic!("no {key}")).clone();
    assert_eq!(of("docs/findings/B-013-language.md")["hidden"], 2);
    assert_eq!(of("docs/findings/README.md")["hidden"], 1);
}

#[test]
fn ls_json_is_a_nested_tree() {
    let p = Project::sample();
    let v = p.md(&["ls", "a00010", "--depth", "2", "--json"]).json();
    assert_eq!(v["nodes"][0]["id"], "a00011");
    assert_eq!(v["nodes"][0]["children"][0]["id"], "a00012");
    assert_eq!(v["nodes"][1]["id"], "a00013");
}

#[test]
fn a_node_without_body_shows_no_lines() {
    let p = Project::sample();
    p.write("docs/empty.md", "# Empty {c00010}\n## Child {c00011}\n");
    assert_eq!(p.md(&["show", "c00010"]).ok(), "# Empty {c00010}\n");
}
