use super::Processable;
use crate::utils::grit_utils;
use anyhow::Result;
use charts_rs::{LineChart, Series};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Utc, Weekday};
use csv::Writer;
use git2::Repository;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fs::File;
use std::io;
use std::io::Write;
use std::ops::Add;

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
    ) -> ByDateArgs {
        ByDateArgs {
            path: path,
            file: file,
            image: image,
            ignore_weekends: ignore_weekends,
            ignore_gap_fill: ignore_gap_fill,
            html: html,
            restrict_authors: restrict_authors,
        }
    }
}

#[derive(PartialOrd, PartialEq, Clone, Debug)]
struct ByDateOutput {
    date: DateTime<Local>,
    count: f32,
}

impl ByDateOutput {
    fn new(date: DateTime<Local>, count: f32) -> ByDateOutput {
        ByDateOutput { date, count }
    }
}

impl FromIterator<ByDateOutput> for BTreeMap<String, Vec<f32>> {
    fn from_iter<T: IntoIterator<Item = ByDateOutput>>(iter: T) -> Self
    where
        T: IntoIterator<Item = ByDateOutput>,
    {
        let mut map = BTreeMap::new();
        for item in iter {
            map.entry(grit_utils::format_date(item.date.clone()))
                .or_insert_with(Vec::new)
                .push(item.count.clone());
        }
        map
    }
}

pub struct ByDate {
    args: ByDateArgs,
}

impl ByDate {
    pub fn new(args: ByDateArgs) -> ByDate {
        ByDate { args: args }
    }

    fn process_date(&self) -> Result<Vec<ByDateOutput>> {
        let end_date = DateTime::<Utc>::MAX_UTC;
        let start_date = DateTime::<Utc>::MIN_UTC;

        let restrict_authors =
            grit_utils::convert_string_list_to_vec(self.args.restrict_authors.clone());

        let end_date_sec = end_date
            .date_naive()
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc()
            .timestamp();
        let start_date_sec = start_date
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();

        let mut output_map: HashMap<DateTime<Local>, ByDateOutput> = HashMap::new();

        let repo = Repository::open(&self.args.path).expect(format_tostr!(
            "Could not open repo for path {}",
            &self.args.path
        ));

        let mut revwalk = repo.revwalk()?;

        revwalk
            .set_sorting(git2::Sort::NONE | git2::Sort::TIME)
            .expect("Could not sort revwalk");

        revwalk.push_head()?;

        let revwalk = revwalk.filter_map(|id| {
            let id = filter_try!(id);
            let commit = filter_try!(repo.find_commit(id));
            let commit_time = commit.time().seconds();

            if self.args.ignore_weekends && self.is_weekend(commit_time) {
                return None;
            }

            if commit_time < start_date_sec {
                return None;
            }

            if commit_time > end_date_sec {
                return None;
            }

            if let Some(v) = &restrict_authors {
                let name: String = commit.clone().author().name().unwrap().to_string();
                if v.iter().any(|a| a == &name) {
                    return None;
                }
            }

            Some(Ok(commit))
        });

        debug!("filtering completed");

        for commit in revwalk {
            let commit = commit?;
            let commit_time = &commit.time();
            let dt = grit_utils::convert_git_time(commit_time);

            let v = match output_map.entry(dt) {
                Vacant(entry) => entry.insert(ByDateOutput::new(dt, 0.0)),
                Occupied(entry) => entry.into_mut(),
            };

            v.count += 1.0;
        }

        let mut output: Vec<ByDateOutput> = output_map.values().cloned().collect();

        output.sort_by(|a, b| a.date.cmp(&b.date));

        if !&self.args.ignore_gap_fill {
            output = self.fill_date_gaps(output);
        }

        Ok(output)
    }

    fn is_weekend(&self, ts: i64) -> bool {
        let d = Local.from_utc_datetime(&DateTime::from_timestamp(ts, 0).unwrap().naive_utc());
        d.weekday() == Weekday::Sun || d.weekday() == Weekday::Sat
    }

    // Assigns a count of 0 to dates that don't have a commit.
    fn fill_date_gaps(&self, input: Vec<ByDateOutput>) -> Vec<ByDateOutput> {
        let mut processing_date: DateTime<Local> = input[0].date;
        let end_date: DateTime<Local> = input[input.len() - 1].date;
        let mut output = input;
        let mut i = 0;

        debug!("starting at : {:?}", processing_date);

        loop {
            if output[i].date != processing_date {
                output.insert(i, ByDateOutput::new(processing_date, 0.0));
            }

            processing_date = processing_date.add(Duration::days(1));
            i += 1;

            if processing_date > end_date {
                break;
            }
        }

        output
    }

    fn display_text_output(&self, output: Vec<ByDateOutput>) -> Result<()> {
        let w = match &self.args.file {
            Some(f) => {
                let file = File::create(f)?;
                Box::new(file) as Box<dyn Write>
            }
            None => Box::new(io::stdout()) as Box<dyn Write>,
        };

        let mut wtr = Writer::from_writer(w);

        wtr.write_record(&["date", "count"])?;

        let mut total_count = 0.0;

        output.iter().for_each(|r| {
            wtr.serialize((grit_utils::format_date(r.date.clone()), r.count))
                .expect("Cannot seralize table row");

            total_count += r.count;
        });

        wtr.serialize(("Total", total_count))
            .expect("Cannot Seralize Total Count Row");

        wtr.flush().expect("Cannot flush writer");

        Ok(())
    }

    fn create_output_image(&self, output: Vec<ByDateOutput>) -> Result<()> {
        let file = self
            .args
            .file
            .clone()
            .unwrap_or_else(|| String::from("commits.svg"));

        let (width, height) = if output.len() > 60 {
            (1920, 960)
        } else if output.len() > 35 {
            (1280, 960)
        } else {
            (1027, 768)
        };

        let (top, right, bottom, left) = (90, 40, 50, 60);

        let data: Vec<(String, f32)> = output
            .iter()
            .map(|d| (grit_utils::format_date(d.date.clone()), d.count.clone()))
            .collect();
        let mut chart_map: BTreeMap<String, Vec<f32>> = BTreeMap::new();
        for (date, count) in data {
            chart_map.entry(date).or_insert_with(Vec::new).push(count);
        }
        let chart_data: Vec<Series> = chart_map
            .iter()
            .map(|(k, v)| (Series::new(k.clone(), v.clone())))
            .collect();

        let dates: Vec<String> = output
            .iter()
            .map(|d| grit_utils::format_date(d.date))
            .collect();

        let mut line_chart = LineChart::new_with_theme(chart_data, dates, "chaulk");

        line_chart.width = width as f32;
        line_chart.height = height as f32;
        line_chart.margin.top = top as f32;
        line_chart.margin.right = right as f32;
        line_chart.margin.bottom = bottom as f32;
        line_chart.margin.left = left as f32;
        line_chart.title_text = String::from("By Date");

        if self.args.html {
            grit_utils::create_html(&file).expect("Failed to make HTML file.");
        }
        Ok(())
    }
}

impl Processable<()> for ByDate {
    fn process(&self) -> Result<()> {
        let output = self.process_date()?;

        if self.args.image {
            self.create_output_image(output)?;
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
                error!("Error in test_by_date_no_end: {:?}", e);
                false
            }
        };

        println!("completed test_by_date_no_ends in {:?}", start.elapsed());

        assert!(result, "test_by_date_no_ends resut {}", result);
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

        assert!(result, "test_by_date_no_weekends resut {}", result);
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

        assert!(result, "test_by_date_end_date_only resut {}", result);
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

        assert!(result, "test_restrict_author resut {}", result);
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

        assert!(result, "test_by_date_image resut {}", result);
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

        println!("test_is_weekend done in {:?}", duration);

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

        let test_data: Vec<ByDateOutput> = [
            ByDateOutput::new(parse_date("2020-03-13"), 15.0),
            ByDateOutput::new(parse_date("2020-03-16"), 45.0),
        ]
        .to_vec();

        let start = Instant::now();
        let test_out = bd.fill_date_gaps(test_data);
        let duration = start.elapsed();

        println!("test_fill_date_gaps done in {:?}", duration);

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
