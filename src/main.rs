#[macro_use]
extern crate log;
extern crate chrono;
extern crate csv;
extern crate scoped_threadpool;
extern crate simple_logger;

mod by_date;
mod fame;

use docopt::Docopt;
use git2::Error;
use log::Level;
use serde_derive::Deserialize;
use std::str;

#[derive(Deserialize)]
struct Args {
    arg_command: String,
    flag_branch: Option<String>,
    flag_debug: bool,
    flag_sort: Option<String>,
    flag_threads: Option<usize>,
    flag_verbose: bool,
}

pub const DEFAULT_THREADS: usize = 10;

const USAGE: &str = "
Usage:
    grit [cmd][options]

Command:
    fame: produces counts by commit author
    bydate: produces commit counts between two specific dates.

Global Options:
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

    let result = match args.arg_command.as_str() {
        "fame" => {
            fame::process_repo(path, branch, args.flag_sort.clone(), threads);
        }
        _ => println!("Only valid commands are fame and bydate"),
    };

    Ok(result)
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
