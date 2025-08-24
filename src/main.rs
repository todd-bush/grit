//! grit
//! Usage:
//! grit fame [--sort=<field>] [--start-days-back=<int>] [--end-days-back=<int>] [--include=<string>] [--exclude=<string>] [--verbose] [--debug]
//! grit bydate [--start-days-back=<int>] [--end-days-back=<int>] [--file=<string>] [--ignore-weekends] [--ignore-gap-fill] [--verbose] [--debug]
//! grit byfile [--in-file=<string>] [--file=<string>] [--verbose] [--debug]
//! grit effort [--start-days-back=<int>] [--end-days-back=<int>] [--table] [--include=<string>] [--exclude=<string>] [--verbose] [--debug]
//!
//! Options:
//! -h, --about                  displays about
//! --sort=<field>              sort field, either 'commit' (default), 'loc', 'files'
//! --start-days-back=<int>     start date in days back from today.
//! --end-days-back=<int>       end date in days back from today.
//! --include=<string>          comma delimited, glob file path to include path1/*,path2/*
//! --exclude=<string>          comma delimited, glob file path to exclude path1/*,path2/*
//! --file=<string>             output file for the by date file.  Sends to stdout by default.
//! --in-file=<string>          input file for by_file
//! --table                     display as a table to stdout
//! --ignore-weekends           ignore weekends when calculating # of commits
//! --ignore-gap-fill           ignore filling empty dates with 0 commits
//! --log-level=<string>        set the log level, one of: error, warn, info, debug, trace

#[macro_use]
extern crate log;
extern crate anyhow;
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

#[cfg(test)]
#[macro_use]
mod grit_test;

use crate::cli::Cli;
pub use crate::utils::grit_utils;
use anyhow::Result;
use clap::Parser;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::str::FromStr;

pub const DEFAULT_THREADS: usize = 10;

pub trait Processable<T> {
    fn process(&self) -> Result<T>;
}

fn main() {
    let cli = Cli::parse();

    set_logging(cli.log_level);

    if let Some(command) = cli.command {
        if let Err(e) = command.execute() {
            eprintln!("Error executing command: {e}");
            std::process::exit(1);
        }
    }
}

fn set_logging(log_level: Option<String>) {
    let level = match log_level {
        Some(level) => level,
        None => "info".to_string(),
    };

    let level = LevelFilter::from_str(&level).unwrap_or(LevelFilter::Info);

    info!("Setting log level to {}", level);

    SimpleLogger::new().with_level(level).init().unwrap();
}
