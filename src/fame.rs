use crate::utils::grit_utils;
use chrono::{Date, Local};
use futures::future::join_all;
use git2::{BlameOptions, Oid, Repository};
use indicatif::ProgressBar;
use prettytable::{cell, format, row, Table};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::runtime;
use tokio::task::JoinHandle;

pub struct FameArgs {
    path: String,
    sort: Option<String>,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    include: Option<String>,
    exclude: Option<String>,
}

impl FameArgs {
    pub fn new(
        path: String,
        sort: Option<String>,
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        include: Option<String>,
        exclude: Option<String>,
    ) -> Self {
        FameArgs {
            path,
            sort,
            start_date,
            end_date,
            include,
            exclude,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct BlameOutput {
    author: String,
    commit_id: String,
    lines: usize,
}

impl BlameOutput {
    fn new(author: String, commit_id: String) -> Self {
        BlameOutput {
            author,
            commit_id,
            lines: 0,
        }
    }
}

#[derive(Clone)]
struct FameOutputLine {
    author: String,
    lines: usize,
    file_count: usize,
    filenames: Vec<String>,
    commits: Vec<String>,
    commits_count: usize,
    perc_lines: f64,
    perc_files: f64,
    perc_commits: f64,
}

impl FameOutputLine {
    fn new() -> FameOutputLine {
        FameOutputLine {
            author: String::new(),
            lines: 0,
            commits: Vec::new(),
            file_count: 0,
            filenames: Vec::new(),
            commits_count: 0,
            perc_files: 0.0,
            perc_lines: 0.0,
            perc_commits: 0.0,
        }
    }
}

type GenResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn process_fame(args: FameArgs) -> GenResult<()> {
    let repo_path = args.path.clone();
    let rpc = args.path.clone();

    let rpa = Arc::new(repo_path);

    let file_names: Vec<String> = grit_utils::generate_file_list(&rpc, args.include, args.exclude)?;

    let (earliest_commit, latest_commit) =
        grit_utils::find_commit_range(rpc, args.start_date, args.end_date)?;

    info!("Early, Late: {:?},{:?}", earliest_commit, latest_commit);

    let per_file: HashMap<String, Vec<BlameOutput>> = HashMap::new();
    let arc_per_file = Arc::new(RwLock::new(per_file));

    let pgb = ProgressBar::new(file_names.len() as u64);
    let arc_pgb = Arc::new(RwLock::new(pgb));

    let mut rt = runtime::Builder::new()
        .threaded_scheduler()
        .thread_name("grit-fame-thread-runner")
        .build()
        .expect("Failed to create threadpool.");

    let mut tasks: Vec<JoinHandle<Result<Vec<BlameOutput>, ()>>> = vec![];

    for file_name in file_names {
        let fne = file_name.clone();
        let rp = rpa.clone();
        let fne = fne.clone();
        let arc_pgb_c = arc_pgb.clone();
        let arc_per_file_c = arc_per_file.clone();

        let ec = earliest_commit.clone();
        let lc = latest_commit.clone();

        tasks.push(rt.spawn(async move {
            process_file(&rp.clone(), &fne, ec, lc)
                .await
                .map(|pr| {
                    arc_per_file_c
                        .write()
                        .expect("Cannot write to shared hash map")
                        .insert(fne.to_string(), (*pr).to_vec());
                    arc_pgb_c
                        .write()
                        .expect("Cannot write to shared progress bar")
                        .inc(1);
                    pr
                })
                .map_err(|err| {
                    error!("Error in processing filenames: {}", err);
                })
        }));
    }

    rt.block_on(join_all(tasks));
    arc_pgb
        .read()
        .expect("Cannot read shared progress bar")
        .finish();

    let max_files = arc_per_file
        .read()
        .expect("Cannot read shared hash map")
        .keys()
        .len();
    let mut max_lines = 0;

    let mut output_map: HashMap<String, FameOutputLine> = HashMap::new();
    let mut total_commits: Vec<String> = Vec::new();

    for (key, value) in arc_per_file
        .read()
        .expect("Cannot read shared hash map")
        .iter()
    {
        for val in value.iter() {
            let om = match output_map.entry(val.author.clone()) {
                Vacant(entry) => entry.insert(FameOutputLine::new()),
                Occupied(entry) => entry.into_mut(),
            };
            om.commits.push(val.commit_id.clone());
            total_commits.push(val.commit_id.clone());
            om.filenames.push(key.to_string());
            om.lines += val.lines;
            max_lines += val.lines;
        }
    }

    // TODO - check on total_files
    total_commits.sort();
    total_commits.dedup();
    let max_commits = total_commits.len();

    info!(
        "Max files/commits/lines: {} {} {}",
        max_files, max_commits, max_lines
    );

    let mut output: Vec<FameOutputLine> = output_map
        .iter_mut()
        .map(|(key, val)| {
            val.commits.sort();
            val.commits.dedup();
            val.commits_count = val.commits.len();
            val.filenames.sort();
            val.filenames.dedup();
            val.file_count = val.filenames.len();
            val.author = key.to_string();
            val.perc_files = (val.file_count) as f64 / (max_files) as f64;
            val.perc_commits = (val.commits_count) as f64 / (max_commits) as f64;
            val.perc_lines = (val.lines) as f64 / (max_lines) as f64;
            val.clone()
        })
        .collect();

    match args.sort {
        Some(ref x) if x == "loc" => output.sort_by(|a, b| b.lines.cmp(&a.lines)),
        Some(ref x) if x == "files" => output.sort_by(|a, b| b.file_count.cmp(&a.file_count)),
        _ => output.sort_by(|a, b| b.commits_count.cmp(&a.commits_count)),
    };

    pretty_print_table(output, max_lines, max_files, max_commits)
}

fn pretty_print_table(
    output: Vec<FameOutputLine>,
    tot_loc: usize,
    tot_files: usize,
    tot_commits: usize,
) -> GenResult<()> {
    println!("Stats on Repo");
    println!("Total files: {}", tot_files);
    println!("Total commits: {}", tot_commits);
    println!("Total LOC: {}", tot_loc);

    let mut table = Table::new();

    table.set_titles(row![
        "Author",
        "Files",
        "Commits",
        "LOC",
        "Distribution (%)"
    ]);

    for o in output.iter() {
        let pf = format!("{:.1}", o.perc_files * 100.0);
        let pc = format!("{:.1}", o.perc_commits * 100.0);
        let pl = format!("{:.1}", o.perc_lines * 100.0);
        let s = format!(
            "{pf:<width$} / {pc:<width$} / {pl:<width$}",
            pf = pf,
            pc = pc,
            pl = pl,
            width = 5
        );

        table.add_row(row![o.author, o.file_count, o.commits_count, o.lines, s]);
    }

    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.printstd();

    Ok(())
}

async fn process_file<'a>(
    repo_path: &'a str,
    file_name: &str,
    earliest_commit: Option<Vec<u8>>,
    latest_commit: Option<Vec<u8>>,
) -> GenResult<Vec<BlameOutput>> {
    let repo = Repository::open(repo_path)?;
    let path = Path::new(file_name);
    let start = Instant::now();

    let mut blame_ops = BlameOptions::new();

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

    let blame = repo.blame_file(path, Some(&mut blame_ops))?;

    info!("Blame executed in {:?}", start.elapsed());

    let mut blame_map: HashMap<BlameOutput, usize> = HashMap::new();

    for hunk in blame.iter() {
        let sig = hunk.final_signature();
        let signame = String::from_utf8_lossy(sig.name_bytes()).to_string();
        let file_blame = BlameOutput::new(signame, hunk.final_commit_id().to_string());

        let v = match blame_map.entry(file_blame) {
            Vacant(entry) => entry.insert(0),
            Occupied(entry) => entry.into_mut(),
        };

        *v += hunk.lines_in_hunk();
    }

    let result: Vec<BlameOutput> = blame_map
        .iter()
        .map(|(k, v)| {
            let mut key = k.clone();
            key.lines = *v;
            key
        })
        .collect();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use chrono::TimeZone;
    use log::Level;
    use tempfile::TempDir;

    #[test]
    fn test_process_fame() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = FameArgs::new(
            path.to_string(),
            Some("loc".to_string()),
            None,
            None,
            None,
            None,
        );

        let start = Instant::now();

        let result = match process_fame(args) {
            Ok(()) => true,
            Err(_t) => false,
        };

        let duration = start.elapsed();

        assert!(result, "test_process_file result was {}", result);

        println!("completed test_process_fame in {:?}", duration);
    }

    #[test]
    fn test_process_fame_start_date() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let utc_dt = NaiveDate::parse_from_str("2020-03-26", "%Y-%m-%d").unwrap();

        let ed = Local.from_local_date(&utc_dt).single().unwrap();

        let args = FameArgs::new(
            path.to_string(),
            Some("loc".to_string()),
            Some(ed),
            None,
            None,
            None,
        );

        let start = Instant::now();

        let result = match process_fame(args) {
            Ok(()) => true,
            Err(_t) => false,
        };

        let duration = start.elapsed();

        assert!(result, "test_process_fame_start_date result was {}", result);

        println!("completed test_process_fame_start_date in {:?}", duration);
    }

    #[test]
    fn test_process_fame_end_date() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let utc_dt = NaiveDate::parse_from_str("2020-03-26", "%Y-%m-%d").unwrap();

        let ed = Local.from_local_date(&utc_dt).single().unwrap();

        let args = FameArgs::new(
            path.to_string(),
            Some("loc".to_string()),
            None,
            Some(ed),
            None,
            None,
        );

        let start = Instant::now();

        let result = match process_fame(args) {
            Ok(()) => true,
            Err(_t) => false,
        };

        let duration = start.elapsed();

        assert!(result, "test_process_fame_end_date result was {}", result);

        println!("completed test_process_fame_end_date in {:?}", duration);
    }

    #[test]
    fn test_process_fame_include() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = FameArgs::new(
            path.to_string(),
            Some("loc".to_string()),
            None,
            None,
            Some("*.rs,*.md".to_string()),
            None,
        );

        let start = Instant::now();

        let result = match process_fame(args) {
            Ok(()) => true,
            Err(_t) => false,
        };

        let duration = start.elapsed();

        assert!(result, "test_process_fame_include result was {}", result);

        println!("completed test_process_fame_include in {:?}", duration);
    }
}
