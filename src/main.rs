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

use crate::by_date::ByDateArgs;
use crate::by_file::ByFileArgs;
use crate::chrono::TimeZone;
use crate::effort::EffortArgs;
use crate::fame::FameArgs;

use anyhow::Result;
use chrono::{Date, Local, NaiveDate};
use clap::{App, Arg};
use docopt::Docopt;
use log::Level;
use serde_derive::Deserialize;
use std::str;

#[derive(Debug, Deserialize)]
struct Args {
    flag_debug: bool,
    flag_sort: Option<String>,
    flag_verbose: bool,
    flag_start_date: Option<String>,
    flag_end_date: Option<String>,
    flag_file: Option<String>,
    flag_include: Option<String>,
    flag_exclude: Option<String>,
    flag_image: bool,
    flag_ignore_weekends: bool,
    flag_ignore_gap_fill: bool,
    flag_in_file: Option<String>,
    flag_html: bool,
    flag_table: bool,
    cmd_fame: bool,
    cmd_bydate: bool,
    cmd_byfile: bool,
    cmd_effort: bool,
}

pub const DEFAULT_THREADS: usize = 10;

const USAGE: &str = "
Grit.

Usage:
    grit fame [--sort=<field>] [--start-date=<string>] [--end-date=<string>] [--include=<string>] [--exclude=<string>] [--verbose] [--debug]
    grit bydate [--start-date=<string>] [--end-date=<string>] [--file=<string>] [--image] [--html] [--ignore-weekends] [--ignore-gap-fill] [--verbose] [--debug]
    grit byfile [--in-file=<string>] [--file=<string>] [--image] [--html] [--verbose] [--debug]
    grit effort [--start-date=<string>] [--end-date=<string>] [--table] [--include=<string>] [--exclude=<string>] [--verbose] [--debug]

Options:
    --debug                     enables debug
    -h, --help                  displays help
    --sort=<field>              sort field, either 'commit' (default), 'loc', 'files'
    --start-date=<string>       start date in YYYY-MM-DD format.
    --end-date=<string>         end date in YYYY-MM-DD format.
    --include=<string>          comma delimited, glob file path to include path1/*,path2/*
    --exclude=<string>          comma delimited, glob file path to exclude path1/*,path2/*
    --file=<string>             output file for the by date file.  Sends to stdout by default.  If using image flag, file name needs to be *.svg
    --in-file=<string>          input file for by_file
    --image                     creates an image for the by_date & by_file graph.  file is required
    --html                      creates a HTML file to help visualize the SVG output
    --table                     display as a table to stdout
    --ignore-weekends           ignore weekends when calculating # of commits
    --ignore-gap-fill           ignore filling empty dates with 0 commits
    -v, --verbose
";

fn run(args: &Args) -> Result<()> {
    let path = ".";

    // set logging level
    let level = if args.flag_debug {
        Level::Debug
    } else if args.flag_verbose {
        Level::Info
    } else {
        Level::Error
    };

    simple_logger::init_with_level(level).unwrap();

    let start_date: Option<Date<Local>> = match &args.flag_start_date {
        Some(b) => {
            let dt = parse_datelocal(b);

            Some(dt?)
        }
        None => None,
    };

    let end_date: Option<Date<Local>> = match &args.flag_end_date {
        Some(d) => {
            let dt = parse_datelocal(d);

            Some(dt?)
        }
        None => None,
    };

    if args.cmd_fame {
        let fame_args = FameArgs::new(
            path.to_string(),
            args.flag_sort.clone(),
            start_date,
            end_date,
            args.flag_include.clone(),
            args.flag_exclude.clone(),
        );

        fame::process_fame(fame_args)?;
    } else if args.cmd_bydate {
        if args.flag_image {
            match args.flag_file.clone() {
                None => {
                    error!("Argument 'flag_file' is required when selecting image.");
                    std::process::exit(1);
                }
                Some(f) => {
                    if !grit_utils::check_file_type(&f, "svg") {
                        error!("Argument 'flag_file' must end with .svg");
                        std::process::exit(1);
                    }
                }
            }
        }

        let by_date_args = ByDateArgs::new(
            start_date,
            end_date,
            args.flag_file.clone(),
            args.flag_image,
            args.flag_ignore_weekends,
            args.flag_ignore_gap_fill,
            args.flag_html,
        );
        by_date::by_date(path, by_date_args)?;
    } else if args.cmd_byfile {
        let in_file = match args.flag_in_file.clone() {
            Some(f) => f,
            None => {
                error!("Argument 'flag_in_file' is required for byfile");
                std::process::exit(1);
            }
        };

        if !grit_utils::check_file_type(&in_file, "svg") {
            error!("Argument 'flag_in_file' must end with .svg");
            std::process::exit(1);
        }

        let by_file_args = ByFileArgs::new(
            path.to_string(),
            in_file,
            args.flag_file.clone(),
            args.flag_image,
            args.flag_html,
        );
        by_file::by_file(by_file_args)?;
    } else if args.cmd_effort {
        let ea = EffortArgs::new(
            path.to_string(),
            start_date,
            end_date,
            args.flag_table,
            args.flag_include.clone(),
            args.flag_exclude.clone(),
        );
        effort::effort(ea)?;
    };

    Ok(())
}

fn parse_datelocal(date_string: &str) -> Result<Date<Local>> {
    let utc_dt = NaiveDate::parse_from_str(date_string, "%Y-%m-%d");

    match utc_dt {
        Ok(d) => Ok(Local
            .from_local_date(&d)
            .single()
            .expect("Cannot unwrap date")),
        Err(_e) => {
            panic!("Dates must be in the 'YYYY-MM-DD' format ");
        }
    }
}

fn main() {
    let args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    match run(&args) {
        Ok(()) => {}
        Err(e) => println!("error: {}", e),
    }

    let arg_start_date = Arg::new("start-date")
        .about("start date in YYYY-MM-DD format")
        .takes_value(true)
        .long("start-date");

    let arg_end_date = Arg::new("end-date")
        .about("end date in YYYY-MM-DD format")
        .takes_value(true)
        .long("end-date");

    let arg_include = Arg::new("include")
        .about("comma delimited, glob file path to include path1/*,path2/*")
        .takes_value(true)
        .long("include");

    let arg_exclude = Arg::new("exclude")
        .about("comma delimited, glob file path to exclude path1/*,path2/*")
        .takes_value(true)
        .long("exclude");

    let arg_file = Arg::new("file").about("output file for the by date file.  Sends to stdout by default.  If using image flag, file name needs to be *.svg").takes_value(true).long("file");

    let matches = App::new("Grit")
        .about("git repository analyzer")
        .author("Todd Bush")
        .arg(Arg::new("debug").about("enables debug logging").short('d'))
        .arg(Arg::new("verbose").about("enables info logging").short('v'))
        .subcommand(
            App::new("fame").args(&[
                Arg::new("sort")
                    .about("sort field, either 'commit', 'loc', 'files")
                    .takes_value(true)
                    .long("sort"),
                arg_start_date.clone(),
                arg_end_date.clone(),
                arg_include.clone(),
                arg_exclude.clone(),
            ]),
        )
        .subcommand(
            App::new("bydate").args(&[
                arg_start_date.clone(),
                arg_end_date.clone(),
                arg_file.clone(),
                Arg::new("image")
                    .about("creates an image for the graph.  file is required")
                    .requires("file")
                    .takes_value(false)
                    .long("image"),
                Arg::new("html")
                    .about("creates a HTML file to help visualize the SVG output")
                    .requires("image")
                    .takes_value(false)
                    .long("html"),
                Arg::new("ignore-weekends")
                    .about("ignore weekends when calculating # of commits")
                    .takes_value(false)
                    .long("ignore-weekends"),
                Arg::new("ignore-gap-fill")
                    .about("ignore filling empty dates with 0 commits")
                    .takes_value(false)
                    .long("ignore-gap-fill"),
            ]),
        )
        .subcommand(
            App::new("byfile").args(&[
                Arg::new("in-file")
                    .about("input file")
                    .takes_value(true)
                    .long("in-file"),
                arg_file.clone(),
                Arg::new("image")
                    .about("creates an image for the graph.  file is required")
                    .requires("file")
                    .takes_value(false)
                    .long("image"),
                Arg::new("html")
                    .about("creates a HTML file to help visualize the SVG output")
                    .requires("image")
                    .takes_value(false)
                    .long("html"),
            ]),
        )
        .subcommand(
            App::new("effort").args(&[
                arg_start_date.clone(),
                arg_end_date.clone(),
                arg_include,
                arg_exclude,
                Arg::new("table")
                    .about("display as a table to stdout")
                    .takes_value(false)
                    .long("table"),
            ]),
        )
        .get_matches();

        let level = if matches.is_present("debug") {
            Level::Debug
        } else if matches.is_present("verbose") {
            Level::Info
        } else {
            Level::Error
        };

        simple_logger::init_with_level(level).unwrap();

}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG_LEVEL: Level = Level::Info;

    #[test]
    fn test_parse_datelocal_good() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let r = parse_datelocal("2020-04-01");

        match r {
            Ok(d) => println!("date parsed to {}", d),
            Err(e) => assert!(false, "error thrown {:?}", e),
        }
    }

    #[test]
    #[should_panic]
    fn test_parse_datelocal_bad() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let r = parse_datelocal("2020-04-01t");

        match r {
            Ok(d) => assert!(false, "date should of failed.  Result:{}", d),
            Err(e) => println!("error expected: {:?}", e),
        }
    }
}
