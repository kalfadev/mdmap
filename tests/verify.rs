//! `mdmap verify`: a sentence tied to a command, which runs what `verify.run`
//! of the mdmap settings allows.

mod common;
use common::Md;
use common::Project;

fn project() -> Project {
    let p = Project::sample();
    p.md_config("verify:\n  run: [ls, echo, printf]\n");
    p.write("src/graph.rs", "");
    p.write("src/id.rs", "");
    p
}

const FILES: &str = "# Files {f00001}\n\nThe source files.\n\n<!-- verify: ls src/*.rs -->\n\n```\nsrc/graph.rs\nsrc/id.rs\n```\n\nProse after the span is not compared.\n";

#[test]
fn a_matching_fence_passes() {
    let p = project();
    p.write("docs/files.md", FILES);
    let c = p.md(&["verify"]);
    assert_eq!(c.code, 0, "{}", c.out);
    assert!(c.out.contains("f00001  Files    ok\n"), "{}", c.out);
    assert!(c.out.ends_with("\n25 nodes, 1 with verify, 0 failed\n"), "{}", c.out);
}

#[test]
fn a_stale_fence_fails_with_a_diff_and_nonzero_exit() {
    let p = project();
    p.write("docs/files.md", FILES);
    std::fs::rename(p.path("src/id.rs"), p.path("src/slug.rs")).unwrap();
    let c = p.md(&["verify"]);
    assert_eq!(c.code, 1);
    assert!(c.out.contains("f00001  Files    FAIL\n  - src/id.rs\n  + src/slug.rs\n"), "{}", c.out);
    assert!(c.out.ends_with("25 nodes, 1 with verify, 1 failed\n"), "{}", c.out);
}

#[test]
fn fix_rewrites_only_the_span() {
    let p = project();
    p.write("docs/files.md", FILES);
    std::fs::rename(p.path("src/id.rs"), p.path("src/slug.rs")).unwrap();
    let c = p.md(&["verify", "--fix"]);
    assert_eq!(c.code, 0, "{}", c.out);
    assert_eq!(p.read("docs/files.md"), FILES.replace("src/id.rs", "src/slug.rs"));
    assert_eq!(p.md(&["verify"]).code, 0);
}

#[test]
fn an_inline_span_holds_a_single_value() {
    let p = project();
    p.write("docs/ver.md", "# Version {f00002}\n\n<!-- verify: echo 1.2.3 -->\n\nThe version is `1.2.2`, and `this` is not compared.\n");
    let c = p.md(&["verify", "f00002"]);
    assert_eq!(c.code, 1);
    assert!(c.out.contains("  - 1.2.2\n  + 1.2.3\n"), "{}", c.out);
    assert!(c.out.ends_with("1 nodes, 1 with verify, 1 failed\n"), "{}", c.out);
    p.md(&["verify", "f00002", "--fix"]).ok();
    assert_eq!(
        p.read("docs/ver.md"),
        "# Version {f00002}\n\n<!-- verify: echo 1.2.3 -->\n\nThe version is `1.2.3`, and `this` is not compared.\n"
    );
}

#[test]
fn a_command_outside_the_allowlist_is_refused_in_the_report() {
    let p = project();
    p.write("docs/rm.md", "# Rm {f00003}\n\n<!-- verify: rm -rf src -->\n\n`x`\n");
    let c = p.md(&["verify"]);
    assert_eq!(c.code, 1);
    assert!(c.out.contains("f00003  Rm    refused\n  rm is not in verify.run of mdmap.yml\n"), "{}", c.out);
    assert!(p.exists("src/graph.rs"));
}

#[test]
fn no_shell_interpretation() {
    let p = project();
    p.write("docs/sh.md", "# Sh {f00004}\n\n<!-- verify: echo a; ls | wc -->\n\n`a; ls | wc`\n");
    p.md(&["verify"]).ok();
}

#[test]
fn an_allowed_command_cannot_chain_another() {
    let p = project();
    p.write("docs/chain.md", "# Chain {f00005}\n\n<!-- verify: ls src && rm -rf src -->\n\n`x`\n");
    p.md(&["verify"]);
    assert!(p.exists("src/graph.rs"), "&& is an argument of ls, not a second command");
}

#[test]
fn verify_without_a_span_does_not_run() {
    let p = project();
    p.write("docs/nospan.md", "# No span {f00005}\n\n<!-- verify: echo hi -->\n\nNothing.\n");
    let c = p.md(&["verify"]);
    assert_eq!(c.code, 0);
    assert!(c.out.contains("f00005  No span    no span\n"), "{}", c.out);
}

#[test]
fn verify_of_an_id_checks_only_that_node() {
    let p = project();
    p.write("docs/files.md", FILES);
    p.write("docs/ver.md", "# Version {f00002}\n\n<!-- verify: echo 1.2.3 -->\n\n`1.2.2`\n");
    let c = p.md(&["verify", "f00001"]);
    assert_eq!(c.code, 0, "{}", c.out);
    assert!(!c.out.contains("Version"), "{}", c.out);
}

#[test]
fn a_timeout_fails() {
    let p = project();
    p.md_config("verify:\n  run:\n    - {command: sleep, timeout: 1}\n");
    p.write("docs/slow.md", "# Slow {f00006}\n\n<!-- verify: sleep 5 -->\n\n`x`\n");
    let c = p.md(&["verify"]);
    assert_eq!(c.code, 1);
    assert!(c.out.contains("timeout"), "{}", c.out);
}

#[test]
fn verify_json() {
    let p = project();
    p.write("docs/files.md", FILES);
    let v = p.md(&["verify", "--json"]).json();
    assert_eq!(v["checks"][0]["id"], "f00001");
    assert_eq!(v["checks"][0]["status"], "ok");
}

#[test]
fn an_entry_that_names_a_command_runs_it_with_the_other_words_as_arguments() {
    let p = project();
    p.md_config("verify:\n  run:\n    - {command: say, run: \"printf %s-%s {args}\"}\n    - {command: plain, run: \"printf plain\"}\n");
    p.write("docs/say.md", "# Say {f00007}\n\n<!-- verify: say a b -->\n\n`a-b`\n\n## Plain {f00008}\n\n<!-- verify: plain -->\n\n`plain`\n");
    let c = p.md(&["verify"]);
    assert!(c.out.contains("f00007  Say") && c.out.contains("f00008  Plain"), "{}", c.out);
    assert!(c.out.ends_with("26 nodes, 2 with verify, 0 failed\n"), "the command of each entry ran: {}", c.out);
}
