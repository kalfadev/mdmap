//! `mdmap add`, `update`, `remove`: the only ways the agent writes.

mod common;
use common::Md;
use common::{Project, SAMPLE_ARCH, SAMPLE_README};

fn is_id(s: &str) -> bool {
    s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[test]
fn add_to_a_file_appends_a_level_two_heading_and_touches_nothing_else() {
    let p = Project::sample();
    let out = p.md(&["add", "a00010", "--title", "Edge", "--body", "Edges link nodes."]).ok().to_string();
    let id = p.id_of("docs/arch.md", "Edge");
    assert!(is_id(&id));
    assert_eq!(out, format!("added ## Edge {{{id}}} · docs/arch.md:20\n"));
    let text = p.read("docs/arch.md");
    assert_eq!(text, format!("{SAMPLE_ARCH}\n## Edge {{{id}}}\n\nEdges link nodes.\n"));
}

#[test]
fn add_to_a_file_path_appends_a_top_heading() {
    let p = Project::sample();
    let out = p.md(&["add", "docs/README.md", "--title", "How to write"]).ok().to_string();
    let id = p.id_of("docs/README.md", "How to write");
    assert_eq!(out, format!("added # How to write {{{id}}} · docs/README.md:9\n"));
    assert_eq!(p.read("docs/README.md"), format!("{SAMPLE_README}\n# How to write {{{id}}}\n"));
    let out = p.md(&["ls", "docs/README.md", "--depth", "1"]).ok().to_string();
    let shown: Vec<&str> = out.lines().filter(|l| !l.is_empty() && !l.contains("…")).collect();
    assert_eq!(shown, ["# Docs {a00001}".to_string(), format!("# How to write {{{id}}}")], "{out}");
}

#[test]
fn add_to_a_section_goes_to_the_end_of_its_subtree_one_level_down() {
    let p = Project::sample();
    p.md(&["add", "a00011", "--title", "Slug"]).ok();
    let id = p.id_of("docs/arch.md", "Slug");
    let text = p.read("docs/arch.md");
    let expected = SAMPLE_ARCH.replace(
        "Title, summary, body.\n\n## Id",
        &format!("Title, summary, body.\n\n### Slug {{{id}}}\n\n## Id"),
    );
    assert_eq!(text, expected);
    let out = p.md(&["ls", "a00011"]).ok().to_string();
    assert!(out.lines().nth(1).unwrap().starts_with("### Slug"), "{out}");
}

#[test]
fn add_to_a_folder_creates_a_file_named_by_the_slug() {
    let p = Project::sample();
    let out = p.md(&["add", "docs/", "--title", "Tick sırası", "--body", "Order of ticks."]).ok().to_string();
    let id = p.id_of("docs/tick-sirasi.md", "Tick sırası");
    assert_eq!(out, format!("added # Tick sırası {{{id}}} · docs/tick-sirasi.md:1\n"));
    assert_eq!(p.read("docs/tick-sirasi.md"), format!("# Tick sırası {{{id}}}\n\nOrder of ticks.\n"));
    // It sits among the files in path order, before the folders.
    let out = p.md(&["ls", "docs", "--depth", "1"]).ok().to_string();
    let names: Vec<&str> = out.lines().filter(|l| !l.is_empty() && !l.contains("…")).map(|l| l.split_whitespace().next().unwrap()).collect();
    assert_eq!(names, ["README.md", "arch.md", "design.md", "tick-sirasi.md", "docs/findings/"]);
}

#[test]
fn add_dir_creates_a_folder_whose_readme_holds_the_heading() {
    let p = Project::sample();
    p.md(&["add", "docs/", "--dir", "--title", "Entry protocol"]).ok();
    let id = p.id_of("docs/entry-protocol/README.md", "Entry protocol");
    let v = p.md(&["show", "docs/entry-protocol", "--json", "--info"]).json();
    assert_eq!(v["nodes"][0]["kind"], "folder");
    let v = p.md(&["show", &id, "--json", "--info"]).json();
    assert_eq!(v["nodes"][0]["parent"]["key"], "docs/entry-protocol/README.md");
}

#[test]
fn dir_needs_a_folder() {
    let p = Project::sample();
    let c = p.md(&["add", "a00010", "--dir", "--title", "X"]);
    c.refused();
    assert_eq!(c.instead().unwrap(), "mdmap add a00010 --title \"X\"");
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH);
}

#[test]
fn an_existing_name_is_refused_and_the_refusal_names_it() {
    let p = Project::sample();
    let c = p.md(&["add", "docs/", "--title", "Design"]);
    assert!(c.refused().starts_with("refused: docs/design.md already exists\n"), "{}", c.out);
    let c = p.md(&["add", "docs/", "--dir", "--title", "Findings"]);
    assert!(c.refused().starts_with("refused: docs/findings/ already exists\n"), "{}", c.out);
}

#[test]
fn level_six_has_no_child_and_the_refusal_offers_a_sibling() {
    let p = Project::sample();
    p.write("docs/deep.md", "# D1 {d00001}\n\n## D2 {d00002}\n\n### D3 {d00003}\n\n#### D4 {d00004}\n\n##### D5 {d00005}\n\n###### D6 {d00006}\n");
    let before = p.read("docs/deep.md");
    let c = p.md(&["add", "d00006", "--title", "Ayrıntı"]);
    assert_eq!(c.out, "refused: d00006 is at level 6; markdown has no level 7\ninstead: mdmap add d00005 --title \"Ayrıntı\"\n");
    assert_ne!(c.code, 0);
    assert_eq!(p.read("docs/deep.md"), before);
    // The suggested call works and makes a sibling.
    p.md(&["add", "d00005", "--title", "Ayrıntı"]).ok();
    let id = p.id_of("docs/deep.md", "Ayrıntı");
    assert!(p.read("docs/deep.md").ends_with(&format!("###### D6 {{d00006}}\n\n###### Ayrıntı {{{id}}}\n")));
}

#[test]
fn several_children_in_one_call_in_the_given_order() {
    let p = Project::sample();
    let out = p
        .md(&["add", "a00013", "--title", "Sorun", "--body", "Problem.", "--title", "Gözlem", "--title", "Karar", "--body", "Decision."])
        .ok()
        .to_string();
    assert_eq!(out.lines().count(), 3, "{out}");
    let ids = [p.id_of("docs/arch.md", "Sorun"), p.id_of("docs/arch.md", "Gözlem"), p.id_of("docs/arch.md", "Karar")];
    let text = p.read("docs/arch.md");
    assert!(text.ends_with(&format!(
        "Ids are random. See [the root](README.md).\n\n### Sorun {{{}}}\n\nProblem.\n\n### Gözlem {{{}}}\n\n### Karar {{{}}}\n\nDecision.\n",
        ids[0], ids[1], ids[2]
    )), "{text}");
}

#[test]
fn if_one_child_is_refused_none_is_written() {
    let p = Project::sample();
    let c = p.md(&["add", "docs/", "--title", "New one", "--title", "Design"]);
    c.refused();
    assert!(!p.exists("docs/new-one.md"));
    let c = p.md(&["add", "a00013", "--title", "Fine", "--title", "Bad", "--body", "## heading"]);
    c.refused();
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH);
}

#[test]
fn a_body_with_a_heading_is_refused() {
    let p = Project::sample();
    for body in ["text\n\n## inner", "Title\n=====", "# top"] {
        let c = p.md(&["add", "a00013", "--title", "X", "--body", body]);
        assert!(c.refused().contains("the body contains a heading"), "{body}: {}", c.out);
        let c = p.md(&["update", "a00013", "--body", body]);
        assert!(c.refused().contains("the body contains a heading"), "{body}: {}", c.out);
    }
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH);
}

#[test]
fn a_body_that_leaves_a_fence_open_is_refused() {
    let p = Project::sample();
    // Would swallow `### Fields` and `## Id`.
    let c = p.md(&["update", "a00011", "--body", "Code:\n\n```\nlet x = 1;"]);
    assert!(c.refused().contains("would turn 2 headings in docs/arch.md into plain text"), "{}", c.out);
    // At the end of the file there is nothing to swallow, but the next add would be.
    let c = p.md(&["add", "a00013", "--title", "Open", "--body", "```\nnever closed"]);
    assert!(c.refused().contains("would end inside an open code fence"), "{}", c.out);
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH);
}

#[test]
fn a_fenced_heading_in_a_body_is_fine() {
    let p = Project::sample();
    p.md(&["update", "a00013", "--body", "Example:\n\n```markdown\n## not a node\n```"]).ok();
    let out = p.md(&["show", "a00013", "--info"]).ok().to_string();
    assert!(out.contains("  nodes      1\n"), "{out}");
}

#[test]
fn update_title_keeps_level_and_id() {
    let p = Project::sample();
    let out = p.md(&["update", "a00012", "--title", "Fields and sizes"]).ok().to_string();
    insta::assert_snapshot!(out, @"updated a00012 · lines 13-15");
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH.replace("### Fields {a00012}", "### Fields and sizes {a00012}"));
}

#[test]
fn update_body_replaces_the_whole_body() {
    let p = Project::sample();
    let out = p.md(&["update", "a00011", "--body", "One.\nTwo.\nThree."]).ok().to_string();
    insta::assert_snapshot!(out, @"updated a00011 · lines 9-13");
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH.replace("A node is a heading.\n", "One.\nTwo.\nThree.\n"));
}

#[test]
fn update_lines_replaces_only_those_lines() {
    let p = Project::sample();
    p.md(&["update", "a00011", "--body", "One.\nTwo.\nThree."]).ok();
    let out = p.md(&["update", "a00011", "--lines", "11-11", "--body", "Deux.\nZwei."]).ok().to_string();
    insta::assert_snapshot!(out, @"updated a00011 · lines 9-14");
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH.replace("A node is a heading.\n", "One.\nDeux.\nZwei.\nThree.\n"));
    // Deleting lines: an empty body.
    p.md(&["update", "a00011", "--lines", "11-12", "--body", ""]).ok();
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH.replace("A node is a heading.\n", "One.\nThree.\n"));
}

#[test]
fn update_lines_outside_the_body_is_refused() {
    let p = Project::sample();
    let c = p.md(&["update", "a00011", "--lines", "12-14", "--body", "x"]);
    assert_eq!(c.out, "refused: lines 12-14 are outside the body of a00011, which is lines 9-11\ninstead: mdmap show a00011\n");
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH);
    let c = p.md(&["update", "a00011", "--lines", "10-10"]);
    assert!(c.refused().contains("--lines needs --body"));
}

#[test]
fn update_old_new_replaces_each_text_found_once_in_the_body() {
    let p = Project::sample();
    let out = p.md(&["update", "a00013", "--old", "random", "--new", "six hex, random", "--old", "the root", "--new", "the docs"]).ok().to_string();
    insta::assert_snapshot!(out, @"updated a00013 · lines 17-18");
    let arch = SAMPLE_ARCH.replace("Ids are random. See [the root]", "Ids are six hex, random. See [the docs]");
    assert_eq!(p.read("docs/arch.md"), arch);
    // Every pair is found in the body as it was; a text may span lines, and an empty --new deletes.
    p.md(&["update", "a00011", "--body", "One.\nTwo.\nThree."]).ok();
    p.md(&["update", "a00011", "--old", "One.\nTwo.\n", "--new", "", "--old", "Three.", "--new", "Two."]).ok();
    assert_eq!(p.read("docs/arch.md"), arch.replace("A node is a heading.\n", "Two.\n"));
}

#[test]
fn update_old_found_no_time_twice_or_overlapping_is_refused_and_writes_nothing() {
    let p = Project::sample();
    let c = p.md(&["update", "a00013", "--old", "random", "--new", "x", "--old", "missing", "--new", "y"]);
    assert_eq!(c.out, "refused: --old \"missing\" is found 0 times in the body of a00013, not once\ninstead: mdmap show a00013\n");
    // Only the body of the node counts, not the rest of its file.
    assert!(p.md(&["update", "a00011", "--old", "Ids", "--new", "x"]).refused().contains("is found 0 times"));
    assert!(p.md(&["update", "a00013", "--old", "o", "--new", "x"]).refused().contains("is found 3 times in the body of a00013, not once; give more of the text around it"));
    assert!(p.md(&["update", "a00013", "--old", "Ids are", "--new", "x", "--old", "are random", "--new", "y"]).refused().contains("overlap"));
    assert!(p.md(&["update", "a00013", "--old", "random"]).refused().contains("needs --new"));
    assert!(p.md(&["update", "a00013", "--new", "x"]).refused().contains("comes right after --old"));
    assert!(p.md(&["update", "a00013", "--old", "random", "--new", "x", "--body", "y"]).refused().contains("take the place of --body"));
    assert!(p.md(&["update", "a00013", "--append", "y", "--old", "random", "--new", "x"]).refused().contains("--append takes the place"));
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH);
}

#[test]
fn update_old_counts_every_place_it_starts_overlapping_ones_too() {
    let p = Project::sample();
    p.md(&["update", "a00011", "--body", "aaa"]).ok();
    assert!(p.md(&["update", "a00011", "--old", "aa", "--new", "b"]).refused().contains("is found 2 times"));
    p.md(&["update", "a00011", "--old", "aaa", "--new", "b"]).ok();
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH.replace("A node is a heading.\n", "b\n"));
}

#[test]
fn update_old_new_keeps_the_rules_of_every_write() {
    let p = Project::sample();
    // With --title in one call; level and id stay.
    p.md(&["update", "a00013", "--title", "Ids", "--old", "random", "--new", "fixed"]).ok();
    let arch = SAMPLE_ARCH.replace("## Id {a00013}\n\nIds are random.", "## Ids {a00013}\n\nIds are fixed.");
    assert_eq!(p.read("docs/arch.md"), arch);
    // A new text may not bring a heading, nor leave a fence open.
    assert!(p.md(&["update", "a00011", "--old", "A node", "--new", "## Sub\n\nA node"]).refused().contains("contains a heading"));
    assert!(p.md(&["update", "a00011", "--old", "A node", "--new", "```\nA node"]).refused().contains("fence"));
    // The same --old twice overlaps itself; an empty --old is refused.
    assert!(p.md(&["update", "a00011", "--old", "node", "--new", "x", "--old", "node", "--new", "y"]).refused().contains("overlap"));
    assert!(p.md(&["update", "a00011", "--old", "", "--new", "y"]).refused().contains("it is empty"));
    assert!(p.md(&["update", "a00011", "--old", "node", "--new"]).refused().contains("--new needs a value"));
    assert_eq!(p.read("docs/arch.md"), arch);
    // The text above the first heading of a file is the body of the file; the heading under it is not.
    p.md(&["update", "docs/README.md", "--body", "The docs of the sample."]).ok();
    p.md(&["update", "docs/README.md", "--old", "of the sample", "--new", "here"]).ok();
    assert_eq!(p.read("docs/README.md"), format!("The docs here.\n\n{SAMPLE_README}"));
    assert!(p.md(&["update", "docs/README.md", "--old", "# Docs", "--new", "x"]).refused().contains("is found 0 times"));
    // Adjacent texts do not overlap.
    p.md(&["update", "a00011", "--old", "A node", "--new", "One node", "--old", " is a heading", "--new", " is one heading"]).ok();
    assert_eq!(p.read("docs/arch.md"), arch.replace("A node is a heading.", "One node is one heading."));
}

#[test]
fn ids_do_not_change_when_writing() {
    let p = Project::sample();
    let before = p.ids_in("docs/arch.md");
    p.md(&["update", "a00011", "--title", "Renamed", "--body", "New."]).ok();
    p.md(&["add", "a00010", "--title", "More"]).ok();
    let after = p.ids_in("docs/arch.md");
    assert_eq!(&after[..before.len()], &before[..]);
    assert_eq!(after.len(), before.len() + 1);
}

#[test]
fn remove_a_section_removes_its_byte_range_and_subtree() {
    let p = Project::sample();
    let out = p.md(&["remove", "a00011"]).ok().to_string();
    insta::assert_snapshot!(out, @"removed ## Node {a00011} · 2 nodes");
    assert_eq!(
        p.read("docs/arch.md"),
        SAMPLE_ARCH.replace("## Node {a00011}\n\nA node is a heading.\n\n### Fields {a00012}\n\nTitle, summary, body.\n\n", "")
    );
    assert!(p.md(&["show", "a00012", "--info"]).refused().contains("no node with id a00012"));
}

#[test]
fn an_add_and_a_remove_of_the_same_node_leave_the_file_as_it_was() {
    let p = Project::sample();
    let before = p.read("docs/arch.md");
    p.md(&["add", "a00010", "--title", "Edge", "--body", "Edges link nodes."]).ok();
    let id = p.id_of("docs/arch.md", "Edge");
    p.md(&["remove", id.as_str()]).ok();
    assert_eq!(p.read("docs/arch.md"), before, "the last section takes the blank line above it too");
}

#[test]
fn remove_a_file_or_a_folder_by_its_path() {
    let p = Project::sample();
    let out = p.md(&["remove", "docs/findings/B-013-language.md"]).ok().to_string();
    insta::assert_snapshot!(out, @"removed docs/findings/B-013-language.md · 3 nodes");
    assert!(!p.exists("docs/findings/B-013-language.md"));
    // The heading of a README is a heading: removing it leaves the file.
    p.md(&["remove", "a00030"]).ok();
    assert_eq!(p.read("docs/findings/README.md"), "");
    let out = p.md(&["remove", "docs/findings"]).ok().to_string();
    insta::assert_snapshot!(out, @"removed docs/findings/ · 7 nodes");
    assert!(!p.exists("docs/findings"));
}

/// Two files for `move`: a heading with one under it, and a place to go.
fn two_files(p: &common::Project) {
    p.write("docs/a.md", "# A {b00001}\n\nIntro.\n\n## Keep {b00002}\n\nk\n\n## Go {b00003}\n\ng\n\n### Under {b00004}\n\nu\n");
    p.write("docs/b.md", "# B {b00005}\n\nb\n\n## Last {b00006}\n\nl\n");
}

#[test]
fn move_takes_a_heading_and_all_under_it_to_another_file_ids_kept() {
    let p = Project::sample();
    two_files(&p);
    let out = p.md(&["move", "b00003", "b00005"]).ok().to_string();
    assert_eq!(out, "moved ## Go {b00003} · 2 nodes · docs/b.md:9\n");
    assert_eq!(p.read("docs/a.md"), "# A {b00001}\n\nIntro.\n\n## Keep {b00002}\n\nk\n");
    assert_eq!(p.read("docs/b.md"), "# B {b00005}\n\nb\n\n## Last {b00006}\n\nl\n\n## Go {b00003}\n\ng\n\n### Under {b00004}\n\nu\n");
}

#[test]
fn move_under_a_heading_shifts_every_level() {
    let p = Project::sample();
    two_files(&p);
    p.md(&["move", "b00003", "b00002"]).ok();
    assert_eq!(p.read("docs/a.md"), "# A {b00001}\n\nIntro.\n\n## Keep {b00002}\n\nk\n\n### Go {b00003}\n\ng\n\n#### Under {b00004}\n\nu\n");
    // Up again, to the end of a file: one level below the file is a #.
    p.md(&["move", "b00004", "docs/b.md"]).ok();
    assert_eq!(p.read("docs/b.md"), "# B {b00005}\n\nb\n\n## Last {b00006}\n\nl\n\n# Under {b00004}\n\nu\n");
}

#[test]
fn move_refuses_what_it_cannot_do_and_writes_nothing() {
    let p = Project::sample();
    two_files(&p);
    assert!(p.md(&["move", "b00003", "b00004"]).refused().contains("b00004 is inside b00003"));
    assert!(p.md(&["move", "docs/a.md", "b00005"]).refused().contains("move takes a heading"));
    assert!(p.md(&["move", "b00003", "docs"]).refused().contains("is a folder"));
    assert!(p.md(&["move", "b00003"]).refused().contains("move takes a heading and where it goes"));
    p.write("docs/deep.md", "# D1 {d00001}\n\n## D2 {d00002}\n\n### D3 {d00003}\n\n#### D4 {d00004}\n\n##### D5 {d00005}\n");
    assert!(p.md(&["move", "b00003", "d00005"]).refused().contains("markdown stops at 6"));
    assert!(p.read("docs/a.md").contains("## Go {b00003}"));
}

#[test]
fn a_root_cannot_be_removed() {
    let p = Project::sample();
    let c = p.md(&["remove", "docs/"]);
    assert!(c.refused().contains("docs/ is a root; remove it from roots in mdmap.yml"), "{}", c.out);
    assert!(p.exists("docs/README.md"));
}

#[test]
fn writes_leave_no_temp_files_behind() {
    let p = Project::sample();
    p.md(&["update", "a00011", "--body", "x"]).ok();
    let leftovers: Vec<_> = std::fs::read_dir(p.path("docs"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("mdmap-tmp"))
        .collect();
    assert!(leftovers.is_empty());
}

#[test]
fn new_ids_are_unique_six_hex_characters() {
    let p = Project::sample();
    let titles: Vec<String> = (0..40).map(|i| format!("T{i}")).collect();
    let mut args = vec!["add", "a00013"];
    for t in &titles {
        args.push("--title");
        args.push(t);
    }
    p.md(&args).ok();
    let ids = p.ids_in("docs/arch.md");
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
    assert!(ids.iter().all(|i| is_id(i)), "{ids:?}");
}

#[test]
fn add_refuses_a_file_with_broken_structure() {
    let p = Project::sample();
    p.write("docs/setext.md", "# Top {e00001}\n\nDeep {e00002}\n----\n");
    let c = p.md(&["add", "e00001", "--title", "X"]);
    assert!(c.refused().contains("setext heading"), "{}", c.out);
}

#[test]
fn update_append_puts_the_text_on_the_lines_after_the_body() {
    let p = Project::sample();
    p.md(&["update", "a00013", "--append", "- one more."]).ok();
    assert_eq!(p.md(&["show", "a00013"]).ok(), "## Id {a00013}\n18  Ids are random. See [the root](README.md).\n19  - one more.\n");
    p.md(&["update", "a00013", "--append", "x", "--body", "y"]).refused();
}

#[test]
fn update_a_file_changes_the_text_above_its_first_heading() {
    let p = Project::sample();
    let out = p.md(&["update", "docs/README.md", "--body", "The docs of the sample."]).ok().to_string();
    insta::assert_snapshot!(out, @"updated docs/README.md · lines 1-2");
    assert_eq!(p.read("docs/README.md"), format!("The docs of the sample.\n\n{SAMPLE_README}"));
    assert_eq!(p.md(&["show", "docs/README.md"]).ok(), "docs/README.md\n1  The docs of the sample.\n");
    // A file has no title of its own, a folder no text.
    assert!(p.md(&["update", "docs/README.md", "--title", "X"]).refused().contains("its name is its path"));
    assert!(p.md(&["update", "docs/", "--body", "X"]).refused().contains("docs/ is a folder"));
}
