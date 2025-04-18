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
extern crate charts;
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
use chrono::{DateTime, Local, NaiveDate, TimeZone};
use clap::{Arg, ArgMatches, Command};
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::str;

pub const DEFAULT_THREADS: usize = 10;

pub trait Processable<T> {
    fn process(&self) -> Result<T>;
}

fn parse_datelocal(date_string: String) -> Result<DateTime<Local>> {
    let utc_dt = NaiveDate::parse_from_str(date_string.as_str(), "%Y-%m-%d");

    match utc_dt {
        Ok(d) => {
            let naive_dt = d.and_hms_opt(0, 0, 0).unwrap();
            Ok(Local.from_local_datetime(&naive_dt).unwrap())
        }
        Err(_e) => {
            panic!("Dates must be in the 'YYYY-MM-DD' format ");
        }
    }
}

fn parse_date_arg(date_string: Option<String>) -> Option<DateTime<Local>> {
    let result: Option<DateTime<Local>> = match date_string {
        Some(b) => {
            let dt = parse_datelocal(b);

            Some(dt.unwrap())
        }
        None => None,
    };

    result
}

fn convert_str_string(op: Option<&str>) -> Option<String> {
    let result = match op {
        Some(s) => Some(s.to_string()),
        None => None,
    };

    result
}

fn is_svg(val: &str) -> Result<(), String> {
    if grit_utils::check_file_type(val, "svg") {
        Ok(())
    } else {
        Err(String::from("the file format must by svg"))
    }
}

fn is_csv(val: &str) -> Result<(), String> {
    if grit_utils::check_file_type(val, "csv") {
        Ok(())
    } else {
        Err(String::from("the file format must be csv"))
    }
}

fn main() {
    let arg_start_date = Arg::new("start-date")
        .help("start date in YYYY-MM-DD format")
        .long("start-date");

    let arg_end_date = Arg::new("end-date")
        .help("end date in YYYY-MM-DD format")
        .long("end-date");

    let arg_include = Arg::new("include")
        .help("comma delimited, glob file path to include path1/*,path2/*")
        .long("include");

    let arg_exclude = Arg::new("exclude")
        .help("comma delimited, glob file path to exclude path1/*,path2/*")
        .long("exclude");

    let arg_restrict_author = Arg::new("restrict-author")
        .help("comma delimited of author's names to restrict")
        .long("restrict-author");

    let arg_debug = Arg::new("debug").help("enables debug logging").short('d');
    let arg_verbose = Arg::new("verbose").help("enables info logging").short('v');

    let arg_file = Arg::new("file")
        .help("output file for the by date file.  Sends to stdout by default.  If using image flag, file name needs to be *.svg")
        .long("file").value_parser(is_svg);

    let arg_cvs_file = Arg::new("file")
        .help("output file for csv.  Must end in .csv")
        .long("file")
        .value_parser(is_csv);

    let matches = Command::new("grit")
        .about("git repository analyzer")
        .author("Todd Bush")
        .subcommand(
            Command::new("fame")
            .about("will create a table of metrics per author.  This may take a while for repos with long commit history, consider using date ranges to reduce computation time.")
            .args(&[
                Arg::new("sort")
                    .help("sort field, either 'commit', 'loc', 'files")
                    .default_value("commit")
                    .long("sort"),
                arg_start_date.clone(),
                arg_end_date.clone(),
                arg_include.clone(),
                arg_exclude.clone(),
                arg_restrict_author.clone(),
                Arg::new("csv").help("output to csv, stdout or file if file arg is present").long("csv"),
                arg_cvs_file.clone(),
                arg_debug.clone(),
                arg_verbose.clone(),
            ]),
        )
        .subcommand(
            Command::new("bydate")
            .about("will create a csv of date and commit count to stdout or file.  Option to produce a SVG image.")
            .args(&[
                arg_start_date.clone(),
                arg_end_date.clone(),
                arg_file.clone(),
                Arg::new("image")
                    .help("creates an image for the graph.  file is required")
                    .requires("file")
                    .long("image"),
                Arg::new("html")
                    .help("creates a HTML file to about visualize the SVG output")
                    .requires("image")
                    .long("html"),
                Arg::new("ignore-weekends")
                    .help("ignore weekends when calculating # of commits")
                    .long("ignore-weekends"),
                Arg::new("ignore-gap-fill")
                    .help("ignore filling empty dates with 0 commits")
                    .long("ignore-gap-fill"),
                arg_restrict_author.clone(),
                arg_debug.clone(),
                arg_verbose.clone(),
            ]),
        )
        .subcommand(
            Command::new("byfile")
            .about("will create a csv of author, date, and commit counts to stdout or file.  Option to produce a SVG image.")
            .args(&[
                Arg::new("in-file")
                    .help("input file")
                    .required(true)
                    .long("in-file"),
                arg_file.clone(),
                Arg::new("image")
                    .help("creates an image for the graph.  file is required")
                    .requires("file")
                    .long("image"),
                Arg::new("html")
                    .help("creates a HTML file to about visualize the SVG output")
                    .requires("image")
                    .long("html"),
                arg_restrict_author.clone(),
                arg_debug.clone(),
                arg_verbose.clone(),
            ]),
        )
        .subcommand(
            Command::new("effort")
            .about("will output the # of commits and # of active dates for each file.  Default is CSV, option for a table.  This may take a while for repos with long commit history, consider using date ranges to reduce computation time.")
            .args(&[
                arg_start_date.clone(),
                arg_end_date.clone(),
                arg_include,
                arg_exclude,
                arg_restrict_author.clone(),
                arg_debug.clone(),
                arg_verbose.clone(),
                Arg::new("table")
                    .help("display as a table to stdout")
                    .long("table"),
            ]),
        )
        .get_matches();

    let processasble = match matches.subcommand_name() {
        Some("fame") => handle_fame(matches.subcommand_matches("fame").unwrap()),
        Some("bydate") => handle_bydate(matches.subcommand_matches("bydate").unwrap()),
        Some("byfile") => handle_byfile(matches.subcommand_matches("byfile").unwrap()),
        Some("effort") => handle_effort(matches.subcommand_matches("effort").unwrap()),
        Some(_) => panic!("Unknown command was given"),
        None => panic!("No command was given"),
    };

    processasble.process().expect("Could not complete process");
}

fn handle_fame(args: &ArgMatches) -> Box<dyn Processable<()>> {
    set_logging(args.contains_id("debug"), args.contains_id("verbose"));
    let fame_args = FameArgs::new(
        String::from("."),
        convert_str_string(args.get_one::<String>("sort").map(|s| s.as_str())),
        parse_date_arg(args.get_one::<String>("start-date").cloned()),
        parse_date_arg(args.get_one::<String>("end-date").cloned()),
        args.get_one::<String>("include").cloned(),
        args.get_one::<String>("exclude").cloned(),
        args.get_one::<String>("restrict-author").cloned(),
        args.contains_id("csv"),
        args.get_one::<String>("file").cloned(),
    );

    Box::new(Fame::new(fame_args))
}

fn handle_bydate(args: &ArgMatches) -> Box<dyn Processable<()>> {
    set_logging(args.contains_id("debug"), args.contains_id("verbose"));
    let args = ByDateArgs::new(
        String::from("."),
        convert_str_string(args.get_one::<String>("file").map(|s| s.as_str())),
        args.contains_id("image"),
        args.contains_id("ignore-weekends"),
        args.contains_id("ignore-gap-fill"),
        args.contains_id("html"),
        args.get_one::<String>("restrict-author").cloned(),
    );

    Box::new(ByDate::new(args))
}

fn handle_byfile(args: &ArgMatches) -> Box<dyn Processable<()>> {
    set_logging(args.contains_id("debug"), args.contains_id("verbose"));
    let args = ByFileArgs::new(
        ".".to_string(),
        args.get_one::<String>("in-file").unwrap().to_string(),
        convert_str_string(args.get_one::<String>("file").map(|s| s.as_str())),
        args.contains_id("image"),
        args.contains_id("html"),
        args.get_one::<String>("restrict-author").cloned(),
    );

    Box::new(ByFile::new(args))
}

fn handle_effort(args: &ArgMatches) -> Box<dyn Processable<()>> {
    set_logging(args.contains_id("debug"), args.contains_id("verbose"));
    let ea = EffortArgs::new(
        ".".to_string(),
        parse_date_arg(args.get_one::<String>("start-date").cloned()),
        parse_date_arg(args.get_one::<String>("end-date").cloned()),
        args.contains_id("table"),
        args.get_one::<String>("include").cloned(),
        args.get_one::<String>("exclude").cloned(),
        args.get_one::<String>("restrict-author").cloned(),
    );

    Box::new(Effort::new(ea))
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

#[cfg(test)]
mod tests {
    use super::*;

    const LOG_LEVEL: LevelFilter = LevelFilter::Info;

    #[test]
    fn test_parse_datelocal_good() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let r = parse_datelocal("2020-04-01".to_string());

        match r {
            Ok(d) => println!("date parsed to {}", d),
            Err(e) => assert!(false, "error thrown {:?}", e),
        }
    }

    #[test]
    #[should_panic]
    fn test_parse_datelocal_bad() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let r = parse_datelocal("2020-04-01t".to_string());

        match r {
            Ok(d) => assert!(false, "date should of failed.  Result:{}", d),
            Err(e) => println!("error expected: {:?}", e),
        }
    }
}
