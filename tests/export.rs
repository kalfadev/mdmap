//! `mdmap export`: the whole tree as one JSON document.

mod common;
use common::Md;
use common::Project;

#[test]
fn export_carries_every_node_with_its_place_and_its_links() {
    let p = Project::sample();
    let v = p.md(&["export"]).json();
    assert_eq!(v["version"], 1);
    assert_eq!(v["roots"], serde_json::json!(["docs/"]));
    let nodes = v["nodes"].as_array().unwrap().clone();
    let of = |key: &str| nodes.iter().find(|n| n["key"] == key).unwrap_or_else(|| panic!("no node {key} in the export")).clone();
    let folder = of("docs/");
    assert_eq!(folder["kind"], "folder");
    assert!(folder["parent"].is_null(), "a root has no parent: {folder}");
    let file = of("docs/arch.md");
    assert_eq!(file["kind"], "file");
    assert_eq!(file["parent"], "docs/");
    assert_eq!(file["children"], serde_json::json!(["a00010"]));
    let node = of("a00011");
    assert_eq!(node["kind"], "section");
    assert_eq!(node["heading"], "## Node {a00011}");
    assert_eq!(node["level"], 2);
    assert_eq!(node["title"], "Node");
    assert_eq!(node["path"], "docs/arch.md:8");
    // The body as it stands, the blank lines around it included; show trims it.
    assert_eq!(node["body"], "\nA node is a heading.\n\n");
    assert_eq!(node["summary"], "A node is a heading.");
    assert_eq!(node["parent"], "a00010");
    assert_eq!(node["children"], serde_json::json!(["a00012"]));
    // Both ends of a link are in: the node that links, and the node linked to.
    assert_eq!(of("a00002")["links"], serde_json::json!(["a00013"]));
    assert_eq!(of("a00013")["backlinks"], serde_json::json!(["a00002", "a00021"]));
    // Every node once, folders, files and headings together.
    let keys: Vec<&str> = nodes.iter().map(|n| n["key"].as_str().unwrap()).collect();
    for key in ["docs/findings/", "docs/findings/2026/", "docs/findings/B-013-language.md", "a00032"] {
        assert!(keys.contains(&key), "{key} is not in {keys:?}");
    }
    let unique: std::collections::BTreeSet<&str> = keys.iter().copied().collect();
    assert_eq!(keys.len(), unique.len(), "{keys:?}");
}
