#[macro_use]
extern crate log;
extern crate chrono;
extern crate csv;
extern crate scoped_threadpool;
extern crate simple_logger;

mod by_date;
mod fame;

use crate::by_date::ByDateArgs;
use crate::chrono::TimeZone;
use crate::fame::FameArgs;
use chrono::{Date, Local, NaiveDate};
use docopt::Docopt;
use git2::Error;
use log::Level;
use serde_derive::Deserialize;
use std::str;

#[derive(Debug, Deserialize)]
struct Args {
    flag_debug: bool,
    flag_sort: Option<String>,
    flag_threads: Option<usize>,
    flag_verbose: bool,
    flag_start_date: Option<String>,
    flag_end_date: Option<String>,
    flag_file: Option<String>,
    flag_image: bool,
    cmd_fame: bool,
    cmd_bydate: bool,
}

pub const DEFAULT_THREADS: usize = 10;

const USAGE: &str = "
Grit.

Usage:
    grit fame [--sort=<field>] [--debug]
    grit bydate [--start-date=<string>] [--end-date=<string>] [--file=<string>] [--image] [--debug]

Command:
    fame: produces counts by commit author
    bydate: produces commit counts between two specific dates.

Options:
    --debug                     enables debug
    -h, --help                  displays help
    --sort=<field>              sort field, either 'commit' (default), 'loc', 'files'
    --threads=<number>          number of concurrent processing threads, default is 10
    --start-date=<string>       start date for bydate in YYYY-MM-DD format.
    --end-date=<string>         end date for bydate in YYYY-MM-DD format.
    --file=<string>             output file for the by date file.  Sends to stdout by default
    --image                     creates an image for the by_date graph.  file is required
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

    simple_logger::init_with_level(level).unwrap();

    if args.cmd_fame {
        let threads: usize = match &args.flag_threads {
            None => DEFAULT_THREADS,
            Some(b) => *b,
        };

        let fame_args = FameArgs::new(path.to_string(), args.flag_sort.clone(), threads);

        fame::process_fame(fame_args)?;
    } else if args.cmd_bydate {
        let start_date: Option<Date<Local>> = match &args.flag_start_date {
            Some(b) => {
                let dt = parse_datelocal(b);
                Some(dt)
            }
            None => None,
        };

        let end_date: Option<Date<Local>> = match &args.flag_end_date {
            Some(d) => {
                let dt = parse_datelocal(d);
                Some(dt)
            }
            None => None,
        };

        if args.flag_image {
            match args.flag_file {
                None => panic!("File is requird when selecting image"),
                Some(_) => (),
            }
        }

        let by_date_args = ByDateArgs::new(
            start_date,
            end_date,
            args.flag_file.clone(),
            args.flag_image,
        );
        by_date::by_date(path, by_date_args)?;
    };

    Ok(())
}

fn parse_datelocal(date_string: &str) -> Date<Local> {
    let local_now = Local::now();
    let utc_dt = NaiveDate::parse_from_str(date_string, "%Y-%m-%d").unwrap();

    local_now
        .timezone()
        .from_local_date(&utc_dt)
        .single()
        .unwrap()
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
