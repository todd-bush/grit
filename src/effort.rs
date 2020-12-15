use super::Processable;
use crate::utils::grit_utils;
use anyhow::Result;
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
use tokio::runtime;
use tokio::task::JoinHandle;

pub struct EffortArgs {
    path: String,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    table: bool,
    include: Option<String>,
    exclude: Option<String>,
    restrict_authors: Option<String>,
}

impl EffortArgs {
    pub fn new(
        path: String,
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        table: bool,
        include: Option<String>,
        exclude: Option<String>,
        restrict_authors: Option<String>,
    ) -> EffortArgs {
        EffortArgs {
            path: path,
            start_date: start_date,
            end_date: end_date,
            table: table,
            include: include,
            exclude: exclude,
            restrict_authors: restrict_authors,
        }
    }
}

#[derive(Clone)]
struct EffortOutput {
    file: String,
    commits: i32,
    active_days: i32,
}

impl EffortOutput {
    pub fn new(file: String) -> EffortOutput {
        EffortOutput {
            file: file,
            commits: 0,
            active_days: 0,
        }
    }
}

#[derive(Clone)]
struct EffortProcessor {
    path: String,
    earliest_commit: Option<Vec<u8>>,
    latest_commit: Option<Vec<u8>>,
    restrict_authors: Option<Vec<String>>,
}

impl EffortProcessor {
    pub fn new(
        path: String,
        earliest_commit: Option<Vec<u8>>,
        latest_commit: Option<Vec<u8>>,
        restrict_authors: Option<Vec<String>>,
    ) -> EffortProcessor {
        EffortProcessor {
            path: path,
            earliest_commit: earliest_commit,
            latest_commit: latest_commit,
            restrict_authors: restrict_authors,
        }
    }

    async fn process_file(&self, file_name: &str) -> Result<EffortOutput> {
        let repo = Repository::open(&self.path)?;
        let mut bo = BlameOptions::new();

        bo.track_copies_any_commit_copies(false);

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

        let mut effort_commits: HashSet<String> = HashSet::new();
        let mut effort_dates: HashSet<Date<Local>> = HashSet::new();

        let file_path = Path::new(file_name);

        let blame = repo.blame_file(file_path, Some(&mut bo))?;

        for hunk in blame.iter() {
            let commit_id = hunk.final_commit_id();
            let commit = repo.find_commit(commit_id)?;
            let commit_date = grit_utils::convert_git_time(&commit.time());

            if let Some(v) = &self.restrict_authors {
                let name: String = commit.clone().author().name().unwrap().to_string();
                if v.iter().any(|a| a == &name) {
                    break;
                }
            }

            effort_commits.insert(commit_id.to_string());
            effort_dates.insert(commit_date);
        }

        let mut result = EffortOutput::new(String::from(file_name));
        result.commits = effort_commits.len() as i32;
        result.active_days = effort_dates.len() as i32;

        Ok(result)
    }
}

pub struct Effort {
    args: EffortArgs,
}

impl Effort {
    pub fn new(args: EffortArgs) -> Effort {
        Effort { args: args }
    }

    fn display_csv(&self, data: Vec<EffortOutput>) -> Result<()> {
        let mut wtr = Writer::from_writer(io::stdout());

        wtr.write_record(&["file", "commits", "active days"])
            .expect("cannot serialize header row");

        data.iter().for_each(|r| {
            wtr.serialize((r.file.clone(), r.commits, r.active_days))
                .expect("Cannot serialize table row");
        });

        wtr.flush().expect("Cannot flush the writer");

        Ok(())
    }

    fn display_table(&self, data: Vec<EffortOutput>) -> Result<()> {
        let mut table = Table::new();

        table.set_titles(row!["File", "Commits", "Active Days"]);

        data.iter().for_each(|r| {
            table.add_row(row![r.file, r.commits, r.active_days]);
        });

        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
        table.printstd();

        Ok(())
    }
}

impl Processable<()> for Effort {
    fn process(&self) -> Result<()> {
        let (earliest_commit, latest_commit) = grit_utils::find_commit_range(
            self.args.path.clone(),
            self.args.start_date,
            self.args.end_date,
        )?;

        let file_names: Vec<String> = grit_utils::generate_file_list(
            &self.args.path,
            self.args.include.clone(),
            self.args.exclude.clone(),
        )?;

        let restrict_authors =
            grit_utils::convert_string_list_to_vec(self.args.restrict_authors.clone());

        let ep = EffortProcessor::new(
            self.args.path.clone(),
            earliest_commit,
            latest_commit,
            restrict_authors,
        );

        let pgb = ProgressBar::new(file_names.len() as u64);
        let arc_pgb = Arc::new(RwLock::new(pgb));

        let mut rt = runtime::Builder::new()
            .threaded_scheduler()
            .thread_name("grit-effort-thread-runner")
            .build()
            .expect("Fail to create threadpool");

        let mut tasks: Vec<JoinHandle<Result<EffortOutput, ()>>> = vec![];

        for file_name in file_names {
            let ep = ep.clone();
            let arc_pgb_c = arc_pgb.clone();
            tasks.push(rt.spawn(async move {
                ep.process_file(&file_name.clone())
                    .await
                    .map(|e| {
                        arc_pgb_c
                            .write()
                            .expect("cannot open ProgressBar to write")
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
            .expect("Cannot open ProgressBar to write")
            .finish();

        let mut results: Vec<EffortOutput> = jh_results
            .into_iter()
            .map(|jh| jh.unwrap().unwrap().clone())
            .collect();

        results.sort_by(|a, b| b.commits.cmp(&a.commits));

        if self.args.table {
            self.display_table(results)
                .expect("Failed to create Effort table");
        } else {
            self.display_csv(results)
                .expect("Failed to create Effort CSV");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::LevelFilter;
    use tempfile::TempDir;

    const LOG_LEVEL: LevelFilter = LevelFilter::Info;

    #[test]
    fn test_effort() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = EffortArgs::new(String::from(path), None, None, false, None, None, None);

        let effort = Effort::new(args);

        let _result = effort.process();
    }

    #[test]
    fn test_effort_include() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();
        let ea = EffortArgs::new(
            path.to_string(),
            None,
            None,
            true,
            Some("*.rs,*.md".to_string()),
            None,
            None,
        );

        let e = Effort::new(ea);

        let _result = e.process();
    }

    #[test]
    fn test_effort_restrict_author() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();
        let ea = EffortArgs::new(
            path.to_string(),
            None,
            None,
            true,
            None,
            None,
            Some(String::from("todd-bush-ln")),
        );

        let e = Effort::new(ea);

        let _result = e.process();
    }
}
