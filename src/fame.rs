use chrono::{Date, Local};
use futures::future::join_all;
use git2::{BlameOptions, Error, Repository, StatusOptions};
use glob::Pattern;
use indicatif::ProgressBar;
use prettytable::{cell, format, row, Table};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
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

pub fn process_fame(args: FameArgs) -> Result<(), Error> {
    let repo_path: String = args.path.clone();

    let file_names = generate_file_list(repo_path.as_ref(), args.include, args.exclude)?;

    let per_file: HashMap<String, Vec<BlameOutput>> = HashMap::new();
    let arc_per_file = Arc::new(RwLock::new(per_file));

    let start_date = args.start_date;
    let end_date = args.end_date;

    let pgb = ProgressBar::new(file_names.len() as u64);
    let arc_pgb = Arc::new(RwLock::new(pgb));

    let mut rt = runtime::Builder::new()
        .threaded_scheduler()
        .thread_name("grit-thread-runner")
        .build()
        .expect("Failed to create threadpool.");

    let rpa = Arc::new(repo_path);

    let mut tasks: Vec<JoinHandle<Result<Vec<BlameOutput>, ()>>> = vec![];

    for file_name in file_names {
        let fne = file_name.clone();
        let rp = rpa.clone();
        let fne = fne.clone();
        let arc_pgb_c = arc_pgb.clone();
        let arc_per_file_c = arc_per_file.clone();
        tasks.push(rt.spawn(async move {
            process_file(&rp.clone(), &fne, start_date, end_date)
                .await
                .map(|pr| {
                    arc_per_file_c
                        .write()
                        .unwrap()
                        .insert(fne.to_string(), (*pr).to_vec());
                    arc_pgb_c.write().unwrap().inc(1);
                    pr
                })
                .map_err(|err| {
                    panic!(err);
                })
        }));
    }

    rt.block_on(join_all(tasks));
    arc_pgb.read().unwrap().finish();

    let max_files = arc_per_file.read().unwrap().keys().len();
    let mut max_lines = 0;

    let mut output_map: HashMap<String, FameOutputLine> = HashMap::new();
    let mut total_commits: Vec<String> = Vec::new();

    for (key, value) in arc_per_file.read().unwrap().iter() {
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

fn generate_file_list(
    path: &str,
    include: Option<String>,
    exclude: Option<String>,
) -> Result<Vec<String>, Error> {
    let repo = Repository::open(path)?;

    let mut status_opts = StatusOptions::new();
    status_opts.include_untracked(false);
    status_opts.include_unmodified(true);
    status_opts.include_ignored(false);
    status_opts.include_unreadable(false);
    status_opts.exclude_submodules(true);

    let statuses = repo.statuses(Some(&mut status_opts))?;

    let includes: Vec<Pattern> = match include {
        Some(e) => e.split(',').map(|s| Pattern::new(s).unwrap()).collect(),
        None => Vec::new(),
    };

    let excludes: Vec<Pattern> = match exclude {
        Some(e) => e.split(',').map(|s| Pattern::new(s).unwrap()).collect(),
        None => Vec::new(),
    };

    let file_names: Vec<String> = statuses
        .iter()
        .filter_map(|se| {
            let s = se.path().unwrap().to_string();
            let exclude_s = s.clone();
            let mut result = None;

            if includes.is_empty() {
                info!("including {} to the file list", s);
                result = Some(s);
            } else {
                for p in &includes {
                    if p.matches(&s) {
                        result = Some(se.path().unwrap().to_string());
                        break;
                    };
                }
            }

            if !excludes.is_empty() && result.is_some() {
                for p in &excludes {
                    if p.matches(&exclude_s) {
                        result = None;
                        info!("removing {} from the file list", exclude_s);
                        break;
                    }
                }
            }

            result
        })
        .collect();

    Ok(file_names)
}

fn pretty_print_table(
    output: Vec<FameOutputLine>,
    tot_loc: usize,
    tot_files: usize,
    tot_commits: usize,
) -> Result<(), Error> {
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

async fn process_file(
    repo_path: &str,
    file_name: &str,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
) -> Result<Vec<BlameOutput>, Error> {
    let repo = Repository::open(repo_path)?;
    let mut bo = BlameOptions::new();
    let path = Path::new(file_name);
    let blame = repo.blame_file(path, Some(&mut bo))?;

    let mut blame_map: HashMap<BlameOutput, usize> = HashMap::new();

    let start_date_sec = match start_date {
        Some(d) => Some(d.naive_local().and_hms(0, 0, 0).timestamp()),
        None => None,
    };

    let end_date_sec = match end_date {
        Some(d) => Some(d.naive_local().and_hms(23, 59, 59).timestamp()),
        None => None,
    };

    for hunk in blame.iter() {
        if let Some(d) = start_date_sec {
            let commit = repo.find_commit(hunk.final_commit_id())?;
            if d > commit.time().seconds() {
                continue;
            }
        }

        if let Some(d) = end_date_sec {
            let commit = repo.find_commit(hunk.final_commit_id())?;
            if d < commit.time().seconds() {
                continue;
            }
        }

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
    use std::time::Instant;
    use tempfile::TempDir;

    // #[test]
    // fn test_process_file() {
    //     simple_logger::init_with_level(Level::Info).unwrap_or(());
    //
    //     let td: TempDir = crate::grit_test::init_repo();
    //     let td_arc = Arc::new(td);
    //
    //     let rt = runtime::Builder::new().build().unwrap();
    //
    //     let start = Instant::now();
    //     let mut result: Vec<BlameOutput> = Vec::new();
    //
    //     rt.spawn(async move {
    //         let path = td_arc.path().to_str().unwrap();
    //         result = try_join!(process_file(path, "README.md", None, None))
    //             .unwrap()
    //             .0;
    //         assert!(
    //             result.len() >= 9,
    //             "test_process_file result was {}",
    //             result.len()
    //         );
    //     });
    //
    //     let duration = start.elapsed();
    //
    //     println!("completed test_process_file in {:?}", duration);
    // }

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

        let dt_local = Local::now();

        let utc_dt = NaiveDate::parse_from_str("2020-03-26", "%Y-%m-%d").unwrap();

        let ed = dt_local
            .timezone()
            .from_local_date(&utc_dt)
            .single()
            .unwrap();

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

        let dt_local = Local::now();

        let utc_dt = NaiveDate::parse_from_str("2020-03-26", "%Y-%m-%d").unwrap();

        let ed = dt_local
            .timezone()
            .from_local_date(&utc_dt)
            .single()
            .unwrap();

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

    #[test]
    fn test_generate_file_list_all() {
        let result = generate_file_list(".", None, None).unwrap();

        assert!(
            result.len() >= 6,
            "test_generate_file_list_all was {}",
            result.len()
        );
    }

    #[test]
    fn test_generate_file_list_rust() {
        let result = generate_file_list(".", Some("*.rs".to_string()), None).unwrap();

        assert!(
            result.len() == 4,
            "test_generate_file_list_all was {}",
            result.len()
        );
    }

    #[test]
    fn test_generate_file_list_exclude_rust() {
        let result = generate_file_list(".", None, Some("*.rs".to_string())).unwrap();

        assert!(
            result.len() >= 3,
            "test_generate_file_list_exclude_rust was {}",
            result.len()
        );
    }
}
