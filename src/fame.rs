use git2::{BlameOptions, Error, Repository, StatusOptions};
use indicatif::ProgressBar;
use prettytable::{cell, format, row, Table};
use scoped_threadpool::Pool;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

pub struct FameArgs {
    path: String,
    sort: Option<String>,
    threads: usize,
}

impl FameArgs {
    pub fn new(path: String, sort: Option<String>, threads: usize) -> Self {
        FameArgs {
            path,
            sort,
            threads,
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
    let repo_path: &str = args.path.as_ref();
    let repo = Repository::open(repo_path)?;

    let mut status_opts = StatusOptions::new();
    status_opts.include_untracked(false);
    status_opts.include_unmodified(true);
    status_opts.include_ignored(false);
    status_opts.include_unreadable(false);
    status_opts.exclude_submodules(true);

    let statuses = repo.statuses(Some(&mut status_opts))?;

    let file_names: Vec<String> = statuses
        .iter()
        .map(|se| se.path().unwrap().to_string())
        .collect();

    let per_file: HashMap<String, Vec<BlameOutput>> = HashMap::new();
    let arc_per_file = Arc::new(RwLock::new(per_file));

    let pgb = ProgressBar::new(file_names.len() as u64);
    let arc_pgb = Arc::new(RwLock::new(pgb));

    let mut pool = Pool::new(args.threads as u32);

    pool.scoped(|scoped| {
        for file_name in file_names {
            let fne = Arc::new(file_name);
            let inner_pbg = arc_pgb.clone();
            let inner_per_file = arc_per_file.clone();
            scoped.execute(move || {
                let blame_map = process_file(repo_path, fne.as_ref()).unwrap();
                inner_per_file
                    .write()
                    .unwrap()
                    .insert(fne.to_string(), blame_map);
                inner_pbg.write().unwrap().inc(1);
            });
        }
    });

    arc_pgb.write().unwrap().finish();

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

    pretty_print_table(output)
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

fn process_file(repo_path: &str, file_name: &str) -> Result<Vec<BlameOutput>, Error> {
    let repo = Repository::open(repo_path)?;
    let mut bo = BlameOptions::new();
    let path = Path::new(file_name);
    let blame = repo.blame_file(path, Some(&mut bo))?;

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
    use log::Level;
    use std::time::Instant;

    #[test]
    fn test_process_file() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let start = Instant::now();

        let result = process_file(".", "README.md").unwrap();

        let duration = start.elapsed();

        println!("completed test_process_file in {:?}", duration);

        assert!(
            result.len() >= 9,
            "test_process_file result was {}",
            result.len()
        );
    }

    #[test]
    fn test_process_fame() {
        simple_logger::init_with_level(Level::Info).unwrap_or(());

        let args = FameArgs::new(".".to_string(), Some("loc".to_string()), 15);

        let start = Instant::now();

        let result = match process_fame(args) {
            Ok(()) => true,
            Err(_t) => false,
        };

        let duration = start.elapsed();

        assert!(result, "test_process_file result was {}", result);

        println!("completed test_process_fame in {:?}", duration);
    }
}
