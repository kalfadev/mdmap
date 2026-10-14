//! `stats`: which headings the log shows the agent opened.

mod common;

use common::Md;
use common::Project;

/// Two calls in one session: a00013 named twice, a00001 once.
fn with_log(p: &Project) -> &Project {
    p.write(
        ".harno/log/harno/s1.jsonl",
        "{\"n\":1,\"ts\":\"2026-01-01T00:00:00Z\",\"cwd\":\".\",\"verb\":\"run\",\"args\":[\"mdmap\",\"show\",\"a00013\"]}\n{\"n\":2,\"ts\":\"2026-01-01T00:00:01Z\",\"cwd\":\".\",\"verb\":\"run\",\"args\":[\"mdmap\",\"show\",\"a00013\",\"a00001\"]}\n",
    )
}

#[test]
fn stats_count_how_often_the_log_opened_each_heading() {
    let p = Project::sample();
    with_log(&p);
    let out = p.md(&["stats"]).ok().to_string();
    assert!(out.contains("headings  opened 2  never opened "), "{out}");
    assert!(out.contains("\nopened, most called first\n"), "{out}");
    assert!(out.contains("2  ## Id {a00013}"), "{out}");
    assert!(out.contains("1  # Docs {a00001}"), "{out}");
    assert!(out.contains("\nnever opened, biggest first\n"), "{out}");
    assert!(out.contains("## Decision record {a00032}"), "{out}");
}

#[test]
fn unread_prints_only_what_no_call_opened() {
    let p = Project::sample();
    with_log(&p);
    let out = p.md(&["stats", "--unread"]).ok().to_string();
    assert!(!out.contains("opened, most called first"), "{out}");
    assert!(!out.contains("## Id {a00013}"), "{out}");
    assert!(out.contains("## Decision record {a00032}"), "{out}");
}

#[test]
fn cost_ranks_the_opened_headings_by_size_times_calls() {
    let p = Project::sample();
    let call = |id: &str| format!("{{\"args\":[\"mdmap\",\"show\",\"{id}\"]}}\n");
    p.write(".harno/log/harno/s1.jsonl", &[call("a00032"), call("a00032"), call("a00032"), call("a00013")].concat());
    let out = p.md(&["stats"]).ok().to_string();
    assert!(out.find("{a00032}").unwrap() < out.find("{a00013}").unwrap(), "{out}");
    // A short body called three times costs less than a longer one called once.
    let out = p.md(&["stats", "--cost"]).ok().to_string();
    assert!(out.contains("\nopened, most costly first (chars × calls)\n"), "{out}");
    assert!(out.find("{a00013}").unwrap() < out.find("{a00032}").unwrap(), "{out}");
    assert!(out.contains(" chars × 3 calls\n"), "{out}");
    let v = p.md(&["stats", "--cost", "--json"]).json();
    assert_eq!(v["nodes"][0]["id"], "a00013");
    assert_eq!(v["nodes"][0]["cost"], v["nodes"][0]["chars"]);
    let never = v["nodes"].as_array().unwrap().iter().find(|n| n["id"] == "a00011").unwrap();
    assert_eq!(never["cost"], 0);
    let out = p.md(&["stats", "--cost", "--unread"]).ok().to_string();
    assert!(!out.contains("most costly") && !out.contains("{a00013}"), "{out}");
    assert!(p.md(&["stats", "--cost", "a00013"]).refused().contains("stats"));
}

#[test]
fn stats_count_the_corpus_itself() {
    let p = common::Project::sample();
    let v = p.md(&["stats", "--json"]).json();
    let out = p.md(&["stats"]).ok().to_string();
    assert!(
        out.starts_with(&format!("corpus: {} files, {} folders, {} headings, {} chars\n", v["files"], v["folders"], v["headings"], v["chars"])),
        "{out}"
    );
    assert!(v["files"].as_u64().unwrap() >= 7 && v["folders"] == 3, "{v}");
    assert_eq!(v["empty"], 0, "every heading of the sample has a body: {v}");
}

#[test]
fn without_a_log_every_heading_is_unopened() {
    let p = Project::sample();
    let out = p.md(&["stats"]).ok().to_string();
    assert!(out.contains("  opened 0  never opened "), "{out}");
}

#[test]
fn json_carries_the_same_numbers() {
    let p = Project::sample();
    with_log(&p);
    let v = p.md(&["stats", "--json"]).json();
    assert_eq!(v["opened"], 2);
    assert_eq!(v["nodes"][0]["id"], "a00013");
    assert_eq!(v["nodes"][0]["calls"], 2);
}
