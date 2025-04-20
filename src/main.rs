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
mod effort;
mod fame;

#[cfg(test)]
#[macro_use]
mod grit_test;

pub use crate::utils::grit_utils;

use crate::by_date::{ByDate, ByDateArgs};
use crate::by_file::{ByFile, ByFileArgs};
use crate::effort::{Effort, EffortArgs};
use crate::fame::{Fame, FameArgs};

use anyhow::Result;
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::str;

pub const DEFAULT_THREADS: usize = 10;

pub trait Processable<T> {
    fn process(&self) -> Result<T>;
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    name: Option<String>,

    /// Turn debugging information on
    #[arg(short='d', long="debug", action = clap::ArgAction::SetTrue)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Adds files to myapp
    Fame { 
        name: Option<String>,

        #[arg(short='s', long="sort", help="sort field, either 'commit', 'loc', 'files", default_value = "commit")]
        sort: Option<String>,

        #[arg(long="start-date", help="start date in YYYY-MM-DD format")]
        start_date: Option<DateTime<Local>>,

        #[arg(long="end-date", help="end date in YYYY-MM-DD format")]
        end_date: Option<DateTime<Local>>,

        #[arg(long="include", help="comma delimited, glob file path to include path1/*,path2/*")]
        include: Option<String>,

        #[arg(long="exclude", help="comma delimited, glob file path to exclude path1/*,path2/*")]
        exclude: Option<String>,
        
        #[arg(short='d', long="debug", action = clap::ArgAction::SetTrue)]
        debug: bool,

        #[arg(short='v', long="verbose", action = clap::ArgAction::SetTrue)]
        verbose: bool,
        
    },
    Bydate { 
        name: Option<String>,

        #[arg(long="start-date", help="start date in YYYY-MM-DD format")]
        start_date: Option<DateTime<Local>>,

        #[arg(long="end-date", help="end date in YYYY-MM-DD format")]
        end_date: Option<DateTime<Local>>,

        #[arg(long="file", help="output file for the by date file.  Sends to stdout by default.  If using image flag, file name needs to be *.svg")]
        file: Option<String>,

        #[arg(long="image", help="creates an image for the graph.  file is required")]
        image: bool,

        #[arg(long="html", help="creates a HTML file to about visualize the SVG output")]
        html: bool,

        #[arg(long="ignore-weekends", help="ignore weekends when calculating # of commits")]
        ignore_weekends: bool,

        #[arg(long="ignore-gap-fill", help="ignore filling empty dates with 0 commits")]
        ignore_gap_fill: bool,
        
        #[arg(short='d', long="debug", action = clap::ArgAction::SetTrue)]
        debug: bool,

        #[arg(short='v', long="verbose", action = clap::ArgAction::SetTrue)]
        verbose: bool,
        
    },
    Byfile { 
        name: Option<String>,

        #[arg(long="in-file", help="input file for by_file")]
        in_file: Option<String>,    

        #[arg(long="file", help="output file for the by file file.  Sends to stdout by default.  If using image flag, file name needs to be *.svg")]
        file: Option<String>,

        #[arg(long="image", help="creates an image for the graph.  file is required")]
        image: bool,

        #[arg(long="html", help="creates a HTML file to about visualize the SVG output")]
        html: bool,

        #[arg(long="restrict-author", help="comma delimited of author's names to restrict")]
        restrict_author: Option<String>,

        #[arg(short='d', long="debug", action = clap::ArgAction::SetTrue)]
        debug: bool,

        #[arg(short='v', long="verbose", action = clap::ArgAction::SetTrue)]
        verbose: bool,


    },
    Effort { 
        name: Option<String>,

        #[arg(long="start-date", help="start date in YYYY-MM-DD format")]
        start_date: Option<DateTime<Local>>,

        #[arg(long="end-date", help="end date in YYYY-MM-DD format")]
        end_date: Option<DateTime<Local>>,

        #[arg(long="table", help="display as a table to stdout")]
        table: bool,

        #[arg(long="include", help="comma delimited, glob file path to include path1/*,path2/*")]
        include: Option<String>,

        #[arg(long="exclude", help="comma delimited, glob file path to exclude path1/*,path2/*")]
        exclude: Option<String>,

        #[arg(short='d', long="debug", action = clap::ArgAction::SetTrue)]
        debug: bool,

        #[arg(short='v', long="verbose", action = clap::ArgAction::SetTrue)]
        verbose: bool,
        
    },
}


fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Fame { name: _, 
            sort, 
            start_date, 
            end_date, 
            include, 
            exclude,
            debug,
            verbose }) => {
            let fame_args = FameArgs::new(
                String::from("."),
                sort,
                start_date,
                end_date,
                include,
                exclude,
                None, false, None
            );
            set_logging(debug, verbose);
            Fame::new(fame_args).process().unwrap();
        }
        Some(Commands::Bydate { name: _, 
            start_date: _, 
            end_date: _, 
            file, 
            image, 
            html, 
            ignore_weekends, 
            ignore_gap_fill,
            debug,
            verbose }) => {
            let bydate_args = ByDateArgs::new(
                String::from("."),
                file,
                image,
                ignore_weekends,
                ignore_gap_fill,
                html,
                None,
            );
            set_logging(debug, verbose);
            ByDate::new(bydate_args).process().unwrap();
        }
        Some(Commands::Byfile { name: _, 
            in_file , 
            file, 
            image, 
            html, 
            restrict_author,
            debug,
            verbose }) => {
            let byfile_args = ByFileArgs::new(
                String::from("."),
                in_file.unwrap(),
                file,
                image,
                html,
                restrict_author,
            );
            set_logging(debug, verbose);
            ByFile::new(byfile_args).process().unwrap();
        }
        Some(Commands::Effort { name: _, 
            start_date, 
            end_date, 
            table, 
            include, 
            exclude,
            debug,
            verbose }) => {
            let effort_args = EffortArgs::new(
                String::from("."),
                start_date,
                end_date,
                table,
                include,
                exclude,
                None,
            );
            set_logging(debug, verbose);
            Effort::new(effort_args).process().unwrap();
        }
        _ => {}
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

