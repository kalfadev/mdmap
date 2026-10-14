//! `mdmap export`: the whole tree as one JSON document, the input of the
//! graph `mdmap serve` draws.

use serde_json::{json, Value};

use crate::graph::{Act, Graph};

/// A stable handle for a node: a heading's id, a folder's or a file's path.
pub fn key(g: &Graph, i: usize) -> String {
    g.key(i)
}

/// The keys of the nodes a read may reach; what `deny.read` closes is left out.
fn keys(g: &Graph, v: &[usize]) -> Vec<String> {
    v.iter().filter(|&&c| !g.denied(c, Act::Read)).map(|&c| key(g, c)).collect()
}

pub fn export(g: &Graph) -> Value {
    let nodes: Vec<Value> = (0..g.nodes.len())
        .filter(|&i| !g.denied(i, Act::Read))
        .map(|i| {
            let n = &g.nodes[i];
            json!({
                "key": key(g, i),
                "id": n.id,
                "heading": g.heading(i),
                "level": n.level,
                "kind": n.kind.name(),
                "title": n.title,
                "path": g.location(i),
                "summary": g.summary(i),
                "body": g.body_text(i),
                "body_chars": n.body_chars,
                "subtree": n.subtree,
                "nodes": n.nodes,
                "max_depth": n.max_depth,
                "parent": n.parent.map(|p| key(g, p)),
                "children": keys(g, &n.children),
                "links": keys(g, &n.links),
                "backlinks": keys(g, &n.backlinks),
            })
        })
        .collect();
    json!({
        "version": 1,
        "roots": keys(g, &g.roots),
        "nodes": nodes,
    })
}
