# Roadmap of mdmap {4be5a2}

What is designed but not built in mdmap. Each item is a section: what it is, why, and its open questions. An item leaves when it is built and the docs describe it, or when it is dropped.

## diff {0e044f}

`mdmap diff <commit>..<commit>`: which files and headings were added, removed or retitled, and whose body changed, by id. Why: a reviewer asks what a change did to the docs, and an agent where the work stood; `git diff` shows lines, this shows headings.

## Links from docs to code {9180f2}

A link from a doc to a source file (`[graph.rs](../src/graph.rs)`) is known, not broken: `show` names the files a heading describes, and a heading whose file changed after it is flagged stale, read from `git log`. Why: `verify` checks what a command prints, not the reasons prose gives; a stale signal covers part of that.

Open: whether a code file is only a link target; the noise, since every code change flags every heading that points at it; how the config names paths outside `roots`.
A first step: `lint` names a heading whose linked file changed in a commit after the last one that changed the heading, such as `a00013 stale: src/graph.rs changed in 0906aee`.
