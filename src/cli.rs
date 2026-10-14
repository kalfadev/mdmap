//! Argument parsing and dispatch for `mdmap <verb>`.
//!
//! Every call first gives every heading an id, then reads the tree. A node
//! is named by the id of its heading, or, for a folder or a file, by its
//! path.

use std::io::Write;
use std::path::Path;

use crate::id;
use crate::out::{quote, Output, Refusal, R};


use crate::check;
use crate::config::{self, Config};
use crate::export;
use crate::graph::{Act, Graph};
use crate::lint;
use crate::stats;
use crate::verify;
use crate::view::{self, LsArgs, ShowArgs};
use crate::write::{self, no_such, AddMode, Applied, Child, UpdateArgs};

pub const USAGE: &str = "usage: mdmap <verb> [args]

The docs as one tree: folders and files by their path, headings by their id.

read
  mdmap ls [<id>|<path>] [--depth N] [--summary] [--body]
  mdmap show <id>|<path>... [--info] [--path] [--outline]
  mdmap search \"<text>\" [--summary] [--body]
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
";

const VERBS: &[&str] = &["ls", "show", "search", "stats", "add", "update", "remove", "move", "check", "lint", "verify", "export"];

/// Where a call runs: the directory mdmap reads the project from. Taken
/// explicitly so the tests can drive a call in-process.
pub struct Env {
    pub cwd: std::path::PathBuf,
}

impl Env {
    pub fn from_process() -> Env {
        Env { cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")) }
    }
}

pub fn main(args: Vec<String>, env: Env, out: &mut dyn Write) -> i32 {
    let Some(verb) = args.first().cloned() else {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    };
    let mut rest: Vec<String> = args[1..].to_vec();
    let json = if let Some(p) = rest.iter().position(|a| a == "--json") {
        rest.remove(p);
        true
    } else {
        false
    };
    let emit = |out: &mut dyn Write, text: &str| {
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
    };
    let render = |r: &Refusal| {
        if json {
            serde_json::to_string_pretty(&r.to_json()).unwrap() + "\n"
        } else {
            r.render()
        }
    };

    if matches!(verb.as_str(), "help" | "--help" | "-h") {
        emit(out, USAGE);
        return 0;
    }
    let Some(project) = config::find_project(&env.cwd) else {
        emit(out, &render(&Refusal::new(format!("no {} here or in any parent directory", config::MDMAP_PATH)).instead("write mdmap.yml with roots: [docs/]")));
        return 1;
    };
    let config = match config::load(&project) {
        Ok(c) => c,
        Err(r) => {
            emit(out, &render(&r));
            return 1;
        }
    };
    if !VERBS.contains(&verb.as_str()) {
        emit(out, &render(&Refusal::new(format!("unknown verb '{verb}'")).instead("mdmap help")));
        return 1;
    }

    let (result, notes) = match load_with_ids(&project, config) {
        Ok((g, autofixed)) => {
            let result = graph_verb(&g, &verb, &rest, json);
            let mut notes = autofixed.unwrap_or_default();
            if let Ok(o) = &result {
                if verb != "check" {
                    notes.push_str(&problem_warning(&g, &o.touched).unwrap_or_default());
                }
            }
            (result, notes)
        }
        Err(r) => (Err(r), String::new()),
    };
    let output = match result {
        Ok(o) => o,
        Err(r) => Output { text: render(&r), code: 1, ..Default::default() },
    };
    let mut text = notes;
    for a in &output.advice {
        text.push_str(a);
        text.push('\n');
    }
    text.push_str(&output.text);
    emit(out, &text);
    output.code
}

/// The warning line for check problems in the files a call read.
fn problem_warning(g: &Graph, touched: &std::collections::BTreeSet<String>) -> Option<String> {
    let n = check::problems(g).iter().filter(|p| touched.contains(&p.file)).count();
    (n > 0).then(|| format!("warning: {n} check problem{} — mdmap check\n", if n == 1 { "" } else { "s" }))
}

/// Load the tree, first giving every heading an id. Two calls running at
/// once must not give one heading two different ids, so the pass runs under
/// a lock on the config file and re-reads the corpus inside it.
fn load_with_ids(project: &Path, config: Config) -> R<(Graph, Option<String>)> {
    let g = Graph::load(project, config.clone())?;
    if !check::problems(&g).iter().any(|p| p.id_problem()) {
        return Ok((g, None));
    }
    // The lock is the settings file; a project running on the defaults has
    // none, and nothing there can race either.
    let lock = config::path_of(project).map(|p| std::fs::File::open(project.join(p))).transpose()?;
    if let Some(l) = &lock {
        l.lock()?;
    }
    let g = Graph::load(project, config.clone())?;
    let (fixed, written) = check::apply_fixes(&g, check::Scope::Ids)?;
    let g = Graph::load(project, config)?;
    if let Some(l) = &lock {
        l.unlock()?;
    }
    if fixed.is_empty() {
        return Ok((g, None));
    }
    let note = format!(
        "autofix: {} heading{} got an id — {}\n",
        fixed.len(),
        if fixed.len() == 1 { "" } else { "s" },
        written.join(", ")
    );
    Ok((g, Some(note)))
}

fn graph_verb(g: &Graph, verb: &str, rest: &[String], json: bool) -> R<Output> {
    match verb {
        "show" => show_verb(g, rest, json),
        "ls" => ls_verb(g, rest, json),
        "search" => search_verb(g, rest, json),
        "stats" => {
            let f = flags(verb, rest, &["--unread", "--cost"], 0)?;
            stats::stats(g, &stats::StatsArgs { unread: f.has("--unread"), cost: f.has("--cost") }, json)
        }
        "add" => add_verb(g, rest, json),
        "update" => update_verb(g, rest, json),
        "remove" => remove_verb(g, rest, json),
        "move" => move_verb(g, rest, json),
        "check" => {
            if flags(verb, rest, &["--fix"], 0)?.has("--fix") {
                check::fix(g, json)
            } else {
                Ok(check::check(g, json))
            }
        }
        "lint" => {
            flags(verb, rest, &[], 0)?;
            Ok(lint::lint(g, json))
        }
        "verify" => {
            let f = flags(verb, rest, &["--fix"], 1)?;
            let node = match f.positional.first() {
                Some(a) => Some(resolve(g, a, verb, rest)?),
                None => None,
            };
            verify::verify(g, node, f.has("--fix"), json)
        }
        "export" => {
            flags(verb, rest, &[], 0)?;
            Ok(Output::text(serde_json::to_string_pretty(&export::export(g)).unwrap() + "\n"))
        }
        _ => Err(Refusal::new(format!("unknown verb '{verb}'")).instead("mdmap help")),
    }
}

struct Flags {
    set: Vec<String>,
    positional: Vec<String>,
}

impl Flags {
    fn has(&self, f: &str) -> bool {
        self.set.iter().any(|s| s == f)
    }
}

/// Boolean flags and up to `max_pos` positional arguments.
fn flags(verb: &str, rest: &[String], allowed: &[&str], max_pos: usize) -> R<Flags> {
    let mut f = Flags { set: Vec::new(), positional: Vec::new() };
    for a in rest {
        if a.starts_with("--") {
            if !allowed.contains(&a.as_str()) {
                return Err(unknown_option(verb, a));
            }
            f.set.push(a.clone());
        } else {
            f.positional.push(a.clone());
        }
    }
    if f.positional.len() > max_pos {
        return Err(Refusal::new(format!("too many arguments for {verb}")).instead("mdmap help"));
    }
    Ok(f)
}

fn unknown_option(verb: &str, a: &str) -> Refusal {
    Refusal::new(format!("unknown option {a} for {verb}")).instead("mdmap help")
}

/// The node an argument names: a heading by its id, a folder or a file by
/// its path.
fn resolve(g: &Graph, arg: &str, verb: &str, rest: &[String]) -> R<usize> {
    if let Some(i) = g.find(arg) {
        return Ok(i);
    }
    if id::is_id(arg) {
        return Err(no_such(arg));
    }
    let stripped = arg.trim().trim_start_matches('{').trim_end_matches('}');
    if arg.trim().starts_with('{') && id::is_id(stripped) {
        let args: Vec<String> = rest.iter().map(|a| if a == arg { stripped.to_string() } else { quote(a) }).collect();
        return Err(Refusal::new("ids are written without braces on the command line")
            .instead(format!("mdmap {verb} {}", args.join(" "))));
    }
    Err(Refusal::new(format!("nothing at '{arg}': a node is the id of a heading, or the path of a folder or a file")).instead("mdmap ls"))
}

fn value<'a>(rest: &'a [String], i: usize, flag: &str) -> R<&'a String> {
    rest.get(i + 1).ok_or_else(|| Refusal::new(format!("{flag} needs a value")))
}

fn show_verb(g: &Graph, rest: &[String], json: bool) -> R<Output> {
    let f = flags("show", rest, &["--info", "--path", "--outline"], usize::MAX)?;
    if f.positional.is_empty() {
        return Err(Refusal::new("show needs an id or a path").instead("mdmap ls"));
    }
    let mut nodes = Vec::new();
    for a in &f.positional {
        match resolve(g, a, "show", rest) {
            Ok(i) => nodes.push(reachable(g, i, Act::Read, false, "show")?),
            Err(mut r) => {
                let known: Vec<String> = f.positional.iter().filter(|x| g.find(x).is_some()).map(|x| quote(x)).collect();
                if !known.is_empty() {
                    r = r.instead(format!("mdmap show {}", known.join(" ")));
                }
                return Err(r);
            }
        }
    }
    view::show(g, &nodes, &ShowArgs { body: !f.has("--info"), path: f.has("--path"), outline: f.has("--outline") }, json)
}

/// The node, unless `deny` of `mdmap.yml` closes it to `act`; with `under`,
/// unless it closes any node of its subtree.
fn reachable(g: &Graph, i: usize, act: Act, under: bool, verb: &str) -> R<usize> {
    let closed = if under { g.denied_under(i, act) } else { g.denied(i, act) };
    if !closed {
        return Ok(i);
    }
    let what = if under { " or a node under it" } else { "" };
    Err(Refusal::new(format!("deny of mdmap.yml closes {}{what} to {verb}", g.key(i))).instead("mdmap ls"))
}

fn ls_verb(g: &Graph, rest: &[String], json: bool) -> R<Output> {
    let mut a = LsArgs { depth: None, summary: false, body: false };
    let mut node = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--depth" => {
                let v = value(rest, i, "--depth")?;
                a.depth = v
                    .parse()
                    .ok()
                    .filter(|&d: &usize| d >= 1)
                    .map(Some)
                    .ok_or_else(|| Refusal::new(format!("--depth takes a number of levels, 1 or more; got '{v}'")))?;
                i += 1;
            }
            "--summary" => a.summary = true,
            "--body" => a.body = true,
            x if x.starts_with("--") => return Err(unknown_option("ls", x)),
            x => {
                if node.is_some() {
                    return Err(Refusal::new("ls takes one id or path").instead("mdmap help"));
                }
                node = Some(reachable(g, resolve(g, x, "ls", rest)?, Act::Read, false, "ls")?);
            }
        }
        i += 1;
    }
    view::ls(g, node, &a, json)
}

fn search_verb(g: &Graph, rest: &[String], json: bool) -> R<Output> {
    let f = flags("search", rest, &["--summary", "--body"], usize::MAX)?;
    if f.positional.len() > 1 {
        let text = f.positional.join(" ");
        return Err(Refusal::new("search takes one text: quote it").instead(format!("mdmap search \"{text}\"")));
    }
    let text = f.positional.first().map(String::as_str).unwrap_or("");
    view::search(g, text, &view::SearchArgs { summary: f.has("--summary"), body: f.has("--body") }, json)
}

fn add_verb(g: &Graph, rest: &[String], json: bool) -> R<Output> {
    let mut parent = None;
    let mut mode = AddMode::Default;
    let mut children: Vec<Child> = Vec::new();
    let mut has_body = false;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--title" => {
                let v = value(rest, i, "--title")?;
                children.push(Child { title: Some(v.trim().to_string()), body: String::new(), template: None });
                has_body = false;
                i += 1;
            }
            "--body" => {
                let v = value(rest, i, "--body")?;
                let Some(last) = children.last_mut().filter(|c| c.title.is_some()) else {
                    return Err(Refusal::new("--body belongs to the --title before it; there is none"));
                };
                if has_body {
                    return Err(Refusal::new(format!("--title {} has two --body values", quote(last.title.as_deref().unwrap_or_default()))));
                }
                last.body = v.clone();
                has_body = true;
                i += 1;
            }
            "--template" => {
                let v = value(rest, i, "--template")?;
                let applied = Applied { name: v.trim().to_string(), fills: Vec::new() };
                match children.last_mut() {
                    None => children.push(Child { title: None, body: String::new(), template: Some(applied) }),
                    Some(last) if last.template.is_none() => last.template = Some(applied),
                    Some(_) => return Err(Refusal::new("one --title takes one --template")),
                }
                i += 1;
            }
            "--fill" => {
                let path = value(rest, i, "--fill")?.trim().trim_matches('/').to_string();
                let text = rest.get(i + 2).ok_or_else(|| Refusal::new("--fill needs a heading path and a text"))?;
                let Some(a) = children.last_mut().and_then(|c| c.template.as_mut()) else {
                    return Err(Refusal::new("--fill belongs to the --template before it; there is none"));
                };
                if a.fills.iter().any(|f| f.0 == path) {
                    return Err(Refusal::new(format!("--fill {} is given twice", quote(&path))));
                }
                a.fills.push((path, text.clone()));
                i += 2;
            }
            "--dir" => mode = AddMode::Dir,
            x if x.starts_with("--") => return Err(unknown_option("add", x)),
            x => {
                if parent.is_some() {
                    return Err(Refusal::new("add takes one parent").instead("mdmap help"));
                }
                parent = Some(reachable(g, resolve(g, x, "add", rest)?, Act::Update, false, "add")?);
            }
        }
        i += 1;
    }
    let parent = parent.ok_or_else(|| Refusal::new("add needs a parent: an id or a path").instead("mdmap ls"))?;
    write::add(g, parent, &children, mode, json)
}

/// `--append <text>`: `text` goes on the lines after the body's last line,
/// sent on as a `--body` with the whole new body.
fn append_to_body(g: &Graph, rest: &[String]) -> R<Vec<String>> {
    let Some(p) = rest.iter().position(|a| a == "--append") else { return Ok(rest.to_vec()) };
    let text = value(rest, p, "--append")?.clone();
    if rest.iter().any(|a| a == "--body" || a == "--lines" || a == "--old") {
        return Err(Refusal::new("--append takes the place of --body, --lines and --old").instead("mdmap help"));
    }
    let i = resolve(g, rest.first().map(String::as_str).unwrap_or(""), "update", rest)?;
    let body = g.body_text(i).trim_start_matches(['\n', '\r']).trim_end();
    let new = if body.is_empty() { text } else { format!("{body}\n{text}") };
    let mut out = rest[..p].to_vec();
    out.extend(["--body".to_string(), new]);
    out.extend(rest[p + 2..].iter().cloned());
    Ok(out)
}

fn update_verb(g: &Graph, rest: &[String], json: bool) -> R<Output> {
    let appended = append_to_body(g, rest)?;
    let rest: &[String] = &appended;
    let mut node = None;
    let mut a = UpdateArgs { title: None, body: None, lines: None, replace: Vec::new() };
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--title" => {
                a.title = Some(value(rest, i, "--title")?.trim().to_string());
                i += 1;
            }
            "--body" => {
                a.body = Some(value(rest, i, "--body")?.clone());
                i += 1;
            }
            "--lines" => {
                let v = value(rest, i, "--lines")?;
                let parsed = v.split_once('-').and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)));
                a.lines = Some(parsed.ok_or_else(|| Refusal::new(format!("--lines takes <a>-<b>, e.g. 48-52; got '{v}'")))?);
                i += 1;
            }
            "--old" => {
                let old = value(rest, i, "--old")?.clone();
                if rest.get(i + 2).map(String::as_str) != Some("--new") {
                    return Err(Refusal::new("--old <text> needs --new <text> right after it").instead("mdmap help"));
                }
                a.replace.push((old, value(rest, i + 2, "--new")?.clone()));
                i += 3;
            }
            "--new" => return Err(Refusal::new("--new <text> comes right after --old <text>").instead("mdmap help")),
            x if x.starts_with("--") => return Err(unknown_option("update", x)),
            x => {
                if node.is_some() {
                    return Err(Refusal::new("update takes one id or file").instead("mdmap help"));
                }
                node = Some(reachable(g, resolve(g, x, "update", rest)?, Act::Update, false, "update")?);
            }
        }
        i += 1;
    }
    let node = node.ok_or_else(|| Refusal::new("update needs an id or a file").instead("mdmap ls"))?;
    write::update(g, node, &a, json)
}

fn move_verb(g: &Graph, rest: &[String], json: bool) -> R<Output> {
    let f = flags("move", rest, &[], 2)?;
    if f.positional.len() != 2 {
        return Err(Refusal::new("move takes a heading and where it goes: move <id> <id>|<file>").instead("mdmap help"));
    }
    let node = reachable(g, resolve(g, &f.positional[0], "move", rest)?, Act::Remove, true, "move")?;
    let to = reachable(g, resolve(g, &f.positional[1], "move", rest)?, Act::Update, false, "move")?;
    write::move_node(g, node, to, json)
}

fn remove_verb(g: &Graph, rest: &[String], json: bool) -> R<Output> {
    let f = flags("remove", rest, &[], 1)?;
    let arg = f.positional.first().ok_or_else(|| Refusal::new("remove needs an id or a path").instead("mdmap ls"))?;
    let node = reachable(g, resolve(g, arg, "remove", rest)?, Act::Remove, true, "remove")?;
    write::remove(g, node, json)
}
