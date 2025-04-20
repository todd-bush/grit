use crate::by_date::{ByDate, ByDateArgs};
use crate::by_file::{ByFile, ByFileArgs};
use crate::effort::{Effort, EffortArgs};
use crate::fame::{Fame, FameArgs};
use crate::Processable;
use anyhow::Result;
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    name: Option<String>,

    /// Turn debugging information on
    #[arg(short='d', long="debug", action = clap::ArgAction::SetTrue)]
    pub debug: bool,

    /// Enable verbose output
    #[arg(short='v', long="verbose", action = clap::ArgAction::SetTrue)]
    pub verbose: bool,

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
}

impl FameCommand {
    fn execute(&self) -> Result<()> {
        let fame_args = FameArgs::new(
            String::from("."),
            self.sort.clone(),
            self.start_date,
            self.end_date,
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
}

impl ByDateCommand {
    fn execute(&self) -> Result<()> {
        let bydate_args = ByDateArgs::new(
            String::from("."),
            self.file.clone(),
            self.image,
            self.ignore_weekends,
            self.ignore_gap_fill,
            self.html,
            None,
        );
        ByDate::new(bydate_args).process()?;
        Ok(())
    }
}

#[derive(Parser)]
pub struct ByFileCommand {
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
}

impl ByFileCommand {
    fn execute(&self) -> Result<()> {
        let byfile_args = ByFileArgs::new(
            String::from("."),
            self.in_file.clone().unwrap(),
            self.file.clone(),
            self.image,
            self.html,
            self.restrict_author.clone(),
        );
        ByFile::new(byfile_args).process()?;
        Ok(())
    }
}

#[derive(Parser)]
pub struct EffortCommand {
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
}

impl EffortCommand {
    fn execute(&self) -> Result<()> {
        let effort_args = EffortArgs::new(
            String::from("."),
            self.start_date,
            self.end_date,
            self.table,
            self.include.clone(),
            self.exclude.clone(),
            None,
        );
        Effort::new(effort_args).process()?;
        Ok(())
    }
} 