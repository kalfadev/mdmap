//! Links, backlinks, and what `lint` says about them.

mod common;
use common::Md;
use common::Project;

/// The nodes of one kind of row of `show --info`: a heading by its id, a
/// folder or a file by its path.
fn rows(out: &str, label: &str) -> Vec<String> {
    // A label is printed once; following rows of the same kind are blank.
    let mut in_group = false;
    let mut found = Vec::new();
    for l in out.lines() {
        let t = l.trim_start();
        if t.starts_with(label) {
            in_group = true;
        } else if ["parent", "children", "link", "backlink"].iter().any(|x| t.starts_with(x)) || t.is_empty() {
            in_group = false;
        }
        if in_group {
            match l.find('{') {
                Some(open) => found.push(l[open + 1..open + 7].to_string()),
                None => found.push(l[12..].split_whitespace().next().unwrap().to_string()),
            }
        }
    }
    found
}

#[test]
fn a_link_to_path_and_id_goes_to_the_id() {
    let p = Project::sample();
    let out = p.md(&["show", "a00021", "--info"]).ok().to_string();
    assert_eq!(rows(&out, "link"), ["a00013"], "{out}");
}

#[test]
fn backlinks_list_every_node_that_links_here() {
    let p = Project::sample();
    let out = p.md(&["show", "a00013", "--info"]).ok().to_string();
    assert_eq!(rows(&out, "backlink"), ["a00002", "a00021"], "{out}");
}

#[test]
fn link_forms() {
    let p = Project::sample();
    p.write(
        "docs/forms.md",
        "# Forms {c00020}\n\n\
         [bare fragment](#a00012)\n\
         [file](design.md)\n\
         [dir](findings/)\n\
         [external](https://example.com/x.md)\n\
         [outside](../README.md)\n\
         [image](pic.png)\n\
         [wrong path, right id](design.md#a00011)\n",
    );
    p.write("README.md", "# Repo\n");
    let out = p.md(&["show", "c00020", "--info"]).ok().to_string();
    // A path alone goes to the file or the folder.
    assert_eq!(rows(&out, "link"), ["a00012", "docs/design.md", "docs/findings/", "a00011"], "{out}");
}

#[test]
fn relative_paths_are_read_from_the_linking_file() {
    let p = Project::sample();
    p.write(
        "docs/findings/2026/B-043-turns.md",
        "# B-043 Turns {a00041}\n\nSee [the language record](../B-013-language.md) and [arch](../../arch.md).\n",
    );
    let out = p.md(&["show", "a00041", "--info"]).ok().to_string();
    assert_eq!(rows(&out, "link"), ["docs/findings/B-013-language.md", "docs/arch.md"], "{out}");
}

#[test]
fn a_link_survives_a_moved_file_because_the_id_does() {
    let p = Project::sample();
    std::fs::rename(p.path("docs/arch.md"), p.path("docs/findings/arch.md")).unwrap();
    let out = p.md(&["show", "a00021", "--info"]).ok().to_string();
    assert_eq!(rows(&out, "link"), ["a00013"], "{out}");
    // …and lint notices the path is stale.
    let out = p.md(&["lint"]).out;
    assert!(out.contains("docs/design.md:7   stale path → arch.md#a00013 (node is in docs/findings/arch.md)"), "{out}");
}

#[test]
fn lint_reports_broken_links_and_exits_nonzero() {
    let p = Project::sample();
    p.write("docs/broken.md", "# Broken {c00021}\n\nSee [gone](arch.md#9f3c72), [missing](nope.md) and [slug](arch.md#node).\n");
    let c = p.md(&["lint"]);
    assert_eq!(c.code, 1);
    assert!(c.out.contains("docs/broken.md:3   broken link → arch.md#9f3c72 (no such id)"), "{}", c.out);
    assert!(c.out.contains("broken link → nope.md (no such file)"), "{}", c.out);
    assert!(c.out.contains("broken link → arch.md#node (fragment is not an id)"), "{}", c.out);
}

#[test]
fn removing_a_linked_node_is_allowed_and_lint_reports_the_broken_link() {
    let p = Project::sample();
    p.md(&["remove", "a00013"]).ok();
    let out = p.md(&["lint"]).out;
    assert!(out.contains("docs/README.md:7   broken link → arch.md#a00013 (no such id)"), "{out}");
    assert!(out.contains("docs/design.md:7   broken link → arch.md#a00013 (no such id)"), "{out}");
}


#[test]
fn lint_reports_verify_without_a_span() {
    let p = Project::sample();
    p.write("docs/v.md", "# V {c00024}\n\nThe count.\n\n<!-- verify: ls -->\n\nNo code here.\n\n## Next {c00025}\n\n`too late`\n");
    let out = p.md(&["lint"]).out;
    assert!(out.contains("docs/v.md:5   verify without a span"), "{out}");
}

#[test]
fn lint_reports_an_ignore_pattern_that_matches_nothing() {
    let p = Project::new();
    p.write("mdmap.yml", "roots: [docs/]\nignore: [drafts/**, docs/drafts/]\n");
    p.write("docs/README.md", "# Docs {c00030}\n");
    p.write("docs/drafts/wip.md", "# Wip {c00031}\n");
    let c = p.md(&["lint"]);
    assert_eq!(c.code, 1, "{}", c.out);
    assert_eq!(c.out, "ignore docs/drafts/   matches no path under the roots, so it hides nothing\n");
}

#[test]
fn a_clean_corpus_lints_clean() {
    let p = Project::sample();
    let c = p.md(&["lint"]);
    assert_eq!(c.out, "no lint findings\n");
    assert_eq!(c.code, 0);
}

#[test]
fn lint_reports_an_id_in_code_no_heading_has_and_a_bold_title_that_is_not_the_heading() {
    let p = Project::sample();
    p.write("docs/refs.md", "# Refs {c00022}\n\nSee **Id** `{a00013}`, **Architecture: Id** `{a00013}`, **Ids** `{a00013}` and `{9f3c72}`; `a00013` is no reference.\n");
    let c = p.md(&["lint"]);
    assert_eq!(c.code, 1);
    assert_eq!(c.out, "docs/refs.md:3   stale title → **Ids** {a00013} (the heading is Id)\ndocs/refs.md:3   broken id → {9f3c72} (no such id)\n");
}

#[test]
fn lint_reads_ids_only_in_inline_code_and_titles_as_written() {
    let p = Project::sample();
    p.write(
        "docs/refs.md",
        "# Refs {c00022}\n\n## The `ls` verb {c00023}\n\nSee **The `ls` verb** `{c00023}`, **Note**: `{a00013}`, **Id**`{a00013}` and **Docs: Architecture: Id** `{a00013}`.\n\n```\n**Ids** `{9f3c72}`\n```\n\n    `{9f3c72}`\n",
    );
    let c = p.md(&["lint"]);
    assert_eq!(c.out, "no lint findings\n");
}

#[test]
fn lint_names_the_line_after_frontmatter_and_tells_no_title_of_a_denied_heading() {
    let p = Project::sample();
    let arch = p.read("docs/arch.md");
    p.write("docs/arch.md", &(arch + "\nSee **Wrong** `{a00011}` and `{9f3c72}`.\n"));
    let c = p.md(&["lint"]);
    assert_eq!(c.out, "docs/arch.md:20   stale title → **Wrong** {a00011} (the heading is Node)\ndocs/arch.md:20   broken id → {9f3c72} (no such id)\n");
    p.md_config("deny:\n  read:\n    ids: [a00011]\n");
    let c = p.md(&["lint"]);
    assert_eq!(c.out, "docs/arch.md:20   broken id → {9f3c72} (no such id)\n");
}
