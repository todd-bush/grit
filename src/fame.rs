use super::Processable;
use crate::utils::grit_utils;
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use csv::Writer;
use futures::future::join_all;
use git2::{BlameOptions, Oid, Repository};
use indicatif::ProgressBar;
use prettytable::{Table, format, row};
use std::boxed::Box;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Configuration for the Fame analysis
#[derive(Debug)]
pub struct FameArgs {
    path: String,
    sort: Option<String>,
    start_date: Option<DateTime<Local>>,
    end_date: Option<DateTime<Local>>,
    include: Option<String>,
    exclude: Option<String>,
    restrict_authors: Option<String>,
    csv: bool,
    file: Option<String>,
}

impl FameArgs {
    pub fn new(
        path: String,
        sort: Option<String>,
        start_date: Option<DateTime<Local>>,
        end_date: Option<DateTime<Local>>,
        include: Option<String>,
        exclude: Option<String>,
        restrict_authors: Option<String>,
        csv: bool,
        file: Option<String>,
    ) -> Self {
        Self {
            path,
            sort,
            start_date,
            end_date,
            include,
            exclude,
            restrict_authors,
            csv,
            file,
        }
    }
}

/// Represents a single blame entry for a file
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct BlameEntry {
    author: String,
    commit_id: String,
    lines: i32,
    file_name: String,
}

impl BlameEntry {
    fn new(author: String, commit_id: String, file_name: String) -> Self {
        Self {
            author,
            commit_id,
            lines: 0,
            file_name,
        }
    }
}

/// Represents the final output for an author
#[derive(Clone)]
struct AuthorStats {
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

impl AuthorStats {
    fn new() -> Self {
        Self {
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

/// Processes git blame for a single file
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
    ) -> Self {
        Self {
            path,
            earliest_commit,
            latest_commit,
        }
    }

    async fn process(&self, file_name: String) -> Result<Vec<BlameEntry>> {
        let repo = Repository::open(&self.path)
            .with_context(|| format!("Failed to open repository at {}", self.path))?;
        
        let file_path = Path::new(&file_name);
        let start = Instant::now();

        let mut options = BlameOptions::new();
        
        if let Some(ev) = &self.earliest_commit {
            let oid = Oid::from_bytes(ev)?;
            options.oldest_commit(oid);
        }

        if let Some(ov) = &self.latest_commit {
            let oid = Oid::from_bytes(ov)?;
            options.newest_commit(oid);
        }

        let blame = repo.blame_file(file_path, Some(&mut options))?;
        let mut blame_map: HashMap<String, BlameEntry> = HashMap::new();

        for hunk in blame.iter() {
            let sig = hunk.final_signature();
            let author = String::from_utf8_lossy(sig.name_bytes()).to_string();
            let commit_id = hunk.final_commit_id().to_string();
            let blame_key = format!("{}-{}", author, commit_id);

            let entry = match blame_map.entry(blame_key) {
                Vacant(entry) => entry.insert(BlameEntry::new(author, commit_id, file_name.clone())),
                Occupied(entry) => entry.into_mut(),
            };

            entry.lines += hunk.lines_in_hunk() as i32;
        }

        let result: Vec<BlameEntry> = blame_map.into_values().collect();
        info!("Processed {} in {:?}", file_name, start.elapsed());

        Ok(result)
    }
}

/// Main Fame analysis struct
pub struct Fame {
    args: FameArgs,
}

impl Fame {
    pub fn new(args: FameArgs) -> Self {
        Self { args }
    }

    fn print_table(
        &self,
        output: Vec<AuthorStats>,
        total_lines: i32,
        total_files: usize,
        total_commits: usize,
    ) -> Result<()> {
        println!("Stats on Repo");
        println!("Total files: {}", total_files);
        println!("Total commits: {}", total_commits);
        println!("Total LOC: {}", total_lines);

        let mut table = Table::new();
        table.set_titles(row!["Author", "Files", "Commits", "LOC", "Distribution (%)"]);

        for stats in output.iter() {
            let files_pct = format!("{:.1}", stats.perc_files * 100.0);
            let commits_pct = format!("{:.1}", stats.perc_commits * 100.0);
            let lines_pct = format!("{:.1}", stats.perc_lines * 100.0);
            let distribution = format!(
                "{files_pct:<5} / {commits_pct:<5} / {lines_pct:<5}",
                files_pct = files_pct,
                commits_pct = commits_pct,
                lines_pct = lines_pct
            );

            table.add_row(row![
                stats.author,
                stats.file_count,
                stats.commits_count,
                stats.lines,
                distribution
            ]);
        }

        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
        table.printstd();

        Ok(())
    }

    fn write_csv(&self, output: Vec<AuthorStats>, file_name: Option<String>) -> Result<()> {
        let writer: Box<dyn Write> = match file_name {
            Some(f) => Box::new(File::create(f)?),
            None => Box::new(io::stdout()),
        };

        let mut wrt = Writer::from_writer(writer);
        wrt.write_record(&[
            "Author",
            "Files",
            "Commits",
            "LOC",
            "Distribution (%) - Files",
            "Distribution (%) - Commits",
            "Distribution (%) - LoC",
        ])?;

        for stats in output.iter() {
            wrt.serialize([
                stats.author.clone(),
                stats.file_count.to_string(),
                stats.commits_count.to_string(),
                stats.lines.to_string(),
                format!("{:.1}", stats.perc_files * 100.0),
                format!("{:.1}", stats.perc_commits * 100.0),
                format!("{:.1}", stats.perc_lines * 100.0),
            ])?;
        }

        wrt.flush()?;
        Ok(())
    }

    async fn process_files(
        &self,
        file_names: Vec<String>,
        earliest_commit: Option<Vec<u8>>,
        latest_commit: Option<Vec<u8>>,
    ) -> Result<Vec<BlameEntry>> {
        let processor = BlameProcessor::new(
            self.args.path.clone(),
            earliest_commit,
            latest_commit,
        );

        let progress = ProgressBar::new(file_names.len() as u64);
        let progress = Arc::new(RwLock::new(progress));

        let mut tasks = Vec::new();

        for file_name in file_names {
            let processor = processor.clone();
            let progress = progress.clone();

            tasks.push(tokio::spawn(async move {
                processor.process(file_name.clone())
                    .await
                    .map(|result| {
                        progress.write().unwrap().inc(1);
                        result
                    })
                    .map_err(|err| {
                        error!("Error processing file {}: {}", file_name, err);
                        err
                    })
            }));
        }

        let results = join_all(tasks).await;
        let blame_entries: Vec<BlameEntry> = results
            .into_iter()
            .filter_map(|r| r.ok())
            .filter_map(|r| r.ok())
            .flatten()
            .collect();

        progress.write().unwrap().finish();
        Ok(blame_entries)
    }

    fn calculate_stats(
        &self,
        blame_entries: Vec<BlameEntry>,
        restrict_authors: Option<Vec<String>>,
    ) -> (Vec<AuthorStats>, i32, usize, usize) {
        let mut author_stats: HashMap<String, AuthorStats> = HashMap::new();
        let mut total_commits = HashSet::new();
        let mut total_lines = 0;

        for entry in blame_entries {
            if let Some(ra) = &restrict_authors {
                if ra.contains(&entry.author) {
                    continue;
                }
            }

            let stats = match author_stats.entry(entry.author.clone()) {
                Vacant(e) => e.insert(AuthorStats::new()),
                Occupied(e) => e.into_mut(),
            };

            stats.commits.insert(entry.commit_id.clone());
            total_commits.insert(entry.commit_id);
            stats.filenames.insert(entry.file_name);
            stats.lines += entry.lines;
            total_lines += entry.lines;
        }

        let total_files = author_stats.values()
            .map(|s| s.filenames.len())
            .sum();

        let mut output: Vec<AuthorStats> = author_stats
            .into_iter()
            .map(|(author, mut stats)| {
                stats.author = author;
                stats.commits_count = stats.commits.len() as i32;
                stats.file_count = stats.filenames.len();
                stats.perc_files = stats.file_count as f64 / total_files as f64;
                stats.perc_commits = stats.commits_count as f64 / total_commits.len() as f64;
                stats.perc_lines = stats.lines as f64 / total_lines as f64;
                stats
            })
            .collect();

        match self.args.sort.as_deref() {
            Some("loc") => output.sort_by(|a, b| b.lines.cmp(&a.lines)),
            Some("files") => output.sort_by(|a, b| b.file_count.cmp(&a.file_count)),
            _ => output.sort_by(|a, b| b.commits_count.cmp(&a.commits_count)),
        }

        (output, total_lines, total_files, total_commits.len())
    }
}

impl Processable<()> for Fame {
    fn process(&self) -> Result<()> {
        let (earliest_commit, latest_commit) = grit_utils::find_commit_range(
            &self.args.path,
            self.args.start_date,
            self.args.end_date,
        )?;

        info!("Commit range: {:?} to {:?}", earliest_commit, latest_commit);

        let restrict_authors = grit_utils::convert_string_list_to_vec(self.args.restrict_authors.clone());
        let file_names = grit_utils::generate_file_list(
            &self.args.path,
            self.args.include.clone(),
            self.args.exclude.clone(),
        )?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .context("Failed to create tokio runtime")?;

        let blame_entries = rt.block_on(self.process_files(
            file_names,
            earliest_commit,
            latest_commit,
        ))?;

        let (output, total_lines, total_files, total_commits) = 
            self.calculate_stats(blame_entries, restrict_authors);

        if self.args.csv {
            self.write_csv(output, self.args.file.clone())?;
        } else {
            self.print_table(output, total_lines, total_files, total_commits)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate, TimeZone};
    use log::LevelFilter;
    use std::ops::Add;
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
            false,
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
        let naive_dt = utc_dt.and_hms_opt(0, 0, 0).unwrap();
        let ed = Local.from_local_datetime(&naive_dt).unwrap();

        let args = FameArgs::new(
            path.to_string(),
            Some("loc".to_string()),
            Some(ed),
            None,
            None,
            None,
            None,
            false,
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

        let ed = Local::now().add(Duration::days(-30));

        let args = FameArgs::new(
            path.to_string(),
            Some("loc".to_string()),
            None,
            Some(ed),
            None,
            None,
            None,
            true,
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
            true,
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
            false,
            None,
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
