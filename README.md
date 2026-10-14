# mdmap

Markdown docs as a tree that an AI agent can walk one heading at a time.

## Why

Coding agents find their way through project docs blind. They guess a file
name, grep for a word, open a file to see what is in it, then open another.
Every wrong turn costs context, and a 40 KB design doc costs 40 KB even when
the agent needed one paragraph of it.

mdmap gives the agent a map first. `mdmap ls` prints the headings of every
doc as one tree: folders, files, and the headings inside them, each with a
short id. In one call the agent sees everything the docs cover and where it
lives, so it goes straight to the right place instead of searching for it.
It walks the tree down with `mdmap ls <id>` and opens only the section it
needs with `mdmap show <id>`. To answer a question about login it reads the
2 KB Authentication section, not the whole design doc.

## What it looks like

`mdmap ls` prints the whole map: every doc and every heading in it. This is
the docs folder of this repository:

```
$ mdmap ls
docs/
   README.md  # docs {944eda}
   code-map.md  # Code map {522a4e}
      ## Corpus: parsing, tree, ids, links, settings {256dda}
      ## Reading: ls, show, search, stats, export {5078e7}
      ## Writing: add, update, move, remove {c2b96f}
      ## Checks: check, lint, verify {3d7ee4}
      ## Entry: arguments, refusals, output {8448e6}
      ## Tests: helpers and the sample corpus {e5a6a3}
      ## Dependencies {dba6b7}
   guide.md  # Guide {9e9f62}
      ## Install {743350}
      ## Start a corpus {d159cf}
      ## Settings: mdmap.yml {b64ddd}
      ## Verbs {1f44f8}
   overview.md  # Overview {8905cd}
      ## What mdmap is {10ddcd}
      ## Concepts {2aba67}
      ## One call {bb1876}
      ## Where things live {748d5a}
   roadmap.md  # Roadmap of mdmap {4be5a2}
      ## diff {0e044f}
      ## Links from docs to code {9180f2}
```

When the docs are big, look from higher up first. `--depth 1` stops at the
files and says how much is under each one; `mdmap ls <id>` then opens one
branch:

```
$ mdmap ls --depth 1
docs/
   README.md  # docs {944eda}
   code-map.md  # Code map {522a4e}
      … 7 more
   guide.md  # Guide {9e9f62}
      … 4 more
   overview.md  # Overview {8905cd}
      … 4 more
   roadmap.md  # Roadmap of mdmap {4be5a2}
      … 2 more

$ mdmap ls 9e9f62
## Install {743350}
## Start a corpus {d159cf}
## Settings: mdmap.yml {b64ddd}
## Verbs {1f44f8}
```

Found it. `mdmap show` reads just that section:

```
$ mdmap show 743350
## Install {743350}
7  From a checkout of this repository: `cargo install --path . --locked` puts ...
```

`{743350}` is the id. mdmap writes it at the end of the heading line the
first time it reads the file, and it never changes: rename the heading or
move it to another file and the id stays. So a prompt, a link or a note can
point at a section and still find it next month. On GitHub the id shows up at
the end of the heading; that is the price of keeping the files plain markdown.

## What else it does

- **Edit by id.** `add`, `update`, `move` and `remove` work on headings, so
  an agent changes one section without rewriting the file or guessing which
  string to replace.
- **Search.** `mdmap search "text"` answers with the headings that hold it,
  ids included, ready for `show`.
- **Checks.** `check` finds missing or duplicate ids, `lint` finds broken
  links and paths that no longer exist.
- **Docs that do not go stale.** Put `<!-- verify: <command> -->` above a
  code block and `verify` compares the block with the output of that
  command; `verify --fix` rewrites it. This repository keeps its dependency
  list and its `mdmap help` text that way. Only the commands listed in
  `mdmap.yml` may run.
- **Size warnings.** A section that grows too long comes back with a hint to
  split it.
- **Access rules.** `deny` in `mdmap.yml` closes paths or headings to
  reading, editing or removing.
- **Numbers.** `mdmap stats` gives the size of the docs and names the
  sections that are too long or empty.

It calls no model, runs no server and keeps no index. One binary, and your
docs stay ordinary markdown files.

## Install

macOS and Linux:

```
curl -LsSf https://github.com/kalfadev/mdmap/releases/latest/download/mdmap-installer.sh | sh
```

Windows (PowerShell):

```
powershell -ExecutionPolicy Bypass -c "irm https://github.com/kalfadev/mdmap/releases/latest/download/mdmap-installer.ps1 | iex"
```

Or download the binary for your system from the
[releases page](https://github.com/kalfadev/mdmap/releases). With a Rust
toolchain: `cargo install --git https://github.com/kalfadev/mdmap --locked`.

Then `mdmap help` shows it works.

## Quick start

In the repository that holds your docs, create `mdmap.yml`:

```
roots: [docs/]
```

Run `mdmap ls`. The first call gives every heading an id; commit the ids
with the docs. Then tell your agent about it, for example in `CLAUDE.md` or
`AGENTS.md`:

```
The docs in docs/ are read with mdmap, never opened as files.
Start with `mdmap ls` to see every heading, then `mdmap show <id>` for what you need.
```

## Commands

```
read
  mdmap ls [<id>|<path>] [--depth N] [--summary] [--body]
  mdmap show <id>|<path>... [--info] [--path] [--outline]
  mdmap search "<text>" [--summary] [--body]
  mdmap stats [--unread] [--cost]
  mdmap export
write
  mdmap add <id>|<path> --title <text> [--body <text>] [--template <name> ...]
  mdmap update <id>|<file> [--title <text>] [--body <text>] [--append <text>] ...
  mdmap remove <id>|<path>
  mdmap move <id> <id>|<file>
corpus
  mdmap check [--fix]
  mdmap lint
  mdmap verify [<id>|<path>] [--fix]
```

Every command takes `--json`. `mdmap help` prints the full list, and the
[Guide](docs/guide.md) explains every command and setting.


## Documentation

The `docs/` folder of this repository is an mdmap corpus itself. Start with
[Overview](docs/overview.md); [docs/README.md](docs/README.md) lists the
rest.

## License

MIT, see [LICENSE](LICENSE).
