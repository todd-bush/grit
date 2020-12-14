use super::Processable;
use crate::utils::grit_utils;
use anyhow::Result;
use chrono::{Date, Local};
use futures::future::join_all;
use git2::{BlameOptions, Oid, Repository};
use indicatif::ProgressBar;
use prettytable::{cell, format, row, Table};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet};
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
    restrict_authors: Option<String>,
}

impl FameArgs {
    pub fn new(
        path: String,
        sort: Option<String>,
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        include: Option<String>,
        exclude: Option<String>,
        restrict_authors: Option<String>,
    ) -> FameArgs {
        FameArgs {
            path: path,
            sort: sort,
            start_date: start_date,
            end_date: end_date,
            include: include,
            exclude: exclude,
            restrict_authors: restrict_authors,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct BlameOutput {
    author: String,
    commit_id: String,
    lines: i32,
    file_name: String,
}

impl BlameOutput {
    fn new(author: String, commit_id: String) -> BlameOutput {
        BlameOutput {
            author: author,
            commit_id: commit_id,
            lines: 0,
            file_name: String::new(),
        }
    }
}

#[derive(Clone)]
struct FameOutputLine {
    author: String,
    lines: i32,
    file_count: usize,
    filenames: HashSet<String>,
    commits: HashSet<String>,
    commits_count: i32,
    perc_lines: f64,
    perc_files: f64,
    perc_commits: f64,
}

impl FameOutputLine {
    fn new() -> FameOutputLine {
        FameOutputLine {
            author: String::new(),
            lines: 0,
            commits: HashSet::new(),
            file_count: 0,
            filenames: HashSet::new(),
            commits_count: 0,
            perc_files: 0.0,
            perc_lines: 0.0,
            perc_commits: 0.0,
        }
    }
}

pub struct Fame {
    args: FameArgs,
}

#[derive(Clone)]
struct BlameProcessor {
    path: String,
    earliest_commit: Option<Vec<u8>>,
    latest_commit: Option<Vec<u8>>,
}

impl BlameProcessor {
    fn new(
        path: String,
        earliest_commit: Option<Vec<u8>>,
        latest_commit: Option<Vec<u8>>,
    ) -> BlameProcessor {
        BlameProcessor {
            path: path,
            earliest_commit: earliest_commit,
            latest_commit: latest_commit,
        }
    }

    async fn process(&self, file_name: String) -> Result<Vec<BlameOutput>> {
        let repo = Repository::open(&self.path)?;
        let file_path = Path::new(&file_name);

        let mut bo = BlameOptions::new();

        if let Some(ev) = &self.earliest_commit {
            let oid: Oid = Oid::from_bytes(&ev)?;
            let commit = repo.find_commit(oid)?;
            bo.oldest_commit(commit.id());
        };

        if let Some(ov) = &self.latest_commit {
            let oid: Oid = Oid::from_bytes(&ov)?;
            let commit = repo.find_commit(oid)?;
            bo.newest_commit(commit.id());
        };

        let blame = repo.blame_file(file_path, Some(&mut bo))?;

        let mut blame_map: HashMap<String, BlameOutput> = HashMap::new();

        for hunk in blame.iter() {
            let sig = hunk.final_signature();
            let signame = String::from_utf8_lossy(sig.name_bytes()).to_string();
            let f_commit = hunk.final_commit_id().to_string();
            let blame_key = &[&signame, "-", &f_commit].join("");

            let v = match blame_map.entry(blame_key.to_string()) {
                Vacant(entry) => entry.insert(BlameOutput::new(signame, f_commit)),
                Occupied(entry) => entry.into_mut(),
            };

            v.lines += hunk.lines_in_hunk() as i32;
        }

        let result: Vec<BlameOutput> = blame_map.values().cloned().collect();

        Ok(result)
    }
}

impl Fame {
    pub fn new(args: FameArgs) -> Self {
        Fame { args: args }
    }

    fn pretty_print_table(
        &self,
        output: Vec<FameOutputLine>,
        tot_loc: i32,
        tot_files: usize,
        tot_commits: usize,
    ) -> Result<()> {
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
}

impl Processable<()> for Fame {
    fn process(&self) -> Result<()> {
        let (earliest_commit, latest_commit) = grit_utils::find_commit_range(
            self.args.path.clone(),
            self.args.start_date,
            self.args.end_date,
        )?;

        info!("Early, Late: {:?}, {:?}", earliest_commit, latest_commit);

        let restrict_authors: Option<Vec<String>> =
            grit_utils::convert_string_list_to_vec(self.args.restrict_authors.clone());

        let file_names: Vec<String> = grit_utils::generate_file_list(
            &self.args.path,
            self.args.include.clone(),
            self.args.exclude.clone(),
        )?;

        let bp = BlameProcessor::new(
            self.args.path.clone(),
            earliest_commit.clone(),
            latest_commit.clone(),
        );

        let pgb = ProgressBar::new(file_names.len() as u64);
        let arc_pgb = Arc::new(RwLock::new(pgb));

        let mut rt = runtime::Builder::new()
            .threaded_scheduler()
            .thread_name("grit-fame-thread-runner")
            .build()
            .expect("Failed to create threadpool.");

        let mut tasks: Vec<JoinHandle<Result<Vec<BlameOutput>, ()>>> = vec![];

        for file_name in file_names.iter() {
            let file_name = file_name.clone();
            let bp = bp.clone();
            let arc_pgb_c = arc_pgb.clone();

            info!("processing file {}", file_name);
            tasks.push(rt.spawn(async move {
                bp.process(String::from(file_name.clone()))
                    .await
                    .map(|mut pr| {
                        pr.iter_mut().for_each(|p| p.file_name = file_name.clone());
                        &arc_pgb_c
                            .write()
                            .expect("cannot open progress bar for write")
                            .inc(1);
                        pr
                    })
                    .map_err(|err| error!("Error in processing file: {}", err))
            }));
        }

        let jh_results = rt.block_on(join_all(tasks));

        arc_pgb
            .write()
            .expect("cannot open progress bar for write")
            .finish();

        let collector: Vec<Vec<BlameOutput>> = jh_results
            .into_iter()
            .map(|jh| jh.unwrap().unwrap().clone())
            .collect();

        let max_files = collector.len();

        let blame_outputs: Vec<BlameOutput> = collector.into_iter().flatten().collect();

        let mut max_lines = 0;
        let mut output_map: HashMap<String, FameOutputLine> = HashMap::new();
        let mut total_commits: HashSet<String> = HashSet::new();

        for v in blame_outputs.iter() {
            if let Some(ra) = &restrict_authors {
                if ra.contains(&v.author) {
                    break;
                }
            }

            let om = match output_map.entry(v.author.clone()) {
                Vacant(entry) => entry.insert(FameOutputLine::new()),
                Occupied(entry) => entry.into_mut(),
            };

            om.commits.insert(v.commit_id.clone());
            total_commits.insert(v.commit_id.clone());
            om.filenames.insert(v.file_name.clone());
            om.lines += v.lines;
            max_lines += v.lines;
        }

        let max_commits = total_commits.len();

        info!(
            "Max files/commits/lines: {} {} {}",
            max_files, max_commits, max_lines
        );

        let mut output: Vec<FameOutputLine> = output_map
            .iter_mut()
            .map(|(key, val)| {
                val.commits_count = val.commits.len() as i32;
                val.file_count = val.filenames.len();
                val.author = String::from(key);
                val.perc_files = (val.file_count) as f64 / (max_files) as f64;
                val.perc_commits = (val.commits_count) as f64 / (max_commits) as f64;
                val.perc_lines = (val.lines) as f64 / (max_lines) as f64;
                val.clone()
            })
            .collect();

        match self.args.sort {
            Some(ref x) if x == "loc" => output.sort_by(|a, b| b.lines.cmp(&a.lines)),
            Some(ref x) if x == "files" => output.sort_by(|a, b| b.file_count.cmp(&a.file_count)),
            _ => output.sort_by(|a, b| b.commits_count.cmp(&a.commits_count)),
        }

        self.pretty_print_table(output, max_lines, max_files, max_commits)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate, TimeZone};
    use log::LevelFilter;
    use std::ops::Add;
    use std::time::Instant;
    use tempfile::TempDir;

    const LOG_LEVEL: LevelFilter = LevelFilter::Info;

    #[test]
    fn test_process_fame() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = FameArgs::new(
            String::from(path),
            Some("loc".to_string()),
            None,
            None,
            None,
            None,
            None,
        );

        let f = Fame::new(args);

        let result = match f.process() {
            Ok(()) => true,
            Err(_t) => false,
        };

        assert!(result, "test_process_file result was {}", result);
    }

    #[test]
    fn test_process_fame_start_date() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

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
            None,
        );

        let fame = Fame::new(args);

        let start = Instant::now();

        let result = match fame.process() {
            Ok(()) => true,
            Err(_t) => false,
        };

        let duration = start.elapsed();

        assert!(result, "test_process_fame_start_date result was {}", result);

        println!("completed test_process_fame_start_date in {:?}", duration);
    }

    #[test]
    fn test_process_fame_end_date() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let ed = Local::now().add(Duration::days(-30)).date();

        let args = FameArgs::new(
            path.to_string(),
            Some("loc".to_string()),
            None,
            Some(ed),
            None,
            None,
            None,
        );

        let fame = Fame::new(args);

        let start = Instant::now();

        let result = match fame.process() {
            Ok(()) => true,
            Err(_t) => false,
        };

        let duration = start.elapsed();

        assert!(result, "test_process_fame_end_date result was {}", result);

        println!("completed test_process_fame_end_date in {:?}", duration);
    }

    #[test]
    fn test_process_fame_include() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = FameArgs::new(
            path.to_string(),
            Some("loc".to_string()),
            None,
            None,
            Some("*.rs,*.md".to_string()),
            None,
            None,
        );

        let fame = Fame::new(args);

        let start = Instant::now();

        let result = match fame.process() {
            Ok(()) => true,
            Err(_t) => false,
        };

        let duration = start.elapsed();

        assert!(result, "test_process_fame_include result was {}", result);

        println!("completed test_process_fame_include in {:?}", duration);
    }

    #[test]
    fn test_process_fame_restrict_author() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = FameArgs::new(
            path.to_string(),
            Some("loc".to_string()),
            None,
            None,
            None,
            None,
            Some(String::from("todd-bush")),
        );

        let fame = Fame::new(args);

        let start = Instant::now();

        let result = match fame.process() {
            Ok(()) => true,
            Err(_t) => false,
        };

        let duration = start.elapsed();

        assert!(
            result,
            "test_process_fame_restrict_author result was {}",
            result
        );

        println!(
            "completed test_process_fame_restrict_author in {:?}",
            duration
        );
    }
}
