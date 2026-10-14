# Guide {9e9f62}

Install mdmap, start a corpus, the settings and every verb.

## Install {743350}

From a checkout of this repository: `cargo install --path . --locked` puts `mdmap` into `~/.cargo/bin`; `mdmap help` shows it works. Releases: a tag `v<version>` pushed to GitHub runs `.github/workflows/release.yml`, which cargo-dist generates from `dist-workspace.toml` (`dist generate` after a change there): binaries for macOS, Linux and Windows on the release page, and an install script for macOS and Linux (`mdmap-installer.sh`) and one for Windows (`mdmap-installer.ps1`). The crate is not on crates.io.

To work on mdmap itself: `cargo build`, `cargo test`; the binary is `target/debug/mdmap`, and the installed copy is the one in use. `mdmap verify` needs python3 and ast-grep for the `outline` blocks of **Code map**.

## Start a corpus {d159cf}

In the directory that holds the docs, usually the repository root:

1. Write `mdmap.yml` with `roots`, the folders that hold the markdown (`[docs/]` by default), and `ignore`, what mdmap must not touch. `ignore` globs are relative to the root (`drafts/**`, not `docs/drafts/`); one that matches nothing is silent, so look at the tree in the next step, and `mdmap lint` names it. A project without `mdmap.yml` but with a `.git` runs on the defaults.
2. `mdmap ls`. The first call gives every heading an id (`{d159cf}` at the end of the heading line) and says so in an `autofix:` line. `mdmap check` reports what is left, `mdmap check --fix` repairs what it can.
3. Commit the settings and the ids. Ids never change: links, `CLAUDE.md` and a log name headings by them.

Then read with `ls`, `show`, `search` and write with `add`, `update`, `move`, `remove` (**Verbs**).

## Settings: mdmap.yml {b64ddd}

`mdmap.yml` at the root of the project; an unknown field is refused, and a project without the file runs on the defaults.

- `roots`: the folders of the corpus, `[docs/]` by default.
- `ignore`: globs relative to the root they are under, left out of the corpus. In the globs of `ignore` and `deny`, `*` stays inside one folder and `**` crosses folders, as in `.gitignore`: `*.md` is the files at the top of a root, `**/*.md` every file under it.
- `templates`: `<name>: [<title>, {<title>: [<title>…]}…]`, heading trees that `add --template <name>` writes, each body given by `--fill <path> <text>` in the same call, the path being titles joined by `/`; at most 5 levels, no `/` in a title, no title twice under one heading.
- `warn.body`: characters of prose past which a node comes with advice to split it, 8000 by default, 0 turns it off; the block under a verify comment is a command output and is not counted.
- `verify.run`: what a verify line may run, matched on its first words. An entry is the words themselves, and the line runs as it is written, or `{command, run, timeout}`, and `run` runs with `{args}` taking the other words of the line. There is no shell. `verify.timeout`: seconds, 120 by default.
- `deny`: what a verb may not reach, under `read`, `update` and `remove`, each with `paths`, globs relative to the project matched on the path of a folder or a file (`docs/history/**` also matches the folder), and `ids`, heading ids; a node under a denied one is denied too. `read` closes a node to `show`, `ls`, `search`, `stats`, `export` and every write, since nothing is written blind; `ls` and `show --info` print it as `[denied]` after the `#` of a heading or the path of a folder or a file, with no id, no title, no size and nothing under it, so an agent sees that something is there and does not try it; what holds it counts it as one node with no body. `update` closes a node to `update`, and to `add` or `move` into it; `remove` closes it to `remove` and to `move` out of its place, and so every folder, file or heading above it too. `check`, `lint` and `verify` see every node, and `verify --fix` still writes; `lint` reports an entry that names no node, since it closes nothing. Paths and ids stand in lists of their own, so no text is read as either one.

## Verbs {1f44f8}

The docs as one tree: a folder is its path, a file is its path and its body is the text before its first heading, and every heading has an id and holds the headings under it with more `#`. Every verb takes an id or a path, and `--json`. Every call gives every heading an id before it reads (`autofix:` says what it wrote).

<!-- verify: mdmap help -->

```
usage: mdmap <verb> [args]

The docs as one tree: folders and files by their path, headings by their id.

read
  mdmap ls [<id>|<path>] [--depth N] [--summary] [--body]
  mdmap show <id>|<path>... [--info] [--path] [--outline]
  mdmap search "<text>" [--summary] [--body]
  mdmap stats [--unread] [--cost]       the corpus in numbers, and how often the log opened each heading
  mdmap export                          the whole tree as JSON, for whatever reads it from outside
write
  mdmap add <id>|<path> --title <text> [--body <text>] [--template <name> [--fill <path> <text>]…] [--dir] [--title …]…
  mdmap update <id>|<file> [--title <text>] [--body <text>] [--lines <a>-<b>] [--append <text>] [--old <text> --new <text>]…
  mdmap remove <id>|<path>
  mdmap move <id> <id>|<file>   a heading and all under it, ids kept, to the end of another
corpus
  mdmap check [--fix]
  mdmap lint
  mdmap verify [<id>|<path>] [--fix]

Every verb takes --json. The settings are mdmap.yml at the root of the project.
```

## Contributing {709579}

`main` takes pull requests only: work on a branch and open a pull request. `.github/workflows/test.yml` runs `cargo test --locked` on Linux, macOS and Windows for every pull request and every push to `main`, and its jobs `linux`, `macos` and `windows` must pass before a pull request merges. The verify tests run `ls`, `echo`, `printf` and `sleep` as programs; on Windows they come from `C:\Program Files\Git\usr\bin`, which the workflow adds to the end of PATH, and a Windows machine needs the same to run the tests.

A mac compiles neither Linux nor Windows, so before a commit these pass too (`rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-gnu` once): `cargo clippy --target x86_64-unknown-linux-gnu --all-targets --locked` and `cargo clippy --target x86_64-pc-windows-gnu --all-targets --locked`.
