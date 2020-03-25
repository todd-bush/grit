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

#[derive(Debug, Deserialize)]
struct Args {
    flag_branch: Option<String>,
    flag_debug: bool,
    flag_sort: Option<String>,
    flag_threads: Option<usize>,
    flag_verbose: bool,
    cmd_fame: bool,
    cmd_bydate: bool,
}

pub const DEFAULT_THREADS: usize = 10;

const USAGE: &str = "
Grit.

Usage:
    grit fame [--branch=<string>] [--sort=<field>]
    grit bydate [--branch=<string>] [--start_date=<string>] [--end_date=<string>]

Command:
    fame: produces counts by commit author
    bydate: produces commit counts between two specific dates.

Options:
    --branch=<string>           branch to use, defaults to current HEAD
    --debug                     enables debug
    -h, --help                  displays help
    --sort=<field>              sort field, either 'commit' (default), 'loc', 'files'
    --threads=<number>          number of concurrent processing threads, default is 10
    --start_date=<string>       start date for bydate in YYYY-MM-DD format.
    --end_date=<string>         end date for bydate in YYYY-MM-DD format.
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

    let 

    simple_logger::init_with_level(level).unwrap();

    let result = if args.cmd_fame {
        fame::process_repo(path, branch, args.flag_sort.clone(), threads);
    } else if args.cmd_bydate {
        by_date::by_date(path, None, None);
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
