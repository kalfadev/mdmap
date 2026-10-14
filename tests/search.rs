//! `search`: the nodes that hold a text, each as a callable heading.

mod common;
use common::Md;
use common::Project;

#[test]
fn search_prints_only_the_headings_in_tree_order() {
    let p = Project::sample();
    let out = p.md(&["search", "random"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    ## Id {a00013}
    ## Why random ids {a00021}
    ");
}

#[test]
fn case_is_ignored() {
    let p = Project::sample();
    assert_eq!(p.md(&["search", "RANDOM"]).ok(), p.md(&["search", "random"]).ok());
}

#[test]
fn the_text_matches_whole_words_only() {
    let p = Project::sample();
    // `Id` in a title and in two bodies; `ids` and `a00013` are other words.
    let out = p.md(&["search", "id"]).ok().to_string();
    insta::assert_snapshot!(out, @"
    ## How to read {a00002}
    ## Id {a00013}
    ## Why random ids {a00021}
    ");
    assert_eq!(p.md(&["search", "rand"]).ok(), "no node holds \"rand\"\n");
    // An edge that is not a word character needs nothing next to it.
    assert!(p.md(&["search", "(arch.md"]).ok().contains("{a00021}"));
}

#[test]
fn summary_adds_the_summary_and_body_the_body() {
    let p = Project::sample();
    let out = p.md(&["search", "design", "--summary"]).ok().to_string();
    // A file is found by its name too.
    insta::assert_snapshot!(out, @"
    # Architecture {a00010}
      The design in one file.
    docs/design.md
    # Design {a00020}
      Why things are the way they are.
    ");
    let out = p.md(&["search", "random", "--body"]).ok().to_string();
    assert_eq!(
        out,
        "## Id {a00013}\n18  Ids are random. See [the root](README.md).\n\n## Why random ids {a00021}\n7  The rule is [in the Id section](arch.md#a00013).\n"
    );
}

#[test]
fn with_body_summary_is_dropped() {
    let p = Project::sample();
    assert_eq!(p.md(&["search", "random", "--summary", "--body"]).ok(), p.md(&["search", "random", "--body"]).ok());
}

#[test]
fn no_match_is_one_line_and_exit_zero() {
    let p = Project::sample();
    assert_eq!(p.md(&["search", "nothing here"]).ok(), "no node holds \"nothing here\"\n");
}

#[test]
fn search_without_text_is_refused() {
    let p = Project::sample();
    assert!(p.md(&["search"]).refused().starts_with("refused: search takes the text to find\n"));
    // The shell drops the quotes, so only several words show that they were missing.
    let c = p.md(&["search", "byte", "range"]);
    assert!(c.refused().starts_with("refused: search takes one text: quote it\n"));
    assert_eq!(c.instead().unwrap(), "mdmap search \"byte range\"");
}

#[test]
fn json_carries_the_same_content() {
    let p = Project::sample();
    let v = p.md(&["search", "random", "--json"]).json();
    assert_eq!(v["nodes"][0]["id"], "a00013");
    assert_eq!(v["nodes"][1]["id"], "a00021");
    assert!(v["nodes"][0].get("summary").is_none() && v["nodes"][0].get("body").is_none(), "{v}");
    let v = p.md(&["search", "random", "--summary", "--body", "--json"]).json();
    assert!(v["nodes"][0].get("summary").is_none(), "{v}");
    assert_eq!(v["nodes"][0]["body"], "Ids are random. See [the root](README.md).\n");
}
