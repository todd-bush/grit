#[macro_use]
extern crate log;
extern crate scoped_threadpool;
extern crate simple_logger;

mod fame;
mod git_graph;

use docopt::Docopt;
use git2::Error;
use log::Level;
use serde_derive::Deserialize;
use std::str;

#[derive(Deserialize)]
struct Args {
    flag_branch: Option<String>,
    flag_debug: bool,
    flag_sort: Option<String>,
    flag_threads: Option<usize>,
    flag_verbose: bool,
}

pub const DEFAULT_THREADS: usize = 10;

const USAGE: &str = "
Usage:
    grit_fame [options]

Options:
    --branch=<string>   banch to use, defaults to current HEAD
    --debug             enables debug
    -h, --help          displays help
    --sort=<field>      sort field, either 'commit' (default), 'loc', 'files'
    --threads=<number>  number of concurrent processing threads, default is 10
    --verbose
";

fn run(args: &Args) -> Result<(), Error> {
    let path = ".";

    // set logging level
    let level = if args.flag_debug {
        Level::Debug
    } else if args.flag_verbose {
        Level::Info
    } else {
        Level::Error
    };

    let branch = args
        .flag_branch
        .as_ref()
        .map(|s| &s[..])
        .unwrap_or("master");

    let threads: usize = match &args.flag_threads {
        None => DEFAULT_THREADS,
        Some(b) => *b,
    };

    simple_logger::init_with_level(level).unwrap();

    fame::process_repo(path, branch, args.flag_sort.clone(), threads)
}

fn main() {
    let args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    match run(&args) {
        Ok(()) => {}
        Err(e) => println!("error: {}", e),
    }
}
