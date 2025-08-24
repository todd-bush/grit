use crate::Processable;
use crate::by_date::{ByDate, ByDateArgs};
use crate::by_file::{ByFile, ByFileArgs};
use crate::effort::{Effort, EffortArgs};
use crate::fame::{Fame, FameArgs};
use anyhow::Result;
use clap::{Parser, Subcommand};

fn parse_log_level(s: &str) -> Result<String, String> {
    match s.to_lowercase().as_str() {
        "error" | "warn" | "info" | "debug" | "trace" => Ok(s.to_string()),
        _ => Err("Log level must be one of: error, warn, info, debug, trace".to_string()),
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    name: Option<String>,

    #[arg(short='l', long="log-level", help="set the log level", default_value = "info", value_parser = parse_log_level)]
    pub log_level: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze commit fame
    Fame(FameCommand),
    /// Analyze commits by date
    Bydate(ByDateCommand),
    /// Analyze commits by file
    Byfile(ByFileCommand),
    /// Analyze development effort
    Effort(EffortCommand),
}

impl Commands {
    pub fn execute(&self) -> Result<()> {
        match self {
            Commands::Fame(cmd) => cmd.execute(),
            Commands::Bydate(cmd) => cmd.execute(),
            Commands::Byfile(cmd) => cmd.execute(),
            Commands::Effort(cmd) => cmd.execute(),
        }
    }
}

#[derive(Parser)]
pub struct FameCommand {
    name: Option<String>,

    #[arg(
        short = 's',
        long = "sort",
        help = "sort field, either 'commit', 'loc', 'files",
        default_value = "commit"
    )]
    sort: Option<String>,

    #[arg(
        long = "start-days-back",
        help = "the number of days back to collect data from"
    )]
    start_days_back: Option<u32>,

    #[arg(
        long = "end-days-back",
        help = "the number of days back to collect data to"
    )]
    end_days_back: Option<u32>,

    #[arg(
        long = "include",
        help = "comma delimited, glob file path to include path1/*,path2/*"
    )]
    include: Option<String>,

    #[arg(
        long = "exclude",
        help = "comma delimited, glob file path to exclude path1/*,path2/*"
    )]
    exclude: Option<String>,
}

impl FameCommand {
    fn execute(&self) -> Result<()> {
        let fame_args = FameArgs::new(
            String::from("."),
            self.sort.clone(),
            self.start_days_back,
            self.end_days_back,
            self.include.clone(),
            self.exclude.clone(),
            None,
            false,
            None,
        );
        Fame::new(fame_args).process()?;
        Ok(())
    }
}

#[derive(Parser)]
pub struct ByDateCommand {
    name: Option<String>,

    #[arg(
        long = "start-days-back",
        help = "the number of days back to collect data from"
    )]
    start_days_back: Option<u32>,

    #[arg(
        long = "end-days-back",
        help = "the number of days back to collect data to"
    )]
    end_days_back: Option<u32>,

    #[arg(
        long = "file",
        help = "output file for the by date file.  Sends to stdout by default.  If using image flag, file name needs to be *.svg"
    )]
    file: Option<String>,

    #[arg(
        long = "ignore-weekends",
        help = "ignore weekends when calculating # of commits"
    )]
    ignore_weekends: bool,

    #[arg(
        long = "ignore-gap-fill",
        help = "ignore filling empty dates with 0 commits"
    )]
    ignore_gap_fill: bool,
}

impl ByDateCommand {
    fn execute(&self) -> Result<()> {
        let bydate_args = ByDateArgs::new(
            String::from("."),
            self.file.clone(),
            self.ignore_weekends,
            self.ignore_gap_fill,
            None,
        );
        ByDate::new(bydate_args).process()?;
        Ok(())
    }
}

#[derive(Parser)]
pub struct ByFileCommand {
    name: Option<String>,

    #[arg(long = "in-file", help = "input file for by_file")]
    in_file: Option<String>,

    #[arg(
        long = "file",
        help = "output file for the by file file.  Sends to stdout by default.  If using image flag, file name needs to be *.svg"
    )]
    file: Option<String>,

    #[arg(
        long = "restrict-author",
        help = "comma delimited of author's names to restrict"
    )]
    restrict_author: Option<String>,
}

impl ByFileCommand {
    fn execute(&self) -> Result<()> {
        let byfile_args = ByFileArgs::new(
            String::from("."),
            self.in_file.clone().unwrap(),
            self.file.clone(),
            self.restrict_author.clone(),
        );
        ByFile::new(byfile_args).process()?;
        Ok(())
    }
}

#[derive(Parser)]
pub struct EffortCommand {
    name: Option<String>,

    #[arg(
        long = "start-days-back",
        help = "the number of days back to collect data from"
    )]
    start_days_back: Option<u32>,

    #[arg(
        long = "end-days-back",
        help = "the number of days back to collect data to"
    )]
    end_days_back: Option<u32>,

    #[arg(long = "table", help = "display as a table to stdout")]
    table: bool,

    #[arg(
        long = "include",
        help = "comma delimited, glob file path to include path1/*,path2/*"
    )]
    include: Option<String>,

    #[arg(
        long = "exclude",
        help = "comma delimited, glob file path to exclude path1/*,path2/*"
    )]
    exclude: Option<String>,
}

impl EffortCommand {
    fn execute(&self) -> Result<()> {
        let effort_args = EffortArgs::new(
            String::from("."),
            self.start_days_back,
            self.end_days_back,
            self.table,
            self.include.clone(),
            self.exclude.clone(),
            None,
        );
        Effort::new(effort_args).process()?;
        Ok(())
    }
}
