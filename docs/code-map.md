# Code map {522a4e}

The code in areas. Each area node says what the area owns, its rules, what else changes with it and which tests cover it; its last block lists every item of its files with line spans, printed by `show <id> --outline` (`mdmap verify --fix` keeps it current).

| the task touches | area | source | tests |
|---|---|---|---|
| `ls`, `show`, `search`, `stats`, `export` | `5078e7` | view.rs, stats.rs, export.rs | show.rs, search.rs, stats.rs, export.rs |
| `add`, `update`, `move`, `remove`, templates | `c2b96f` | write.rs, the `*_verb` of cli.rs | write.rs, template.rs |
| `check`, `lint`, `verify`, running a verify command | `3d7ee4` | check.rs, lint.rs, verify.rs, exec.rs | check.rs, links.rs, verify.rs |
| parsing, the tree, ids, links, `ignore`, `deny`, `mdmap.yml` | `256dda` | markdown.rs, graph.rs, config.rs, slug.rs, id.rs | tree.rs, links.rs, template.rs, deny.rs |
| arguments, flags, refusals, the id lock, `Output` | `8448e6` | cli.rs, out.rs, lib.rs, main.rs | every file |
| test helpers, the sample corpus | `e5a6a3` | tests/common/mod.rs | |
| crates | `dba6b7` | Cargo.toml | |

Key types. `Graph` (graph.rs): `project`, `config`, `nodes`, `roots`, `docs` (a `Doc` per file) and `by_id`; a verb takes `&Graph` and node indices. `Node`: `kind` (`Folder`, `File`, `Section`), `level`, `title`, `id`, `file`, `body` (a byte range of the file), `parent`, `children`, `links`, `backlinks`, `body_chars`, `subtree`. `Doc` (markdown.rs): `text`, `headings`, `links`, `paragraphs`, `code`, `verifies`, `open_fence_at_eof`. `Output`, `Refusal` and `R<T>` (out.rs): what a verb returns. `Config` (config.rs): `mdmap.yml`.

## Corpus: parsing, tree, ids, links, settings {256dda}

How files become nodes. `markdown.rs` parses one file (pulldown-cmark) into headings, links, paragraphs, code spans and `verify` comments, all with byte offsets. `graph.rs` walks `roots` minus `ignore` and builds the tree: a folder is its path, with no body and no id, its children its `.md` files, then its subfolders; a file is its path, its body the text before its first heading, its children the headings under no other heading; a heading has an id and belongs to the nearest heading above it with fewer `#`, so levels may skip and a file may hold several `#`. Then sizes, links and backlinks, and what `deny` closes each node to (`Graph::denied`). `config.rs` reads the settings, `mdmap.yml` at the root of the project: `roots`, `ignore`, `templates`, `warn.body`, `verify`, `deny`; a project without the file reads the defaults. `slug.rs` turns a title into a file name; ids come from `id.rs`.

Rules. An id is six lowercase hex characters, random, never derived, never changed; every call gives missing ids before it reads. Folders and files are named by their path, headings by their id; every verb takes either. Nothing inside a code fence or inline code is structure. Folders without markdown are not nodes. `ignore` globs are relative to the root they are under, so `docs/x/` matches nothing where `x/**` works. In the globs of `ignore` and `deny`, `*` stays inside one folder and `**` crosses folders, as in `.gitignore` (`path_glob`). Sizes are characters; `warn.body` counts the body `show` prints, so the block under a `verify` comment, which is a command output and not prose, is left out of it. A link whose fragment is not an id is broken. `deny` paths are globs relative to the project, not to a root, matched on the path of a folder or a file, and `docs/x/**` also matches `docs/x`; a `deny` id that is not an id refuses the load; a node under a denied one is denied the same way, and `read` denies every verb.

When changing. A new node property goes into `Node` (graph.rs), then `view.rs` (show, ls, their JSON) and `export.rs`. A new config field: `Config` in config.rs (unknown fields are refused), `mdmap.yml`, **Guide: Settings**. A new verb that names a node asks `reachable` in cli.rs, and one that lists nodes leaves out what `Graph::denied` closes to read. Tests: tests/tree.rs (files to nodes), tests/links.rs (links, backlinks, lint), tests/deny.rs (`deny` over every verb).

<!-- verify: outline src/markdown.rs src/graph.rs src/config.rs src/slug.rs src/id.rs tests/tree.rs tests/links.rs tests/deny.rs -->

```
src/config.rs
  14-14  pub const MDMAP_PATH: &str = "mdmap.yml";
  16-16  pub const ARGS: &str = "{args}";
  20-35  pub struct Config
  41-51  pub struct DenyConfig
  57-62  pub struct DenyList
  64-69  impl DenyConfig
  66-68    pub fn lists(&self) -> [(&str, &DenyList); 3]
  74-83  pub struct VerifyConfig
  85-87  fn verify_timeout() -> u64
  89-93  impl Default for VerifyConfig
  90-92    fn default() -> Self
  99-106  pub struct Allow
  112-115  enum Entry
  119-125  struct Full
  127-134  impl From<Entry> for Allow
  128-133    fn from(e: Entry) -> Allow
  139-143  pub struct WarnConfig
  145-147  fn default_roots() -> Vec<String>
  149-151  fn warn_body() -> usize
  153-157  impl Default for WarnConfig
  154-156    fn default() -> Self
  159-163  impl Default for Config
  160-162    fn default() -> Self
  166-179  impl Config
  168-178    pub fn root_dirs(&self) -> Vec<String>
  183-185  pub fn path_of(project: &Path) -> Option<String>
  189-198  pub fn find_project(cwd: &Path) -> Option<PathBuf>
  200-215  pub fn load(project: &Path) -> R<Config>
  220-223  pub enum TemplateItem
  225-232  impl TemplateItem
  226-231    fn parts(&self) -> (&str, &[TemplateItem])
  236-248  pub fn template_headings(items: &[TemplateItem]) -> Vec<(u8, String, String)>
  237-244    fn walk(items: &[TemplateItem], depth: u8, prefix: &str, out: &mut Vec<(u8, String, String)>)
  253-278  fn check_template(path: &str, name: &str, items: &[TemplateItem], depth: usize) -> R<()>
src/graph.rs
  23-23  pub const README: &str = "README.md";
  26-30  pub enum Kind
  32-40  impl Kind
  33-39    pub fn name(self) -> &'static str
  44-48  pub enum Act
  51-76  pub struct Node
  79-86  pub enum LinkTarget
  89-95  pub struct ResolvedLink
  98-115  pub struct Graph
  117-686  impl Graph
  118-145    pub fn load(project: &Path, config: Config) -> R<Graph>
  147-152    fn root_of<'a>(&self, rel: &'a str, root: &str) -> &'a str
  154-164    fn ignored(&self, rel: &str, root: &str) -> bool
  168-170    pub fn unused_ignores(&self) -> Vec<&str>
  174-177    pub fn denied(&self, i: usize, act: Act) -> bool
  180-182    pub fn denied_under(&self, i: usize, act: Act) -> bool
  186-201    pub fn unused_denies(&self) -> Vec<String>
  205-226    fn denials(&self) -> R<Vec<[bool; 3]>>
  228-235    fn read_doc(&mut self, rel: &str) -> R<()>
  239-266    fn entries(&self, dir: &str, root: &str) -> R<Vec<(String, bool)>>
  268-289    fn has_markdown(&self, dir: &str, root: &str) -> bool
  291-327    fn build_dir(&mut self, dir: &str, root: &str, parent: Option<usize>) -> R<usize>
  331-356    fn push_file_node(&mut self, file: &str, parent: Option<usize>) -> usize
  358-395    fn build_sections(&mut self, file: &str, file_node: usize)
  397-445    fn finish(&mut self)
  447-468    fn sizes(&mut self, i: usize)
  471-530    pub fn resolve(&self, from_file: &str, dest: &str) -> LinkTarget
  532-534    fn is_dir_node(&self, p: &str) -> bool
  536-539    fn node_is_at(&self, t: usize, p: &str) -> bool
  542-545    pub fn node_at_path(&self, p: &str) -> Option<usize>
  549-554    pub fn find(&self, key: &str) -> Option<usize>
  557-564    pub fn key(&self, i: usize) -> String
  566-571    pub fn in_roots(&self, p: &str) -> bool
  573-575    pub fn node(&self, id: &str) -> Option<usize>
  577-579    pub fn doc(&self, i: usize) -> Option<&Doc>
  582-586    pub fn line(&self, i: usize) -> Option<usize>
  588-593    pub fn body_text(&self, i: usize) -> &str
  595-597    pub fn body_lines(&self, i: usize) -> Option<(usize, usize)>
  603-631    pub fn prose_chars(&self, i: usize) -> usize
  633-635    pub fn summary(&self, i: usize) -> Option<String>
  639-645    pub fn heading(&self, i: usize) -> String
  648-655    pub fn location(&self, i: usize) -> String
  658-668    pub fn all_ids(&self) -> HashSet<String>
  671-680    pub fn descendants(&self, i: usize) -> Vec<usize>
  683-685    pub fn files_of(&self, nodes: &[usize]) -> Vec<String>
  689-707  fn build_ignore(patterns: &[String]) -> R<(GlobSet, Vec<usize>)>
  711-724  fn build_deny(verb: &str, patterns: &[String]) -> R<GlobSet>
  728-730  fn path_glob(p: &str) -> Result<Glob, globset::Error>
  733-739  fn node_path(n: &Node) -> Option<&str>
  741-747  pub fn join(dir: &str, name: &str) -> String
  749-754  pub fn parent_dir(file: &str) -> String
  757-772  pub fn normalize(p: &str) -> Option<String>
  774-790  fn percent_decode(s: &str) -> String
src/id.rs
  6-8  pub fn is_id(s: &str) -> bool
  12-19  pub fn generate(taken: &mut HashSet<String>) -> String
src/markdown.rs
  16-28  pub struct Heading
  31-35  pub struct Link
  38-42  pub enum CodeKind
  45-54  pub struct CodeSpan
  57-62  pub struct VerifyComment
  65-79  pub struct Doc
  81-88  fn options() -> Options
  90-280  impl Doc
  91-239    pub fn parse(text: impl Into<String>) -> Doc
  241-243    pub fn line_of(&self, offset: usize) -> usize
  247-252    pub fn line_start(&self, line: usize) -> usize
  255-259    pub fn body_of(&self, i: usize) -> Range<usize>
  262-267    pub fn lines_of(&self, r: &Range<usize>) -> Option<(usize, usize)>
  270-279    pub fn summary_in(&self, r: &Range<usize>) -> Option<String>
  283-285  fn line_of(starts: &[usize], offset: usize) -> usize
  287-289  fn doc_line_start(starts: &[usize], offset: usize) -> usize
  292-300  fn line_end_after(text: &str, offset: usize) -> usize
  303-311  fn atx_text(line: &str) -> String
  314-325  pub fn split_id(text: &str) -> (String, Option<String>)
  329-347  fn fence_inner(text: &str, r: Range<usize>) -> (Range<usize>, bool)
  350-352  pub fn has_heading(text: &str) -> bool
  355-365  pub fn heading_line(level: u8, title: &str, id: Option<&str>) -> String
src/slug.rs
  3-29  pub fn slug(title: &str) -> String
  32-41  mod tests
  36-40    fn examples()
tests/deny.rs
  4-4  mod common;
  11-15  fn project() -> Project
  18-27  fn a_star_in_a_deny_path_stays_inside_one_folder()
  30-53  fn ls_shows_a_read_denied_node_as_denied_and_nothing_under_it()
  56-78  fn every_verb_refuses_a_read_denied_node_and_what_is_under_it()
  81-101  fn update_and_remove_close_only_their_own_verbs()
  104-121  fn search_stats_and_export_leave_out_what_is_closed_to_read()
  124-134  fn a_deny_id_that_is_not_an_id_is_refused()
  139-143  fn closed_files() -> Project
  146-171  fn ls_prints_a_denied_file_and_a_denied_top_heading_on_the_file_line()
  174-195  fn a_link_to_a_denied_node_shows_as_denied_and_export_drops_every_edge_to_it()
  198-206  fn a_denied_root_leaves_nothing_to_read()
  209-221  fn sizes_and_counts_tell_nothing_of_what_is_closed_to_read()
  224-233  fn lint_reports_a_deny_entry_that_names_nothing()
tests/links.rs
  3-3  mod common;
  9-28  fn rows(out: &str, label: &str) -> Vec<String>
  31-35  fn a_link_to_path_and_id_goes_to_the_id()
  38-42  fn backlinks_list_every_node_that_links_here()
  45-62  fn link_forms()
  65-73  fn relative_paths_are_read_from_the_linking_file()
  76-84  fn a_link_survives_a_moved_file_because_the_id_does()
  87-95  fn lint_reports_broken_links_and_exits_nonzero()
  98-104  fn removing_a_linked_node_is_allowed_and_lint_reports_the_broken_link()
  108-113  fn lint_reports_verify_without_a_span()
  116-124  fn lint_reports_an_ignore_pattern_that_matches_nothing()
  127-132  fn a_clean_corpus_lints_clean()
  135-141  fn lint_reports_an_id_in_code_no_heading_has_and_a_bold_title_that_is_not_the_heading()
  144-152  fn lint_reads_ids_only_in_inline_code_and_titles_as_written()
  155-164  fn lint_names_the_line_after_frontmatter_and_tells_no_title_of_a_denied_heading()
tests/tree.rs
  3-3  mod common;
  8-17  fn a_folder_is_its_path_with_no_body_and_no_id()
  20-29  fn the_readme_is_a_file_like_any_other()
  32-57  fn children_are_files_then_folders()
  60-71  fn every_heading_level_reads_the_same_way()
  74-88  fn a_heading_may_skip_levels_and_a_file_may_hold_several_tops()
  91-104  fn a_file_node_body_is_the_text_before_its_first_heading()
  107-111  fn text_before_the_first_heading_belongs_to_the_file()
  114-133  fn sizes_nodes_and_max_depth()
  136-149  fn nothing_inside_code_is_structure()
  152-157  fn a_verify_comment_outside_code_is_part_of_the_body()
  160-169  fn a_long_summary_is_cut_at_280_characters_with_an_ellipsis()
  172-177  fn the_summary_is_the_first_paragraph_joined_to_one_line()
  180-185  fn directories_without_markdown_are_not_nodes()
  188-198  fn ignored_paths_are_not_in_the_graph()
  201-210  fn a_star_in_ignore_stays_inside_one_folder_and_two_stars_cross_folders()
  213-220  fn outside_the_roots_nothing_exists()
  223-235  fn each_root_is_its_own_tree()
  238-247  fn kind_comes_from_structure()
```

## Reading: ls, show, search, stats, export {5078e7}

The read verbs. `view.rs`: `show` is for reading and prints only the heading as written and the body without blank lines at its ends, each line after its file line number; a fenced block under a `<!-- verify: … -->` comment is the output of that command, so its lines are counted and not printed, and `--outline` prints them; several ids or paths give one block each, split by a blank line. `show --info` prints the heading, the summary, body, subtree, nodes and max_depth, then every neighbour (parent, children, links, backlinks) with its body and subtree size, each row a callable id or path; `--path` adds the file and line. `ls` prints the tree with titles, ids and paths (`--depth`, `--summary`, `--body`); where `--depth` cut the tree a line under the node says how many nodes it hides (`… N more`), a legend at the end says how to open one, and `--json` carries that number as `hidden`; `ls <path>` lists a folder, and a file with one top heading shares its line with that heading, which the tree of `--json` keeps as a node of its own and counts among the hidden. `search "<text>"` prints the heading of every node whose title or body holds the text as whole words, case ignored, in tree order; `--summary` adds the summary, `--body` the numbered body (with both, only the body); no match is one line and exit 0; several words without quotes are refused with the quoted call. `stats.rs`: `stats` opens with the corpus in numbers, its files, folders, headings and characters, and how many nodes are over `warn.body` or have no body at all; then, when the project keeps a call log in `.harno/log/harno/`, how often a call named each heading, read out of the arguments of every call, the opened ones most called first and then the ones no call ever opened, the biggest of them first; `--unread` prints only those, and `--cost` ranks the opened ones by size times calls, the costliest first: the node to split or cut first. A call counts once for each id it names, and the size is the one of today. `export.rs`: the whole tree as one JSON document, for whatever reads the corpus from outside.

Rules. Output is never cut: past `warn.body` characters a node gets advice to split, counted over the body `show` prints, so the block a `verify` comment names is left out of it, and `stats` sizes every heading the same way. A summary is the first paragraph joined into one line, cut at 280 characters with an ellipsis. `--json` carries the same content as the text. An unknown id is refused with the known ones as the next call. `stats` reads the call log of the project, so that part is empty until a logged call names an id. The block `show` counts instead of printing is the fenced one right under a `verify` comment, blank lines apart: a block further down is prose, and a fence that never closes keeps its lines. A node that `deny.read` of `mdmap.yml` closes, and all under it, is refused to `show` and `ls` and left out of `search`, `stats` and `export`; `ls` and the neighbour rows of `show --info` print it as `[denied]` after the `#` of a heading or the path of a folder or a file, never with its id or title, and its `--json` holds only `key` (a folder or a file), `level`, `kind` and `denied`. Its size is never told: its row of `show --info` prints `-` for body and subtree, and what holds it counts it as one node with no body and nothing under it, in `subtree`, `nodes`, `max_depth` and `… N more`.

When changing. Text and `--json` together; tests/show.rs and tests/search.rs assert exact text, so an output change is a test change. `stats` reads the field `args` of every line of `.harno/log/harno/*.jsonl`: a change of that format is a change here. Tests: tests/show.rs, tests/search.rs, tests/stats.rs, tests/export.rs.

<!-- verify: outline src/view.rs src/stats.rs src/export.rs tests/show.rs tests/search.rs tests/stats.rs tests/export.rs -->

```
src/export.rs
  9-11  pub fn key(g: &Graph, i: usize) -> String
  14-16  fn keys(g: &Graph, v: &[usize]) -> Vec<String>
  18-49  pub fn export(g: &Graph) -> Value
src/stats.rs
  17-17  const GATE_DIR: &str = ".harno/log/harno";
  20-22  fn is_id(s: &str) -> bool
  24-29  pub struct StatsArgs
  34-57  fn opened(g: &Graph) -> BTreeMap<String, usize>
  61-67  struct Corpus
  69-91  fn corpus(g: &Graph) -> Corpus
  96-153  pub fn stats(g: &Graph, a: &StatsArgs, json_out: bool) -> R<Output>
src/view.rs
  9-14  pub struct ShowArgs
  18-24  fn body_trimmed(g: &Graph, i: usize) -> Option<((usize, usize), String)>
  31-79  fn body_block(g: &Graph, i: usize, outline: bool) -> String
  81-85  fn insert_body(g: &Graph, i: usize, m: &mut Map<String, Value>)
  88-104  fn wrap(text: &str, width: usize, indent: &str) -> String
  106-128  pub fn node_json(g: &Graph, i: usize) -> Map<String, Value>
  131-136  fn touch(g: &Graph, i: usize, out: &mut Output)
  139-149  fn split_advice(g: &Graph, i: usize, out: &mut Output)
  151-255  pub fn show(g: &Graph, nodes: &[usize], a: &ShowArgs, json_out: bool) -> R<Output>
  258-263  pub struct LsArgs
  268-273  struct Row
  276-282  fn only_heading(g: &Graph, i: usize) -> Option<usize>
  284-300  fn collect(g: &Graph, i: usize, depth: usize, max: usize, rows: &mut Vec<Row>)
  302-323  fn tree_json(g: &Graph, i: usize, depth: usize, max: usize, a: &LsArgs) -> Value
  327-335  fn label(g: &Graph, i: usize) -> String
  339-348  fn masked(g: &Graph, i: usize, name: String) -> String
  350-414  pub fn ls(g: &Graph, node: Option<usize>, a: &LsArgs, json_out: bool) -> R<Output>
  417-420  pub struct SearchArgs
  423-425  fn fold(s: &str) -> Vec<char>
  429-437  fn has_words(hay: &[char], needle: &[char]) -> bool
  440-448  fn walk(g: &Graph, i: usize, out: &mut Vec<usize>)
  452-503  pub fn search(g: &Graph, text: &str, a: &SearchArgs, json_out: bool) -> R<Output>
tests/export.rs
  3-3  mod common;
  8-43  fn export_carries_every_node_with_its_place_and_its_links()
tests/search.rs
  3-3  mod common;
  8-15  fn search_prints_only_the_headings_in_tree_order()
  18-21  fn case_is_ignored()
  24-36  fn the_text_matches_whole_words_only()
  39-55  fn summary_adds_the_summary_and_body_the_body()
  58-61  fn with_body_summary_is_dropped()
  64-67  fn no_match_is_one_line_and_exit_zero()
  70-77  fn search_without_text_is_refused()
  80-89  fn json_carries_the_same_content()
tests/show.rs
  3-3  mod common;
  8-18  fn show_starts_with_the_heading_as_written_and_lists_the_numbers()
  21-31  fn a_verify_block_is_counted_not_printed()
  34-40  fn a_fenced_block_the_verify_comment_does_not_own_is_printed_whole()
  43-48  fn a_verify_fence_that_never_closes_keeps_its_lines()
  51-73  fn show_lists_every_neighbour_with_body_and_subtree()
  76-87  fn every_neighbour_row_is_callable()
  90-101  fn body_prints_only_the_heading_the_lines_and_the_body_verbatim()
  104-112  fn path_adds_the_file_and_heading_line()
  115-125  fn several_ids_in_one_call_each_in_its_own_block()
  128-134  fn ids_are_written_without_braces()
  137-146  fn an_unknown_id_is_refused_with_the_known_ones_as_the_next_call()
  149-154  fn a_refusal_is_two_lines_and_a_nonzero_exit()
  157-169  fn json_carries_the_same_content_with_body_in_its_own_field()
  172-180  fn ls_prints_the_whole_tree_with_titles_and_ids_only()
  183-197  fn ls_depth_limits_the_levels()
  200-228  fn ls_without_an_id_prints_every_root_and_everything_under_it()
  231-238  fn ls_summary_and_body_add_to_each_line()
  241-255  fn ls_json_keeps_the_heading_the_text_line_folds_into_a_file()
  258-264  fn ls_json_is_a_nested_tree()
  267-271  fn a_node_without_body_shows_no_lines()
tests/stats.rs
  3-3  mod common;
  9-14  fn with_log(p: &Project) -> &Project
  17-27  fn stats_count_how_often_the_log_opened_each_heading()
  30-37  fn unread_prints_only_what_no_call_opened()
  40-59  fn cost_ranks_the_opened_headings_by_size_times_calls()
  62-72  fn stats_count_the_corpus_itself()
  75-79  fn without_a_log_every_heading_is_unopened()
  82-89  fn json_carries_the_same_numbers()
```

## Writing: add, update, move, remove {c2b96f}

The only ways the agent writes the docs (`write.rs`). A write changes only the bytes it owns. Before anything reaches disk the result is parsed in memory and compared with what the verb was meant to do (`sigs`, `expect`); a mismatch, typically an unclosed fence swallowing the headings after it, refuses the whole write. Files go through a temp file and a rename (`atomic_write`).

Rules. `add` under a folder: a new file named by the slug of the title, its heading a `#`; with `--dir`: a new folder with a `README.md`; under a file: a `#` at its end; under a heading: a heading one level down at the end of what it holds. Several `--title`/`--body` pairs in one call, in order, all or nothing. `--template <name>` after a `--title` writes the config template under that new node; without `--title` the template goes under the node itself (a file or a heading, not a folder); its first level is one below the node it goes under, and `--fill <path> <text>` gives one heading its body, the path being titles joined by `/`. An existing name is refused and named; level 6 has no child, the refusal offers the parent, and a template that would pass level 6 is refused; a file with a check problem is refused. `update`: `--title` keeps level and id, `--body` replaces the body, `--lines a-b` replaces only those lines and must stay inside the body, `--append` adds to its end; `--old <text> --new <text>`, once or more, replaces each text found exactly once in the body, every pair against the body as it was, all or nothing: found no time, twice or overlapping another, it is refused with the count, and it takes the place of `--body`, `--lines` and `--append`; a folder has no text to update. `move <id> <id>|<file>`: a heading and every heading under it go to the end of a file, as a `#`, or of another heading, one level below it, every level shifted by the same amount and every id kept, so links follow; a folder or a file to move, a target inside the moved heading, a file with a check problem and a result past level 6 are refused. `remove`: the node and its subtree; the last section of a file takes the blank line above it too, so an add and a remove of the same node leave the file as it was. A body may not hold a heading or leave a fence open. Ids never change on a write. Every refusal names the next command (`instead`). What `deny` of `mdmap.yml` closes is refused before anything is written: `update` and `add` ask `deny.update` of the node they name, `remove` asks `deny.remove` of every node of its subtree, `move` both, and `deny.read` closes a node to all of them (`reachable` in src/cli.rs).

When changing. The CLI side is `add_verb`, `update_verb`, `move_verb` and `remove_verb` in src/cli.rs, the work `add`, `update`, `move_node` and `remove` here; templates are `TemplateItem` and `template_headings` in src/config.rs. Tests: tests/write.rs (add, update, move, remove), tests/template.rs.

<!-- verify: outline src/write.rs tests/write.rs tests/template.rs -->

```
src/write.rs
  24-30  struct Sig
  32-44  fn sigs(doc: &Doc) -> Vec<Sig>
  47-73  fn expect(file: &str, text: &str, expected: &[Sig]) -> R<Doc>
  75-83  fn validate_title(t: &str) -> R<()>
  85-92  fn validate_body(b: &str) -> R<()>
  96-108  fn body_block(body: &str, followed: bool) -> String
  111-125  fn separated(before: &str, block: &str, followed: bool) -> String
  128-132  pub enum AddMode
  134-140  pub struct Child
  143-146  pub struct Applied
  149-153  struct New
  155-160  enum Plan
  163-198  fn expand(g: &Graph, children: &[Child]) -> R<Vec<New>>
  203-375  pub fn add(g: &Graph, p: usize, children: &[Child], mode: AddMode, json: bool) -> R<Output>
  377-398  fn add_command(parent: &str, children: &[Child], flag: Option<&str>) -> String
  400-402  pub fn no_such(id: &str) -> Refusal
  404-409  pub struct UpdateArgs
  413-553  pub fn update(g: &Graph, i: usize, a: &UpdateArgs, json: bool) -> R<Output>
  557-564  fn ends_clean(text: &str) -> String
  568-571  fn subtree_end(doc: &Doc, h: usize) -> usize
  575-666  pub fn move_node(g: &Graph, i: usize, p: usize, json: bool) -> R<Output>
  670-720  pub fn remove(g: &Graph, i: usize, json: bool) -> R<Output>
tests/template.rs
  4-4  mod common;
  8-16  const CONFIG: &str = "roots: [docs/]
  18-22  fn project() -> Project
  25-43  fn a_template_under_a_directory_makes_a_file_with_every_heading()
  46-62  fn a_template_under_a_section_starts_two_levels_down()
  65-80  fn a_template_without_a_title_goes_under_the_node_itself()
  83-95  fn a_wrong_template_or_path_is_refused_and_nothing_is_written()
  98-110  fn a_template_too_deep_for_its_place_is_refused()
  113-121  fn the_config_refuses_a_template_mdmap_cannot_write()
tests/write.rs
  3-3  mod common;
  7-9  fn is_id(s: &str) -> bool
  12-20  fn add_to_a_file_appends_a_level_two_heading_and_touches_nothing_else()
  23-32  fn add_to_a_file_path_appends_a_top_heading()
  35-47  fn add_to_a_section_goes_to_the_end_of_its_subtree_one_level_down()
  50-60  fn add_to_a_folder_creates_a_file_named_by_the_slug()
  63-71  fn add_dir_creates_a_folder_whose_readme_holds_the_heading()
  74-80  fn dir_needs_a_folder()
  83-89  fn an_existing_name_is_refused_and_the_refusal_names_it()
  92-104  fn level_six_has_no_child_and_the_refusal_offers_a_sibling()
  107-120  fn several_children_in_one_call_in_the_given_order()
  123-131  fn if_one_child_is_refused_none_is_written()
  134-143  fn a_body_with_a_heading_is_refused()
  146-155  fn a_body_that_leaves_a_fence_open_is_refused()
  158-163  fn a_fenced_heading_in_a_body_is_fine()
  166-171  fn update_title_keeps_level_and_id()
  174-179  fn update_body_replaces_the_whole_body()
  182-191  fn update_lines_replaces_only_those_lines()
  194-201  fn update_lines_outside_the_body_is_refused()
  204-214  fn update_old_new_replaces_each_text_found_once_in_the_body()
  217-230  fn update_old_found_no_time_twice_or_overlapping_is_refused_and_writes_nothing()
  233-239  fn update_old_counts_every_place_it_starts_overlapping_ones_too()
  242-264  fn update_old_new_keeps_the_rules_of_every_write()
  267-275  fn ids_do_not_change_when_writing()
  278-287  fn remove_a_section_removes_its_byte_range_and_subtree()
  290-297  fn an_add_and_a_remove_of_the_same_node_leave_the_file_as_it_was()
  300-311  fn remove_a_file_or_a_folder_by_its_path()
  314-317  fn two_files(p: &common::Project)
  320-327  fn move_takes_a_heading_and_all_under_it_to_another_file_ids_kept()
  330-338  fn move_under_a_heading_shifts_every_level()
  341-351  fn move_refuses_what_it_cannot_do_and_writes_nothing()
  354-359  fn a_root_cannot_be_removed()
  362-371  fn writes_leave_no_temp_files_behind()
  374-387  fn new_ids_are_unique_six_hex_characters()
  390-395  fn add_refuses_a_file_with_broken_structure()
  398-403  fn update_append_puts_the_text_on_the_lines_after_the_body()
  406-415  fn update_a_file_changes_the_text_above_its_first_heading()
```

## Checks: check, lint, verify {3d7ee4}

Three ways to ask whether the docs are right. `check.rs`: can the tree be read from these files: a setext heading (an underline, not `#`), a missing or a duplicate id; any heading levels are fine. `--fix` touches structure only. Every call runs the id part itself before it reads, under a lock on the config file (`load_with_ids` in src/cli.rs), and a call that read a file with a check problem starts with a warning. `lint.rs`: broken links, stale paths, a verify comment with nothing to compare, an id in inline code (`{3d7ee4}`) that no heading has, a bold title right before such an id (`**Title**`, or `**File: Title**` led by the titles above it) that is not the title of its heading, and an `ignore` pattern that no path under the roots matches, since a pattern written from the project instead of the root hides nothing and lets its files into the corpus; the walk of a load marks each pattern a path matched (`unused_ignores`). A `deny` entry that names no node, an id no heading has or a path no folder or file matches, is reported the same way, since it closes nothing (`unused_denies`, tested in tests/deny.rs). It reports, never fixes. An example id in prose is the id of the heading that holds it, so it names a heading that is there. `verify.rs`: `<!-- verify: <command> -->` and the first code span after it in the same body; the command output is compared with the span; `--fix` rewrites only the span; `verify <id>|<path>` checks one node. `exec.rs` runs the command: `split` into words, `allowed` against `verify.run`, `with_args`, `argv` with its globs expanded, `exec` under a timeout.

Rules. `verify` runs only what `verify.run` of the settings allows: a line must start with the words of an entry, and what runs is the command of that entry with the other words where `{args}` is, or the line itself when the entry names no command. There is no shell, each command runs under the timeout of its entry, else `verify.timeout`; a line outside the list is refused in the report; a comment without a span does not run. Only stdout is compared: stderr is dropped, and a nonzero exit fails the node. `check --fix` never touches prose. The id pass of every call writes the id of a setext heading at the end of its text and leaves the underline: turning it into a `#` line is structure, which `check --fix` alone does.

When changing. A new problem kind: `ProblemKind`, its `message`, and whether it is an `id_problem` (fixed on every call) or `structural`. The text report of `verify` is a line per node, `<id>  <title>    <status>` with the titles padded to one width and the status `ok`, `fixed`, `FAIL`, `refused` or `timeout`; the detail under it is indented two spaces, a diff line `  + …` or `  - …`; the summary comes last. Programs that filter the report read that shape. Tests: tests/check.rs, tests/verify.rs; lint is in tests/links.rs.

<!-- verify: outline src/check.rs src/lint.rs src/verify.rs src/exec.rs tests/check.rs tests/verify.rs -->

```
src/check.rs
  19-23  pub enum ProblemKind
  26-32  pub struct Problem
  34-55  impl Problem
  35-43    pub fn message(&self) -> String
  46-48    pub fn id_problem(&self) -> bool
  52-54    pub fn structural(&self) -> bool
  58-70  pub fn local_problems(file: &str, doc: &Doc) -> Vec<Problem>
  73-81  pub fn problems(g: &Graph) -> Vec<Problem>
  83-85  fn line_num(at: &str) -> usize
  87-107  fn duplicates<'a>(docs: impl Iterator<Item = (&'a str, &'a Doc)>) -> Vec<Problem>
  109-116  pub fn render(problems: &[Problem]) -> String
  119-138  pub fn check(g: &Graph, json: bool) -> Output
  141-144  pub struct Fixed
  148-154  pub enum Scope
  157-207  pub fn apply_fixes(g: &Graph, scope: Scope) -> R<(Vec<Fixed>, Vec<String>)>
  209-240  pub fn fix(g: &Graph, json: bool) -> R<Output>
  243-250  fn pick(problems: &[Problem], scope: Scope) -> Option<&Problem>
  252-260  fn line_ending(line: &str) -> &'static str
  262-291  fn apply(doc: &Doc, p: &Problem, taken: &mut HashSet<String>) -> String
src/exec.rs
  20-25  pub struct Word
  27-87  pub fn split(s: &str) -> R<Vec<Word>>
  91-121  pub fn argv(words: &[Word], dir: &Path) -> Vec<String>
  126-139  pub fn allowed<'a>(allow: &'a [Allow], words: &[Word]) -> Option<&'a Allow>
  143-151  pub fn refusal_name(allow: &[Allow], words: &[Word], raw: &str) -> String
  155-236  pub fn exec(command: &str, dir: &Path, timeout: Duration, stderr: bool) -> (i32, String, String, f64)
  238-240  fn secs(started: Instant) -> f64
  244-250  pub fn with_args(command: &str, args: &[String]) -> String
  253-255  fn single_quote(s: &str) -> String
src/lint.rs
  13-16  pub struct Finding
  18-73  pub fn findings(g: &Graph) -> Vec<Finding>
  77-80  fn bold_before(before: &str) -> Option<&str>
  82-101  pub fn lint(g: &Graph, json_out: bool) -> Output
src/verify.rs
  23-26  pub fn span_after(doc: &Doc, end: usize) -> Option<&CodeSpan>
  28-34  fn normalize(s: &str, inline: bool) -> String
  37-42  struct Check
  45-71  fn replacement(doc: &Doc, span: &CodeSpan, output: &str) -> Option<(std::ops::Range<usize>, String)>
  73-221  pub fn verify(g: &Graph, node: Option<usize>, fix: bool, json_out: bool) -> R<Output>
tests/check.rs
  5-5  mod common;
  10-28  fn snapshot(p: &Project) -> BTreeMap<String, String>
  11-24    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut BTreeMap<String, String>)
  31-36  fn a_clean_corpus_has_no_problems()
  39-50  fn check_reports_structure_problems_and_writes_nothing_for_them()
  53-74  fn every_call_gives_every_node_an_id_before_it_reads()
  77-85  fn autofix_writes_ids_only_and_leaves_structure_to_check_fix()
  88-102  fn the_id_pass_of_a_read_leaves_a_setext_heading_setext()
  105-138  fn fix_closes_every_problem_and_a_second_run_writes_nothing()
  141-154  fn fix_does_not_touch_bodies_or_other_files()
  157-166  fn fix_reports_what_it_fixed()
  169-175  fn a_missing_root_is_an_empty_folder()
  178-186  fn a_call_that_reads_a_broken_file_starts_with_a_warning()
  189-195  fn check_json()
  198-215  fn parallel_calls_agree_on_the_ids_they_write()
tests/verify.rs
  4-4  mod common;
  8-14  fn project() -> Project
  16-16  const FILES: &str = "# Files {f00001}\n\nThe source files.\n\n<!-- verify: ls src/*.rs -->\n\n```\nsrc/graph.rs\nsrc/id.rs\n```\n\nProse after the span is not compared.\n";
  19-26  fn a_matching_fence_passes()
  29-37  fn a_stale_fence_fails_with_a_diff_and_nonzero_exit()
  40-48  fn fix_rewrites_only_the_span()
  51-63  fn an_inline_span_holds_a_single_value()
  66-73  fn a_command_outside_the_allowlist_is_refused_in_the_report()
  76-80  fn no_shell_interpretation()
  83-88  fn an_allowed_command_cannot_chain_another()
  91-97  fn verify_without_a_span_does_not_run()
  100-107  fn verify_of_an_id_checks_only_that_node()
  110-117  fn a_timeout_fails()
  120-126  fn verify_json()
  129-136  fn an_entry_that_names_a_command_runs_it_with_the_other_words_as_arguments()
```

## Entry: arguments, refusals, output {8448e6}

How a call runs (`src/cli.rs`, `main`): `help` needs no project; else the project is the nearest directory upward that holds `mdmap.yml`, or a `.git` (there the defaults apply), and `config::load` reads the settings. Every verb loads the tree with ids first (`load_with_ids`, under a lock on that file) and goes to `graph_verb`; `resolve` takes an id, `{id}`, or the path of a folder or a file. `out.rs`: `Output` (the text, the exit code, the `advice` lines of the call, the files it `touched`, whose check problems give a warning), `Refusal` (`reason`, `instead`) and `R<T>`, and the helpers `atomic_write`, `quote`, `dq` and `truncate`. `lib.rs` is the crate and `main.rs` the binary. `Env` is the directory of the call, taken explicitly so a test drives a call in-process.

Rules. Every refusal names the next call as `mdmap …` on its `instead:` line. Every verb takes `--json`. The verbs, flags and outputs are a contract with the programs that call mdmap.

When changing. A new verb: `USAGE`, `VERBS`, its case in `graph_verb`, a `<verb>_verb` that parses with `flags(verb, rest, allowed, max_pos)` or by hand like `ls_verb`; the work in its area module; tests in `tests/<verb>.rs`, added to the verify line of its area; **Guide: Verbs** follows at `refresh`.

<!-- verify: outline src/cli.rs src/lib.rs src/main.rs src/out.rs -->

```
src/cli.rs
  24-45  pub const USAGE: &str = "usage: mdmap <verb> [args]
  47-47  const VERBS: &[&str] = &["ls", "show", "search", "stats", "add", "update", "remove", "move", "check", "lint", "verify", "export"];
  51-53  pub struct Env
  55-59  impl Env
  56-58    pub fn from_process() -> Env
  61-130  pub fn main(args: Vec<String>, env: Env, out: &mut dyn Write) -> i32
  133-136  fn problem_warning(g: &Graph, touched: &std::collections::BTreeSet<String>) -> Option<String>
  141-168  fn load_with_ids(project: &Path, config: Config) -> R<(Graph, Option<String>)>
  170-208  fn graph_verb(g: &Graph, verb: &str, rest: &[String], json: bool) -> R<Output>
  210-213  struct Flags
  215-219  impl Flags
  216-218    fn has(&self, f: &str) -> bool
  222-238  fn flags(verb: &str, rest: &[String], allowed: &[&str], max_pos: usize) -> R<Flags>
  240-242  fn unknown_option(verb: &str, a: &str) -> Refusal
  246-260  fn resolve(g: &Graph, arg: &str, verb: &str, rest: &[String]) -> R<usize>
  262-264  fn value<'a>(rest: &'a [String], i: usize, flag: &str) -> R<&'a String>
  266-285  fn show_verb(g: &Graph, rest: &[String], json: bool) -> R<Output>
  289-296  fn reachable(g: &Graph, i: usize, act: Act, under: bool, verb: &str) -> R<usize>
  298-327  fn ls_verb(g: &Graph, rest: &[String], json: bool) -> R<Output>
  329-337  fn search_verb(g: &Graph, rest: &[String], json: bool) -> R<Output>
  339-400  fn add_verb(g: &Graph, rest: &[String], json: bool) -> R<Output>
  404-417  fn append_to_body(g: &Graph, rest: &[String]) -> R<Vec<String>>
  419-462  fn update_verb(g: &Graph, rest: &[String], json: bool) -> R<Output>
  464-472  fn move_verb(g: &Graph, rest: &[String], json: bool) -> R<Output>
  474-479  fn remove_verb(g: &Graph, rest: &[String], json: bool) -> R<Output>
src/lib.rs
  11-11  pub mod check;
  12-12  pub mod cli;
  13-13  pub mod config;
  14-14  pub mod exec;
  15-15  pub mod export;
  16-16  pub mod graph;
  17-17  pub mod id;
  18-18  pub mod lint;
  19-19  pub mod markdown;
  20-20  pub mod out;
  21-21  pub mod slug;
  22-22  pub mod stats;
  23-23  pub mod verify;
  24-24  pub mod view;
  25-25  pub mod write;
src/main.rs
  1-4  fn main()
src/out.rs
  10-13  pub struct Refusal
  15-41  impl Refusal
  16-18    pub fn new(reason: impl Into<String>) -> Self
  20-23    pub fn instead(mut self, command: impl Into<String>) -> Self
  25-31    pub fn render(&self) -> String
  33-40    pub fn to_json(&self) -> serde_json::Value
  43-47  impl fmt::Display for Refusal
  44-46    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
  49-53  impl From<std::io::Error> for Refusal
  50-52    fn from(e: std::io::Error) -> Self
  55-55  pub type R<T> = Result<T, Refusal>;
  59-66  pub struct Output
  68-72  impl Output
  69-71    pub fn text(text: impl Into<String>) -> Self
  76-91  pub fn atomic_write(path: &std::path::Path, text: &str) -> R<()>
  94-102  pub fn truncate(s: &str, max: usize) -> String
  105-112  pub fn dq(s: &str) -> String
  115-130  pub fn quote(s: &str) -> String
```

## Tests: helpers and the sample corpus {e5a6a3}

The tests (`tests/`) drive `mdmap::main` in-process against a temporary project, one file per verb or area, and assert the exact output text. `common/mod.rs` holds `Project`: `new()` (an empty `docs/`), `sample()` (the fixed-id corpus `SAMPLE_README`, `SAMPLE_ARCH`, `SAMPLE_DESIGN`), `bare()`, and on a project `write(rel, text)`, `read`, `exists`, `root`, `path`, `id_of(rel, title)`, `ids_in(rel)`; the `Md` trait gives `p.md(&[args])`, `md_in(cwd, …)` and `md_config(yaml)`, which writes `mdmap.yml`. A `Call` has `code` and `out`; `ok()` asserts success and returns the text, `refused()` asserts a refusal and returns it, `instead()`, `json()`; `field(out, name)` reads one field of a text output.

When changing. A behaviour change is a test change first: find the test by name in the area node. Exact asserts need the sample text: read `Project::sample`, `SAMPLE_README`, `SAMPLE_ARCH` and `SAMPLE_DESIGN` in tests/common/mod.rs first. A whole-output assert is an inline insta snapshot, `insta::assert_snapshot!(out, @"…")`: after an output change the failing `test` shows the diff, and `cargo insta accept` writes the new text into the test (insta ignores the trailing newline). Cargo finds a new file in `tests/` on its own; the area it tests lists it in its `verify` line. A test filter matches test names, not files: run the whole suite.

<!-- verify: outline tests/common/mod.rs -->

```
tests/common/mod.rs
  10-12  pub struct Project
  15-18  pub struct Call
  20-43  impl Call
  23-26    pub fn ok(&self) -> &str
  30-34    pub fn refused(&self) -> &str
  36-38    pub fn instead(&self) -> Option<String>
  40-42    pub fn json(&self) -> serde_json::Value
  45-49  impl Default for Project
  46-48    fn default() -> Self
  51-157  impl Project
  54-59    pub fn new() -> Project
  79-92    pub fn sample() -> Project
  95-97    pub fn bare() -> Project
  99-101    pub fn root(&self) -> &Path
  103-105    pub fn path(&self, rel: &str) -> PathBuf
  109-114    pub fn write(&self, rel: &str, text: &str) -> &Self
  117-119    pub fn read(&self, rel: &str) -> String
  121-123    pub fn exists(&self, rel: &str) -> bool
  129-141    pub fn id_of(&self, rel: &str, title: &str) -> String
  144-154    pub fn ids_in(&self, rel: &str) -> Vec<String>
  159-166  pub const SAMPLE_README: &str = "# Docs {a00001}
  168-186  pub const SAMPLE_ARCH: &str = "---
  188-195  pub const SAMPLE_DESIGN: &str = "# Design {a00020}
  199-209  pub fn field<T: std::str::FromStr>(out: &str, name: &str) -> T
  212-217  pub trait Md
  213-213    fn md(&self, args: &[&str]) -> Call;
  214-214    fn md_in(&self, cwd: &Path, args: &[&str]) -> Call;
  216-216    fn md_config(&self, yaml: &str) -> &Self;
  219-234  impl Md for Project
  220-222    fn md(&self, args: &[&str]) -> Call
  224-229    fn md_in(&self, cwd: &Path, args: &[&str]) -> Call
  231-233    fn md_config(&self, yaml: &str) -> &Self
```

## Dependencies {dba6b7}

The crates mdmap uses, one level deep; `mdmap verify --fix` keeps the block current. `pulldown-cmark` parses markdown (markdown.rs), `globset` matches `ignore` (graph.rs), `glob` expands the paths of a verify command (exec.rs), `similar` makes the diff of `verify` (verify.rs), `fastrand` draws the ids (id.rs). `mdmap.yml` is read with `serde-saphyr`, chosen over `serde_norway` when `serde_yaml` was deprecated; its errors carry a snippet of the file below the message, and a refusal keeps the first line (`config::load`).

<!-- verify: deps -->

```
mdmap v0.1.0
├── fastrand v2.5.0
├── glob v0.3.4
├── globset v0.4.20
├── pulldown-cmark v0.13.4
├── serde v1.0.229
├── serde-saphyr v1.3.0
├── serde_json v1.0.151
└── similar v2.7.0
```
