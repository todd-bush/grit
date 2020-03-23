#[macro_use]
use git2::{BlameOptions, Error, Repository, StatusOptions};
use indicatif::ProgressBar;
use prettytable::{cell, format, row, Table};
use scoped_threadpool::Pool;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::path::Path;
use std::str;
use std::sync::{Arc, RwLock};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct BlameOutputFile {
    author: String,
    commit_id: String,
    lines: usize,
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
struct FameOutputLine {
    author: String,
    lines: usize,
    files_count: usize,
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
            files_count: 0,
            filenames: Vec::new(),
            commits_count: 0,
            perc_files: 0.0,
            perc_lines: 0.0,
            perc_commits: 0.0,
        }
    }
}

fn pretty_print_table(output: Vec<FameOutputLine>) -> Result<(), Error> {
    let mut table = Table::new();

    table.set_titles(row![
        "Author",
        "Files",
        "Commits",
        "LOC",
        "Distribution (%)"
    ]);

    output.iter().for_each(|o| {
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
        table.add_row(row![o.author, o.files_count, o.commits_count, o.lines, s]);
    });

    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.printstd();

    Ok(())
}

fn process_blame(repo_path: &str, path: Arc<&Path>) -> Result<Vec<BlameOutputFile>, Error> {
    let repo = Repository::open(repo_path)?;
    let mut bo = BlameOptions::new();
    let blame = repo.blame_file(path.as_ref(), Some(&mut bo)).unwrap();
    let file_name = path.file_name().unwrap().to_str().unwrap();

    let mut blame_map: HashMap<BlameOutputFile, usize> = HashMap::new();

    info!("process_blame begin loop {}", file_name);
    let start = Instant::now();

    blame.iter().for_each(|hunk| {
        let sig = hunk.final_signature();
        let signame = String::from_utf8_lossy(sig.name_bytes()).to_string();
        let file_blame = BlameOutputFile {
            author: signame,
            commit_id: hunk.final_commit_id().to_string(),
            lines: 0,
        };

        let v = match blame_map.entry(file_blame) {
            Vacant(entry) => entry.insert(0),
            Occupied(entry) => entry.into_mut(),
        };

        *v += hunk.lines_in_hunk();
    });

    let result: Vec<BlameOutputFile> = blame_map
        .iter()
        .map(|(key, val)| {
            let mut k = key.clone();
            let v = *val;
            k.lines = v;
            k
        })
        .collect();

    let duration = start.elapsed();
    info!("process_blame end loop {} in {:?}", file_name, duration);

    Ok(result)
}

pub fn process_repo(
    repo_path: &str,
    _branch: &str,
    sort: Option<String>,
    threads: usize,
) -> Result<(), Error> {
    let repo = Repository::open(repo_path)?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(false);
    opts.include_unmodified(true);
    opts.include_ignored(false);
    opts.include_unreadable(false);
    opts.exclude_submodules(true);
    let statuses = repo.statuses(Some(&mut opts))?;
    let arc_statuses = Arc::new(statuses);
    let arc_statuses_clone = arc_statuses.clone();

    let per_file_hm: HashMap<String, Vec<BlameOutputFile>> = HashMap::new();
    let arc_per_file_hm = Arc::new(RwLock::new(per_file_hm));

    let pb = ProgressBar::new(arc_statuses.len() as u64);
    let arc_pb = Arc::new(RwLock::new(pb));

    let mut pool = Pool::new(threads as u32);

    pool.scoped(|scoped| {
        for se in arc_statuses_clone.iter() {
            let file_name = se.path().unwrap();
            let arc_file_name = Arc::new(file_name.to_owned());
            let hm = arc_per_file_hm.clone();
            let apb = arc_pb.clone();

            scoped.execute(move || {
                let path = Path::new(arc_file_name.as_ref());

                let arc_path = Arc::new(path).clone();

                let arc_path_c = arc_path.clone();
                let arc_file_name_c = arc_file_name.clone();

                let start = Instant::now();

                info!("start processing {}", arc_file_name_c.to_string());
                let stack = process_blame(repo_path, arc_path_c);

                hm.write()
                    .unwrap()
                    .insert(arc_file_name_c.to_string(), stack.unwrap());
                drop(hm);

                apb.write().unwrap().inc(1);
                drop(apb);

                let duration = start.elapsed();
                info!(
                    "completed processing {} in {:?}",
                    arc_file_name_c.to_string(),
                    duration
                );
            });
        }
    });

    arc_pb.write().unwrap().finish();
    let max_files = arc_per_file_hm.read().unwrap().keys().len();
    let mut max_commits = 0;
    let mut max_lines = 0;

    let mut output_map: HashMap<String, FameOutputLine> = HashMap::new();

    for (k, value) in arc_per_file_hm.read().unwrap().iter() {
        for (_, val) in value.iter().enumerate() {
            let om = match output_map.entry(val.author.clone()) {
                Vacant(entry) => entry.insert(FameOutputLine::new()),
                Occupied(entry) => entry.into_mut(),
            };
            om.commits.push(val.commit_id.clone());
            om.filenames.push(k.to_string());
            om.lines += val.lines;
            max_lines += val.lines;
            max_commits += 1;
        }
    }

    info!(
        "Max files/commits/lines: {} {} {}",
        max_files, max_commits, max_lines
    );

    let mut output: Vec<FameOutputLine> = output_map
        .iter_mut()
        .map(|(key, val)| {
            val.commits.dedup();
            val.commits_count = val.commits.len();
            val.filenames.dedup();
            val.files_count = val.filenames.len();
            val.author = key.to_string();
            val.perc_files = (val.files_count) as f64 / (max_files) as f64;
            val.perc_commits = (val.commits_count) as f64 / (max_commits) as f64;
            val.perc_lines = (val.lines) as f64 / (max_lines) as f64;
            val.clone()
        })
        .collect();

    match sort {
        Some(ref x) if x == "loc" => output.sort_by(|a, b| b.lines.cmp(&a.lines)),
        Some(ref x) if x == "files" => output.sort_by(|a, b| b.files_count.cmp(&a.files_count)),
        _ => output.sort_by(|a, b| b.commits_count.cmp(&a.commits_count)),
    };

    pretty_print_table(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_THREADS;
    use log::Level;

    #[test]
    fn test_process_repo_custom_threads() {
        simple_logger::init_with_level(Level::Info).unwrap();

        let start = Instant::now();

        let result = match process_repo(".", "master", Some("loc".to_string()), 5) {
            Ok(()) => true,
            Err(_e) => {
                error!("Error {}", _e);
                false
            }
        };

        let duration = start.elapsed();

        println!("completed test_process_repo_custom_threads {:?}", duration);

        assert!(
            result,
            "test_process_repo_custom_threads result was {}",
            result
        );
    }

    #[test]
    fn test_process_repo_default_threads() {
        let start = Instant::now();

        let result = match process_repo(".", "master", Some("loc".to_string()), DEFAULT_THREADS) {
            Ok(()) => true,
            Err(_e) => false,
        };

        let duration = start.elapsed();

        println!("completed test_process_repo_default_threads {:?}", duration);

        assert!(
            result,
            "test_process_repo_default_threads result was {}",
            result
        );
    }
}
