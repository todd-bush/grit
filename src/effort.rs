use crate::utils::grit_utils;
use chrono::offset::Local;
use chrono::Date;
use csv::Writer;
use futures::future::join_all;
use git2::{BlameOptions, Oid, Repository};
use indicatif::ProgressBar;
use prettytable::{cell, format, row, Table};
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::runtime;
use tokio::task::JoinHandle;

pub struct EffortArgs {
    path: String,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    table: bool,
}

impl EffortArgs {
    pub fn new(
        path: String,
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        table: bool,
    ) -> Self {
        EffortArgs {
            path,
            start_date,
            end_date,
            table,
        }
    }
}

#[derive(Debug, Clone)]
struct EffortOutput {
    file: String,
    commits: usize,
    active_days: usize,
}

impl EffortOutput {
    pub fn new(file: String) -> Self {
        EffortOutput {
            file,
            commits: 0,
            active_days: 0,
        }
    }
}

type GenResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn effort(args: EffortArgs) -> GenResult<()> {
    let results = process_effort(args.path, args.start_date, args.end_date)?;

    if args.table {
        display_table(results).expect("Failed to create Effort table");
    } else {
        display_csv(results).expect("Failted to create Effort CSV");
    }

    Ok(())
}

fn process_effort(
    repo_path: String,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
) -> GenResult<Vec<EffortOutput>> {
    let rpc = repo_path.clone();

    let (earliest_commit, latest_commit) =
        grit_utils::find_commit_range(repo_path.to_string(), start_date, end_date)?;

    let file_names: Vec<String> = grit_utils::generate_file_list(&repo_path, None, None)?;

    let pgb = ProgressBar::new(file_names.len() as u64);

    let arc_pgb = Arc::new(RwLock::new(pgb));
    let rpa = Arc::new(rpc);

    let mut rt = runtime::Builder::new()
        .threaded_scheduler()
        .thread_name("grit-effort-thread-runner")
        .build()
        .expect("Failed to create threadpool.");

    let mut tasks: Vec<JoinHandle<Result<EffortOutput, ()>>> = vec![];

    for file_name in file_names {
        let rp = rpa.clone();
        let fne = file_name.clone();
        let arc_pgb_c = arc_pgb.clone();

        let ec = earliest_commit.clone();
        let lc = latest_commit.clone();

        tasks.push(rt.spawn(async move {
            process_effort_file(&rp.clone(), &fne, ec, lc)
                .await
                .map(|e| {
                    arc_pgb_c
                        .write()
                        .expect("Cannot write to shared progress bar")
                        .inc(1);

                    e
                })
                .map_err(|err| {
                    error!("Error processing effort: {}", err);
                })
        }));
    }

    let jh_results = rt.block_on(join_all(tasks));

    arc_pgb
        .write()
        .expect("Could not open progress bar to write")
        .finish();

    let mut results: Vec<EffortOutput> = vec![];

    for jh in jh_results.into_iter() {
        let r = jh.unwrap().unwrap().clone();

        results.push(r);
    }

    results.sort_by(|a, b| b.commits.cmp(&a.commits));

    Ok(results)
}

async fn process_effort_file<'a>(
    r_path: &'a str,
    file_name: &str,
    earliest_commit: Option<Vec<u8>>,
    latest_commit: Option<Vec<u8>>,
) -> GenResult<EffortOutput> {
    let mut result = EffortOutput::new(file_name.to_string());

    let path = Path::new(file_name);

    let repo = Repository::open(r_path)?;
    let mut blame_ops = BlameOptions::new();
    let mut effort_commits: HashSet<String> = HashSet::new();
    let mut effort_dates: HashSet<Date<Local>> = HashSet::new();

    blame_ops.track_copies_any_commit_copies(false);

    if let Some(ev) = earliest_commit {
        let oid: Oid = Oid::from_bytes(&ev)?;
        let commit = repo.find_commit(oid)?;
        blame_ops.oldest_commit(commit.id());
    };

    if let Some(ov) = latest_commit {
        let oid: Oid = Oid::from_bytes(&ov)?;
        let commit = repo.find_commit(oid)?;
        blame_ops.newest_commit(commit.id());
    };

    let start = Instant::now();

    let blame = repo.blame_file(path, Some(&mut blame_ops))?;

    info!("Blame executed in {:?}", start.elapsed());

    for hunk in blame.iter() {
        let commit_id = hunk.final_commit_id();
        let commit = repo.find_commit(commit_id)?;
        let commit_date = grit_utils::convert_git_time(&commit.time());

        effort_commits.insert(commit_id.to_string());
        effort_dates.insert(commit_date);
    }

    result.commits = effort_commits.len();
    result.active_days = effort_dates.len();

    Ok(result)
}

fn display_csv(data: Vec<EffortOutput>) -> GenResult<()> {
    let mut wtr = Writer::from_writer(io::stdout());

    wtr.write_record(&["file", "commits", "active days"])?;

    data.iter().for_each(|r| {
        wtr.serialize((r.file.clone(), r.commits, r.active_days))
            .expect("Cannot serialize table row");
    });

    wtr.flush()?;

    Ok(())
}

fn display_table(data: Vec<EffortOutput>) -> GenResult<()> {
    let mut table = Table::new();

    table.set_titles(row!["File", "Commits", "Active Days"]);

    data.iter().for_each(|r| {
        table.add_row(row![r.file, r.commits, r.active_days]);
    });

    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.printstd();

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use log::Level;
    use tempfile::TempDir;

    #[test]
    fn test_process_effort() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let result = process_effort(path.to_string(), None, None);

        info!("results: {:?}", result);
    }

    #[test]
    fn test_effort() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();
        let ea = EffortArgs::new(path.to_string(), None, None, true);

        let _result = effort(ea);
    }
}
