//! Running a verify command: the words of its line, the entry of `verify.run`
//! that allows it, and the command that runs.
//!
//! A command is an argument vector, never a shell string: `&&`, `|`, `$VAR`
//! and `;` are plain arguments. The string is split the way a shell would
//! split words (quotes group, backslash escapes), and an unquoted `*`, `?`
//! or `[` is expanded as a glob against the project directory.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::{Allow, ARGS};
use crate::out::{Refusal, R};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    /// The word with quotes removed.
    pub text: String,
    /// A glob pattern when the word has an unquoted wildcard.
    pub pattern: Option<String>,
}

pub fn split(s: &str) -> R<Vec<Word>> {
    let mut words = Vec::new();
    let mut cur: Option<(String, String, bool)> = None; // text, pattern, has wildcard
    let mut chars = s.chars().peekable();
    let push = |cur: &mut Option<(String, String, bool)>, c: char, quoted: bool| {
        let w = cur.get_or_insert_with(|| (String::new(), String::new(), false));
        w.0.push(c);
        if !quoted && matches!(c, '*' | '?' | '[') {
            w.1.push(c);
            w.2 = true;
        } else {
            w.1.push_str(&glob::Pattern::escape(&c.to_string()));
        }
    };
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if let Some((t, p, wild)) = cur.take() {
                    words.push(Word { text: t, pattern: wild.then_some(p) });
                }
            }
            '\'' => {
                cur.get_or_insert_with(|| (String::new(), String::new(), false));
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => push(&mut cur, c, true),
                        None => return Err(Refusal::new(format!("unclosed ' in {s:?}"))),
                    }
                }
            }
            '"' => {
                cur.get_or_insert_with(|| (String::new(), String::new(), false));
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => push(&mut cur, c, true),
                            Some(c) => {
                                push(&mut cur, '\\', true);
                                push(&mut cur, c, true);
                            }
                            None => return Err(Refusal::new(format!("unclosed \" in {s:?}"))),
                        },
                        Some(c) => push(&mut cur, c, true),
                        None => return Err(Refusal::new(format!("unclosed \" in {s:?}"))),
                    }
                }
            }
            '\\' => match chars.next() {
                Some(c) => push(&mut cur, c, true),
                None => push(&mut cur, '\\', true),
            },
            c => push(&mut cur, c, false),
        }
    }
    if let Some((t, p, wild)) = cur.take() {
        words.push(Word { text: t, pattern: wild.then_some(p) });
    }
    Ok(words)
}

/// The argument vector: globs expanded against `dir`, sorted; a glob that
/// matches nothing stays as written.
pub fn argv(words: &[Word], dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for w in words {
        let Some(p) = &w.pattern else {
            out.push(w.text.clone());
            continue;
        };
        let full = if Path::new(p).is_absolute() { p.clone() } else { format!("{}/{p}", glob::Pattern::escape(&dir.to_string_lossy())) };
        let mut matches: Vec<String> = glob::glob(&full)
            .map(|paths| {
                paths
                    .flatten()
                    .map(|m| {
                        if Path::new(p).is_absolute() {
                            m.to_string_lossy().to_string()
                        } else {
                            m.strip_prefix(dir).unwrap_or(&m).to_string_lossy().to_string()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        matches.sort();
        if matches.is_empty() {
            out.push(w.text.clone());
        } else {
            out.extend(matches);
        }
    }
    out
}

/// Word-by-word prefix match: `cargo build` lets `cargo build --release`
/// through but not `cargo buildx`. A word with a wildcard in the allow entry
/// is a glob. Of the entries that match, the longest: its timeout counts.
pub fn allowed<'a>(allow: &'a [Allow], words: &[Word]) -> Option<&'a Allow> {
    allow
        .iter()
        .filter(|entry| {
            let Ok(e) = split(&entry.command) else { return false };
            !e.is_empty()
                && e.len() <= words.len()
                && e.iter().zip(words).all(|(a, w)| match &a.pattern {
                    Some(p) => glob::Pattern::new(p).is_ok_and(|g| g.matches(&w.text)),
                    None => a.text == w.text,
                })
        })
        .max_by_key(|entry| split(&entry.command).map(|e| e.len()).unwrap_or(0))
}

/// Why a command is refused: the program when nothing in the list names it,
/// otherwise the whole command.
pub fn refusal_name(allow: &[Allow], words: &[Word], raw: &str) -> String {
    let prog = words.first().map(|w| w.text.as_str()).unwrap_or("");
    let named = allow.iter().any(|e| split(&e.command).ok().and_then(|v| v.first().cloned()).is_some_and(|w| w.text == prog));
    if named {
        raw.to_string()
    } else {
        prog.to_string()
    }
}

/// Run one command. With `stderr`, stdout and stderr share one pipe so the
/// output reads in the order it was written; without, stderr is dropped.
pub fn exec(command: &str, dir: &Path, timeout: Duration, stderr: bool) -> (i32, String, String, f64) {
    let started = Instant::now();
    let words = match split(command) {
        Ok(w) if !w.is_empty() => w,
        Ok(_) => return (2, "FAIL empty command".into(), String::new(), 0.0),
        Err(e) => return (2, format!("FAIL {}", e.reason), String::new(), 0.0),
    };
    let args = argv(&words, dir);
    let program = if args[0].contains('/') && !Path::new(&args[0]).is_absolute() {
        dir.join(&args[0]).to_string_lossy().to_string()
    } else {
        args[0].clone()
    };
    let (reader, writer) = match std::io::pipe() {
        Ok(p) => p,
        Err(e) => return (1, format!("FAIL {e}"), String::new(), 0.0),
    };
    let mut cmd = Command::new(&program);
    cmd.args(&args[1..]).current_dir(dir).stdin(Stdio::null());
    if stderr {
        match writer.try_clone() {
            Ok(w2) => {
                cmd.stdout(w2);
                cmd.stderr(writer);
            }
            Err(e) => return (1, format!("FAIL {e}"), String::new(), 0.0),
        }
    } else {
        cmd.stdout(writer);
        cmd.stderr(Stdio::null());
    }
    let child = cmd.spawn();
    drop(cmd);
    let mut child = match child {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (127, "FAIL not found".into(), format!("{}: command not found\n", args[0]), secs(started))
        }
        Err(e) => return (126, format!("FAIL {e}"), String::new(), secs(started)),
    };
    let buf = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    {
        let buf = buf.clone();
        let mut reader = reader;
        std::thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.lock().unwrap().extend_from_slice(&chunk[..n]),
                }
            }
            let _ = tx.send(());
        });
    }
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    timed_out = true;
                    break child.wait().ok();
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break None,
        }
    };
    // A background grandchild may hold the pipe open; do not wait for it.
    let _ = rx.recv_timeout(Duration::from_millis(if timed_out { 100 } else { 2000 }));
    let output = String::from_utf8_lossy(&buf.lock().unwrap()).to_string();
    let elapsed = secs(started);
    if timed_out {
        return (124, "FAIL timeout".into(), output, elapsed);
    }
    let code = status.and_then(|s| s.code()).unwrap_or(1);
    let st = if code == 0 { "ok".to_string() } else { format!("FAIL exit {code}") };
    (code, st, output, elapsed)
}

fn secs(started: Instant) -> f64 {
    started.elapsed().as_secs_f64()
}
/// The command of an entry with `{args}` replaced by the other words of the
/// line, each one word that is not split or expanded again; a command without
/// `{args}` takes them at its end.
pub fn with_args(command: &str, args: &[String]) -> String {
    let words: Vec<String> = args.iter().map(|a| single_quote(a)).collect();
    if !command.contains(ARGS) && !words.is_empty() {
        return format!("{command} {}", words.join(" "));
    }
    command.replace(ARGS, &words.join(" ")).trim_end().to_string()
}
/// Quote a word so `split` reads it back exactly: nothing inside single
/// quotes is special.
fn single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
