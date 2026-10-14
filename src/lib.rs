//! mdmap: the docs as one tree an agent walks instead of opening files.
//! Folders and files are named by their path, headings by a random id that
//! never changes; every node shows the size of its neighbours before it is
//! opened.
//!
//! A program of its own: it reads the corpus, its settings (`mdmap.yml`) and,
//! for `stats`, the log of a harno project when there is one.
//! [`cli::main`] takes its arguments, its
//! directory and its output explicitly so the tests can drive it in-process.

pub mod check;
pub mod cli;
pub mod config;
pub mod exec;
pub mod export;
pub mod graph;
pub mod id;
pub mod lint;
pub mod markdown;
pub mod out;
pub mod slug;
pub mod stats;
pub mod verify;
pub mod view;
pub mod write;

pub use cli::{main, Env};
