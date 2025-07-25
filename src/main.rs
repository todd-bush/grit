//! grit
//! Usage:
//! grit fame [--sort=<field>] [--start-date=<string>] [--end-date=<string>] [--include=<string>] [--exclude=<string>] [--verbose] [--debug]
//! grit bydate [--start-date=<string>] [--end-date=<string>] [--file=<string>] [--image] [--html] [--ignore-weekends] [--ignore-gap-fill] [--verbose] [--debug]
//! grit byfile [--in-file=<string>] [--file=<string>] [--image] [--html] [--verbose] [--debug]
//! grit effort [--start-date=<string>] [--end-date=<string>] [--table] [--include=<string>] [--exclude=<string>] [--verbose] [--debug]
//!
//! Options:
//! --debug                     enables debug
//! -h, --about                  displays about
//! --sort=<field>              sort field, either 'commit' (default), 'loc', 'files'
//! --start-date=<string>       start date in YYYY-MM-DD format.
//! --end-date=<string>         end date in YYYY-MM-DD format.
//! --include=<string>          comma delimited, glob file path to include path1/*,path2/*
//! --exclude=<string>          comma delimited, glob file path to exclude path1/*,path2/*
//! --file=<string>             output file for the by date file.  Sends to stdout by default.  If using image flag, file name needs to be *.svg
//! --in-file=<string>          input file for by_file
//! --image                     creates an image for the by_date & by_file graph.  file is required
//! --html                      creates a HTML file to about visualize the SVG output
//! --table                     display as a table to stdout
//! --ignore-weekends           ignore weekends when calculating # of commits
//! --ignore-gap-fill           ignore filling empty dates with 0 commits
//! -v, --verbose

#[macro_use]
extern crate log;
extern crate anyhow;
extern crate charts_rs;
extern crate chrono;
extern crate clap;
extern crate csv;
extern crate simple_logger;
extern crate tokio;

#[macro_use]
mod utils;

mod by_date;
mod by_file;
mod cli;
mod effort;
mod fame;
mod git_utils;

#[cfg(test)]
#[macro_use]
mod grit_test;

use crate::cli::Cli;
pub use crate::utils::grit_utils;
use anyhow::Result;
use clap::Parser;
use log::LevelFilter;
use simple_logger::SimpleLogger;

pub const DEFAULT_THREADS: usize = 10;

pub trait Processable<T> {
    fn process(&self) -> Result<T>;
}

fn main() {
    let cli = Cli::parse();
    set_logging(cli.debug, cli.verbose);

    if let Some(command) = cli.command {
        if let Err(e) = command.execute() {
            eprintln!("Error executing command: {e}");
            std::process::exit(1);
        }
    }
}

fn set_logging(debug: bool, verbose: bool) {
    let level = if debug {
        LevelFilter::Debug
    } else if verbose {
        LevelFilter::Info
    } else {
        LevelFilter::Error
    };

    SimpleLogger::new().with_level(level).init().unwrap();
}
