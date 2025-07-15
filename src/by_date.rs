use super::Processable;
use crate::utils::grit_utils;
use anyhow::{Context, Result};
use charts_rs::{LineChart, Series};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Weekday};
use csv::Writer;
use git2::Repository;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io;
use std::io::Write;
use std::ops::Add;

/// Configuration for the ByDate analysis
#[derive(Debug)]
pub struct ByDateArgs {
    path: String,
    file: Option<String>,
    image: bool,
    ignore_weekends: bool,
    ignore_gap_fill: bool,
    html: bool,
    restrict_authors: Option<String>,
}

impl ByDateArgs {
    pub fn new(
        path: String,
        file: Option<String>,
        image: bool,
        ignore_weekends: bool,
        ignore_gap_fill: bool,
        html: bool,
        restrict_authors: Option<String>,
    ) -> Self {
        Self {
            path,
            file,
            image,
            ignore_weekends,
            ignore_gap_fill,
            html,
            restrict_authors,
        }
    }
}

/// Represents a single day's commit count
#[derive(PartialOrd, PartialEq, Clone, Debug)]
struct CommitDay {
    date: DateTime<Local>,
    count: f32,
}

impl CommitDay {
    fn new(date: DateTime<Local>, count: f32) -> Self {
        Self { date, count }
    }
}

/// Converts a collection of CommitDays into a BTreeMap for charting
impl FromIterator<CommitDay> for BTreeMap<String, Vec<f32>> {
    fn from_iter<T: IntoIterator<Item = CommitDay>>(iter: T) -> Self {
        let mut map = BTreeMap::new();
        for day in iter {
            map.entry(grit_utils::format_date(day.date))
                .or_insert_with(Vec::new)
                .push(day.count);
        }
        map
    }
}

/// Main ByDate analysis struct
pub struct ByDate {
    args: ByDateArgs,
}

impl ByDate {
    pub fn new(args: ByDateArgs) -> Self {
        Self { args }
    }

    /// Processes git commits and returns a vector of CommitDays
    fn process_commits(&self) -> Result<Vec<CommitDay>> {
        let repo = Repository::open(&self.args.path)
            .with_context(|| format!("Could not open repo at {}", self.args.path))?;

        let restrict_authors =
            grit_utils::convert_string_list_to_vec(self.args.restrict_authors.clone());
        let mut output_map: HashMap<DateTime<Local>, CommitDay> = HashMap::new();

        let mut revwalk = repo.revwalk()?;
        revwalk.set_sorting(git2::Sort::NONE | git2::Sort::TIME)?;
        revwalk.push_head()?;

        for commit_id in revwalk {
            let commit = repo.find_commit(commit_id?)?;
            let commit_time = commit.time().seconds();

            if self.args.ignore_weekends && self.is_weekend(commit_time) {
                continue;
            }

            if let Some(authors) = &restrict_authors {
                let author_name = commit.author().name().unwrap_or_default().to_string();
                if authors.contains(&author_name) {
                    continue;
                }
            }

            let dt = grit_utils::convert_git_time(&commit.time());
            let entry = match output_map.entry(dt) {
                Vacant(entry) => entry.insert(CommitDay::new(dt, 0.0)),
                Occupied(entry) => entry.into_mut(),
            };
            entry.count += 1.0;
        }

        let mut output: Vec<CommitDay> = output_map.into_values().collect();
        output.sort_by(|a, b| a.date.cmp(&b.date));

        if !self.args.ignore_gap_fill {
            output = self.fill_date_gaps(output);
        }

        Ok(output)
    }

    /// Checks if a timestamp falls on a weekend
    fn is_weekend(&self, ts: i64) -> bool {
        let dt = Local.from_utc_datetime(&DateTime::from_timestamp(ts, 0).unwrap().naive_utc());
        dt.weekday() == Weekday::Sun || dt.weekday() == Weekday::Sat
    }

    /// Fills in missing dates with zero counts
    fn fill_date_gaps(&self, input: Vec<CommitDay>) -> Vec<CommitDay> {
        if input.is_empty() {
            return input;
        }

        let start_date = input[0].date;
        let end_date = input[input.len() - 1].date;
        let mut date_map: HashMap<DateTime<Local>, f32> =
            input.into_iter().map(|day| (day.date, day.count)).collect();

        let mut current_date = start_date;
        while current_date <= end_date {
            date_map.entry(current_date).or_insert(0.0);
            current_date = current_date.add(Duration::days(1));
        }

        let mut output: Vec<CommitDay> = date_map
            .into_iter()
            .map(|(date, count)| CommitDay::new(date, count))
            .collect();
        output.sort_by(|a, b| a.date.cmp(&b.date));
        output
    }

    /// Displays the commit data as text output
    fn display_text_output(&self, output: Vec<CommitDay>) -> Result<()> {
        let writer: Box<dyn Write> = match &self.args.file {
            Some(f) => Box::new(File::create(f)?),
            None => Box::new(io::stdout()),
        };

        let mut wtr = Writer::from_writer(writer);
        wtr.write_record(["date", "count"])?;

        let mut total_count = 0.0;
        for day in output.iter() {
            wtr.serialize((grit_utils::format_date(day.date), day.count))?;
            total_count += day.count;
        }

        wtr.serialize(("Total", total_count))?;
        wtr.flush()?;

        Ok(())
    }

    /// Creates a chart from the commit data
    fn create_chart(&self, output: Vec<CommitDay>) -> Result<()> {
        let file = self
            .args
            .file
            .clone()
            .unwrap_or_else(|| "commits.svg".to_string());
        let (width, height) = self.calculate_chart_dimensions(output.len());
        let margins = (90, 40, 50, 60);

        let dates: Vec<String> = output
            .iter()
            .map(|d| grit_utils::format_date(d.date))
            .collect();

        let chart_data: Vec<Series> = BTreeMap::from_iter(output)
            .iter()
            .map(|(k, v)| Series::new(k.clone(), v.clone()))
            .collect();

        let mut chart = LineChart::new_with_theme(chart_data, dates, "chaulk");
        self.configure_chart(&mut chart, width, height, margins);

        if self.args.html {
            grit_utils::create_html(&file)?;
        }

        Ok(())
    }

    /// Calculates appropriate chart dimensions based on data size
    fn calculate_chart_dimensions(&self, data_points: usize) -> (u32, u32) {
        match data_points {
            n if n > 60 => (1920, 960),
            n if n > 35 => (1280, 960),
            _ => (1027, 768),
        }
    }

    /// Configures chart properties
    fn configure_chart(
        &self,
        chart: &mut LineChart,
        width: u32,
        height: u32,
        margins: (u32, u32, u32, u32),
    ) {
        chart.width = width as f32;
        chart.height = height as f32;
        chart.margin.top = margins.0 as f32;
        chart.margin.right = margins.1 as f32;
        chart.margin.bottom = margins.2 as f32;
        chart.margin.left = margins.3 as f32;
        chart.title_text = "By Date".to_string();
    }
}

impl Processable<()> for ByDate {
    fn process(&self) -> Result<()> {
        let output = self.process_commits()?;

        if self.args.image {
            self.create_chart(output)?;
        } else {
            self.display_text_output(output)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use log::LevelFilter;
    use std::time::Instant;
    use tempfile::TempDir;

    const LOG_LEVEL: LevelFilter = LevelFilter::Info;

    #[test]
    fn test_by_date_no_end() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = ByDateArgs::new(String::from(path), None, false, false, false, false, None);

        let bd = ByDate::new(args);

        let start = Instant::now();

        let result = match bd.process() {
            Ok(()) => true,
            Err(e) => {
                error!("Error in test_by_date_no_end: {e:?}");
                false
            }
        };

        println!("completed test_by_date_no_ends in {:?}", start.elapsed());

        assert!(result, "test_by_date_no_ends resut {result}");
    }

    #[test]
    fn test_by_date_no_weekends() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let start = Instant::now();

        let args = ByDateArgs::new(String::from(path), None, false, true, true, false, None);

        let bd = ByDate::new(args);

        let result = match bd.process() {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!(
            "completed test_by_date_no_weekends in {:?}",
            start.elapsed()
        );

        assert!(result, "test_by_date_no_weekends resut {result}");
    }

    #[test]
    fn test_by_date_end_date_only() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = ByDateArgs::new(String::from(path), None, false, false, false, false, None);

        let bd = ByDate::new(args);

        let start = Instant::now();

        let result = match bd.process() {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!(
            "completed test_by_date_end_date_only in {:?}",
            start.elapsed()
        );

        assert!(result, "test_by_date_end_date_only resut {result}");
    }

    #[test]
    fn test_restrict_author() {
        crate::grit_test::set_test_logging(LOG_LEVEL);
        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let start = Instant::now();

        let args = ByDateArgs::new(
            String::from(path),
            None,
            false,
            false,
            false,
            false,
            Some(String::from("todd-bush-ln")),
        );

        let bd = ByDate::new(args);

        let result = match bd.process() {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!("completed test_restrict_author in {:?}", start.elapsed());

        assert!(result, "test_restrict_author resut {result}");
    }

    #[test]
    fn test_by_date_image() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let args = ByDateArgs::new(String::from(path), None, true, true, true, false, None);

        let start = Instant::now();

        let bd = ByDate::new(args);

        let result = match bd.process() {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!(
            "completed test_by_date_end_date_only_image in {:?}",
            start.elapsed()
        );

        assert!(result, "test_by_date_image resut {result}");
    }

    #[test]
    fn test_is_weekend() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let args = ByDateArgs::new(String::from("path"), None, true, true, true, false, None);

        let bd = ByDate::new(args);

        let utc_weekday =
            NaiveDateTime::parse_from_str("2020-04-20 0:0", "%Y-%m-%d %H:%M").unwrap();

        let start = Instant::now();
        let weekday = Local.from_local_datetime(&utc_weekday).unwrap();

        let duration = start.elapsed();

        assert!(!bd.is_weekend(weekday.timestamp()), "test_is_weekday");

        println!("test_is_weekend done in {duration:?}");

        let utc_weekend =
            NaiveDateTime::parse_from_str("2020-04-19 0:0", "%Y-%m-%d %H:%M").unwrap();
        let weekend = Local.from_local_datetime(&utc_weekend).unwrap();

        assert!(bd.is_weekend(weekend.timestamp()), "test_is_weekday");
    }

    #[test]
    fn test_fill_date_gaps() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let args = ByDateArgs::new(String::from("path"), None, true, true, true, false, None);

        let bd = ByDate::new(args);

        let test_data: Vec<CommitDay> = [
            CommitDay::new(parse_date("2020-03-13"), 15.0),
            CommitDay::new(parse_date("2020-03-16"), 45.0),
        ]
        .to_vec();

        let start = Instant::now();
        let test_out = bd.fill_date_gaps(test_data);
        let duration = start.elapsed();

        println!("test_fill_date_gaps done in {duration:?}");

        assert_eq!(test_out.len(), 4);
        assert_eq!(test_out[2].count, 0.0);
    }

    fn parse_date(date_str: &str) -> DateTime<Local> {
        crate::grit_test::set_test_logging(LOG_LEVEL);
        let naive_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").unwrap();
        let naive_dt = naive_date.and_hms_opt(0, 0, 0).unwrap();
        Local.from_local_datetime(&naive_dt).unwrap()
    }
}
