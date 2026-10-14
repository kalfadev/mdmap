# Overview {8905cd}

What mdmap is, the parts it is made of, how one call flows through them, and where each part lives. Read this first; the **Guide** has every verb and setting, `mdmap help` the same list in the binary.

## What mdmap is {10ddcd}

An agent that opens a file to find one section pays for the whole file. mdmap reads the markdown under the roots of a project as one tree of folders, files and headings, so the agent opens one heading, not a file, and sees sizes before it reads. It calls no model and needs no other tool: it is one binary that reads `mdmap.yml` at the root of a project.

## Concepts {2aba67}

- **Corpus**: the markdown under the `roots` of `mdmap.yml`, minus `ignore`.
- **Node**: a folder, named by its path, with no body; a file, named by its path, its body the text before its first heading; a heading, named by its six-hex id (`{2aba67}` at the end of the heading line), random and never changed.
- **Tree**: a folder holds its files, then its subfolders; a file the headings under no other heading; a heading the headings under it with more `#`, so levels may skip.
- **Size**: characters of the body `show` prints; past `warn.body` a node comes with advice to split it.
- **Verify block**: a fenced block under `<!-- verify: <command> -->` is the output of that command: `verify` compares it, `verify --fix` rewrites it, `show` counts its lines instead of printing them. What a line may run is `verify.run` of `mdmap.yml`.
- **Template**: a named heading tree in `mdmap.yml` that `add --template` writes in one call, the bodies given with `--fill`.

## One call {bb1876}

What happens when the agent runs `mdmap show 3f9c2a`:

```
mdmap show 3f9c2a (cli::main)
  1. find the project: the nearest mdmap.yml upward, else a .git (the defaults apply)
  2. load the settings (config.rs), walk roots minus ignore into the tree (graph.rs)
  3. give every heading without an id one, under a lock on mdmap.yml when there is one (autofix: says so)
  4. resolve the id or path, run the verb (view, write, check, lint, verify, stats, export)
  5. print the text, or --json; a refusal names the next call
```

## Where things live {748d5a}

| path | what |
|---|---|
| `src/main.rs`, `src/lib.rs` | the binary, which calls `cli::main`, and the crate, which lists the modules (`8448e6`) |
| `src/cli.rs` | the entry: finds the project, takes the id lock, parses the arguments; `graph_verb` sends each verb to its `<verb>_verb` (`8448e6`) |
| `src/out.rs` | what a verb returns: `Output`, `Refusal`, `R<T>`; `atomic_write` (`8448e6`) |
| `src/config.rs` | `mdmap.yml` into `Config`, the templates with it (`256dda`) |
| `src/markdown.rs` | one file into a `Doc`: headings, links, paragraphs, code, verify comments (`256dda`) |
| `src/graph.rs` | the roots minus `ignore` into the `Graph`: nodes, sizes, links, backlinks (`256dda`) |
| `src/id.rs`, `src/slug.rs` | a new six-hex id; a title into a file name (`256dda`) |
| `src/view.rs` | `ls`, `show`, `search` (`5078e7`) |
| `src/stats.rs`, `src/export.rs` | `stats`, the corpus in numbers and what a call log opened; `export`, the tree as JSON (`5078e7`) |
| `src/write.rs` | `add`, `update`, `move`, `remove`, each result parsed in memory before it reaches disk (`c2b96f`) |
| `src/check.rs`, `src/lint.rs` | `check`: can the tree be read; `lint`: links, paths, verify comments, unused `ignore` (`3d7ee4`) |
| `src/verify.rs`, `src/exec.rs` | `verify`: a verify block against its command; `exec`: runs that command without a shell, under `verify.run` and a timeout (`3d7ee4`) |
| `tests/` | behaviour tests, in-process against temporary projects, one file per verb or area; `tests/common/mod.rs` holds the sample corpus (`e5a6a3`) |
| `mdmap.yml` | the settings of this corpus |
| `.github/workflows/test.yml` | the tests on Linux, macOS and Windows for every pull request and every push to `main` (**Guide: Contributing**) |
| `dist-workspace.toml`, `.github/workflows/release.yml` | the release: cargo-dist settings and the workflow `dist generate` writes from them (**Guide: Install**) |
| `scripts/sym.py` | the items of source files with their lines, through ast-grep; the `outline` verify blocks of **Code map** run it |
| `docs/` | this corpus |

The id after a file is its area in **Code map**, which lists every item of the file with its lines. A new source file gets its row here in the plan that adds it.
