//! `add --template`: a heading tree from the config, under a new node or an
//! existing one, bodies given with `--fill`.

mod common;
use common::Md;
use common::{Project, SAMPLE_ARCH};

const CONFIG: &str = "roots: [docs/]
templates:
  bug:
    - Symptom
    - Repro:
        - Steps
        - Expected
    - Remedy
";

fn project() -> Project {
    let p = Project::sample();
    p.md_config(CONFIG);
    p
}

#[test]
fn a_template_under_a_directory_makes_a_file_with_every_heading() {
    let p = project();
    let out = p.md(&["add", "docs/findings", "--title", "Search empty", "--template", "bug", "--fill", "Repro/Steps", "Run it."]).ok().to_string();
    assert_eq!(out.lines().count(), 6, "{out}");
    let f = "docs/findings/search-empty.md";
    let id = |t: &str| p.id_of(f, t);
    assert_eq!(
        p.read(f),
        format!(
            "# Search empty {{{}}}\n\n## Symptom {{{}}}\n\n## Repro {{{}}}\n\n### Steps {{{}}}\n\nRun it.\n\n### Expected {{{}}}\n\n## Remedy {{{}}}\n",
            id("Search empty"),
            id("Symptom"),
            id("Repro"),
            id("Steps"),
            id("Expected"),
            id("Remedy")
        )
    );
}

#[test]
fn a_template_under_a_section_starts_two_levels_down() {
    let p = project();
    p.md(&["add", "a00013", "--title", "Bug", "--template", "bug", "--fill", "Remedy", "Fix it."]).ok();
    let id = |t: &str| p.id_of("docs/arch.md", t);
    assert_eq!(
        p.read("docs/arch.md"),
        format!(
            "{SAMPLE_ARCH}\n### Bug {{{}}}\n\n#### Symptom {{{}}}\n\n#### Repro {{{}}}\n\n##### Steps {{{}}}\n\n##### Expected {{{}}}\n\n#### Remedy {{{}}}\n\nFix it.\n",
            id("Bug"),
            id("Symptom"),
            id("Repro"),
            id("Steps"),
            id("Expected"),
            id("Remedy")
        )
    );
}

#[test]
fn a_template_without_a_title_goes_under_the_node_itself() {
    let p = project();
    p.md(&["add", "a00013", "--template", "bug", "--fill", "Symptom", "Empty."]).ok();
    let id = |t: &str| p.id_of("docs/arch.md", t);
    assert_eq!(
        p.read("docs/arch.md"),
        format!(
            "{SAMPLE_ARCH}\n### Symptom {{{}}}\n\nEmpty.\n\n### Repro {{{}}}\n\n#### Steps {{{}}}\n\n#### Expected {{{}}}\n\n### Remedy {{{}}}\n",
            id("Symptom"),
            id("Repro"),
            id("Steps"),
            id("Expected"),
            id("Remedy")
        )
    );
}

#[test]
fn a_wrong_template_or_path_is_refused_and_nothing_is_written() {
    let p = project();
    let c = p.md(&["add", "a00013", "--title", "Bug", "--template", "nope"]);
    assert!(c.refused().contains("the templates are bug"), "{}", c.out);
    let c = p.md(&["add", "a00013", "--title", "Bug", "--template", "bug", "--fill", "Steps", "x"]);
    assert!(c.refused().contains("its headings are Symptom, Repro, Repro/Steps, Repro/Expected, Remedy"), "{}", c.out);
    let c = p.md(&["add", "docs/findings", "--template", "bug"]);
    assert!(c.refused().contains("instead: mdmap add docs/findings/ --title <title> --template"), "{}", c.out);
    let c = p.md(&["add", "a00013", "--fill", "Symptom", "x"]);
    assert!(c.refused().contains("--fill belongs to the --template before it"), "{}", c.out);
    assert_eq!(p.read("docs/arch.md"), SAMPLE_ARCH);
    assert!(!p.exists("docs/findings/bug.md"));
}

#[test]
fn a_template_too_deep_for_its_place_is_refused() {
    let p = project();
    p.write("docs/deep.md", "# D1 {d00001}\n\n## D2 {d00002}\n\n### D3 {d00003}\n\n#### D4 {d00004}\n");
    let c = p.md(&["add", "d00004", "--title", "Bug", "--template", "bug"]);
    assert!(
        c.refused().starts_with("refused: d00004 is at level 4; the new headings need 3 levels below it and markdown stops at 6\n"),
        "{}",
        c.out
    );
    // Without the title the template fits: levels 5 and 6.
    p.md(&["add", "d00004", "--template", "bug"]).ok();
    assert!(p.read("docs/deep.md").contains("\n###### Steps {"));
}

#[test]
fn the_config_refuses_a_template_mdmap_cannot_write() {
    let p = Project::sample();
    p.md_config("roots: [docs/]\ntemplates:\n  t:\n    - A/B\n");
    let c = p.md(&["ls"]);
    assert!(c.refused().contains("template t: \"A/B\" is not a title"), "{}", c.out);
    p.md_config("roots: [docs/]\ntemplates:\n  t:\n    - A\n    - A\n");
    let c = p.md(&["ls"]);
    assert!(c.refused().contains("template t: \"A\" twice under one heading"), "{}", c.out);
}
