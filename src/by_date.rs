use crate::utils::grit_utils;
use anyhow::Result;
use charts::{
    Chart, LineSeriesView, MarkerType, PointDatum, PointLabelPosition, ScaleBand, ScaleLinear,
};
use chrono::naive::{MAX_DATE, MIN_DATE};
use chrono::offset::{Local, TimeZone};
use chrono::{Date, Datelike, Duration, NaiveDateTime, Weekday};
use csv::Writer;
use git2::Repository;
use std::boxed::Box;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Write;
use std::ops::Add;
use std::path::Path;

#[derive(Ord, Debug, PartialEq, Eq, PartialOrd, Clone)]
struct ByDate {
    date: Date<Local>,
    count: i32,
}

impl ByDate {
    pub fn new(date: Date<Local>, count: i32) -> Self {
        ByDate { date, count }
    }
}

impl PointDatum<String, f32> for ByDate {
    fn get_x(&self) -> String {
        grit_utils::format_date(self.date)
    }

    fn get_y(&self) -> f32 {
        self.count as f32
    }

    fn get_key(&self) -> String {
        String::from("")
    }
}

pub struct ByDateArgs {
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    file: Option<String>,
    image: bool,
    ignore_weekends: bool,
    ignore_gap_fill: bool,
    html: bool,
}

impl ByDateArgs {
    pub fn new(
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        file: Option<String>,
        image: bool,
        ignore_weekends: bool,
        ignore_gap_fill: bool,
        html: bool,
    ) -> Self {
        ByDateArgs {
            start_date,
            end_date,
            file,
            image,
            ignore_weekends,
            ignore_gap_fill,
            html,
        }
    }
}

type GenResult<T> = Result<T>;

pub fn by_date(repo_path: &str, args: ByDateArgs) -> GenResult<()> {
    let output = process_date(
        repo_path,
        args.start_date,
        args.end_date,
        args.ignore_weekends,
        args.ignore_gap_fill,
    )?;

    if args.image {
        match create_output_image(
            output,
            args.file.unwrap_or_else(|| "commits.svg".to_string()),
            args.html,
        ) {
            Ok(_) => {}
            Err(e) => error!("Error thrown while creating image {:?}", e),
        }
    } else {
        match display_output(output, args.file) {
            Ok(_v) => {}
            Err(e) => error!("Error thrown in display_output {:?}", e),
        };
    }

    Ok(())
}

fn process_date(
    repo_path: &str,
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    ignore_weekends: bool,
    ignore_gap_fill: bool,
) -> GenResult<Vec<ByDate>> {
    let end_date = match end_date {
        Some(d) => d,
        None => Local
            .from_local_date(&MAX_DATE)
            .single()
            .expect("Cannot unwrap MAX DATE"),
    };

    let start_date = match start_date {
        Some(d) => d,
        None => Local
            .from_local_date(&MIN_DATE)
            .single()
            .expect("Cannot unwrap MIN DATE"),
    };

    let end_date_sec = end_date.naive_local().and_hms(23, 59, 59).timestamp();
    let start_date_sec = start_date.naive_local().and_hms(0, 0, 0).timestamp();

    let mut output_map: HashMap<Date<Local>, i32> = HashMap::new();

    let repo = Repository::open(repo_path)
        .expect(format_tostr!("Could not open repo for path {}", repo_path));

    let mut revwalk = repo.revwalk()?;
    revwalk
        .set_sorting(git2::Sort::NONE | git2::Sort::TIME)
        .expect("Could not sort revwalk");
    revwalk.push_head()?;

    debug!("filtering revwalk");

    let revwalk = revwalk.filter_map(|id| {
        let id = filter_try!(id);
        debug!("commit id {}", id);
        let commit = filter_try!(repo.find_commit(id));
        let commit_time = commit.time().seconds();

        if ignore_weekends && is_weekend(commit_time) {
            return None;
        }

        if commit_time < start_date_sec {
            return None;
        }

        if commit_time > end_date_sec {
            return None;
        }

        Some(Ok(commit))
    });

    debug!("filtering completed");

    for commit in revwalk {
        let commit = commit?;
        let commit_time = &commit.time();
        let dt = grit_utils::convert_git_time(commit_time);

        let v = match output_map.entry(dt) {
            Vacant(entry) => entry.insert(0),
            Occupied(entry) => entry.into_mut(),
        };
        *v += 1;
    }

    let mut output: Vec<ByDate> = output_map
        .iter()
        .map(|(key, val)| ByDate::new(*key, *val))
        .collect();

    output.sort();

    if !ignore_gap_fill {
        output = fill_date_gaps(output);
    }

    Ok(output)
}

fn fill_date_gaps(input: Vec<ByDate>) -> Vec<ByDate> {
    let mut last_date: Date<Local> = input[0].date;
    let mut output = input;

    let mut i = 0;

    loop {
        if output[i].date != last_date {
            output.insert(i, ByDate::new(last_date, 0));
        }

        last_date = last_date.add(Duration::days(1));
        i += 1;

        if i >= output.len() {
            break;
        }
    }

    output
}

fn is_weekend(ts: i64) -> bool {
    let d = Local.from_utc_datetime(&NaiveDateTime::from_timestamp(ts, 0));

    d.weekday() == Weekday::Sun || d.weekday() == Weekday::Sat
}

fn display_output(output: Vec<ByDate>, file: Option<String>) -> GenResult<()> {
    let w = match file {
        Some(f) => {
            let file = File::create(f)?;
            Box::new(file) as Box<dyn Write>
        }
        None => Box::new(io::stdout()) as Box<dyn Write>,
    };

    let mut wtr = Writer::from_writer(w);

    wtr.write_record(&["date", "count"])?;

    let mut total_count = 0;

    output.iter().for_each(|r| {
        wtr.serialize((grit_utils::format_date(r.date), r.count))
            .expect("Cannot serialize table row");
        total_count += r.count;
    });

    wtr.serialize(("Total", total_count))
        .expect("Cannot serialize Total row");

    wtr.flush()?;

    Ok(())
}

fn create_output_image(output: Vec<ByDate>, file: String, html: bool) -> GenResult<()> {
    let (width, height) = if output.len() > 60 {
        (1920, 960)
    } else if output.len() > 35 {
        (1280, 960)
    } else {
        (1027, 768)
    };

    let (top, right, bottom, left) = (90, 40, 50, 60);

    let dates = output
        .iter()
        .map(|d| grit_utils::format_date(d.date))
        .collect();

    let max_count_obj = output.iter().max_by(|x, y| x.count.cmp(&y.count));

    let max_count = max_count_obj.expect("Cannot access max count object").count as f32 + 5.0;

    let x = ScaleBand::new()
        .set_domain(dates)
        .set_range(vec![0, width - left - right]);

    let y = ScaleLinear::new()
        .set_domain(vec![0_f32, max_count])
        .set_range(vec![height - top - bottom, 0]);

    let line_view = LineSeriesView::new()
        .set_x_scale(&x)
        .set_y_scale(&y)
        .set_marker_type(MarkerType::Circle)
        .set_label_position(PointLabelPosition::NW)
        .set_label_visibility(false) // remove this line to enable point labels, once configurable
        .load_data(&output)
        .expect("Failed to create Line View");

    let _chart = Chart::new()
        .set_width(width)
        .set_height(height)
        .set_margins(top, right, bottom, left)
        .add_title(String::from("By Date"))
        .add_view(&line_view)
        .add_axis_bottom(&x)
        .add_axis_left(&y)
        .add_left_axis_label("Commits")
        .set_bottom_axis_tick_label_rotation(-45)
        .save(Path::new(&file))
        .expect("Failed to create Chart");

    if html {
        grit_utils::create_html(&file).expect("Failed to make HTML file.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use log::Level;
    use std::time::Instant;
    use tempfile::TempDir;

    const LOG_LEVEL: Level = Level::Info;

    #[test]
    fn test_by_date_no_ends() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let start = Instant::now();

        let args = ByDateArgs::new(None, None, None, false, false, false, false);

        let result = match by_date(path, args) {
            Ok(()) => true,
            Err(_e) => false,
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

        let args = ByDateArgs::new(None, None, None, false, true, true, false);

        let result = match by_date(path, args) {
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

        let ed = parse_date("2020-03-26");
        let args = ByDateArgs::new(None, Some(ed), None, false, false, false, false);

        let start = Instant::now();

        let result = match by_date(path, args) {
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
    fn test_by_date_image() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let start = Instant::now();

        let output = process_date(path, None, None, false, true);

        let result = match create_output_image(
            output.unwrap(),
            "target/test_image.svg".to_string(),
            false,
        ) {
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

        let utc_weekday =
            NaiveDateTime::parse_from_str("2020-04-20 0:0", "%Y-%m-%d %H:%M").unwrap();

        let start = Instant::now();
        let weekday = Local.from_local_datetime(&utc_weekday).unwrap();

        let duration = start.elapsed();

        assert!(!is_weekend(weekday.timestamp()), "test_is_weekday");

        println!("test_is_weekend done in {:?}", duration);

        let utc_weekend =
            NaiveDateTime::parse_from_str("2020-04-19 0:0", "%Y-%m-%d %H:%M").unwrap();
        let weekend = Local.from_local_datetime(&utc_weekend).unwrap();

        assert!(is_weekend(weekend.timestamp()), "test_is_weekday");
    }

    #[test]
    fn test_fill_date_gaps() {
        crate::grit_test::set_test_logging(LOG_LEVEL);
        let test_data: Vec<ByDate> = [
            ByDate::new(parse_date("2020-03-13"), 15),
            ByDate::new(parse_date("2020-03-16"), 45),
        ]
        .to_vec();

        let start = Instant::now();
        let test_out = fill_date_gaps(test_data);
        let duration = start.elapsed();

        println!("test_fill_date_gaps done in {:?}", duration);

        assert_eq!(test_out.len(), 4);
        assert_eq!(test_out[2].count, 0);
    }

    fn parse_date(date_str: &str) -> Date<Local> {
        crate::grit_test::set_test_logging(LOG_LEVEL);
        let utc_dt = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").unwrap();

        Local.from_local_date(&utc_dt).single().unwrap()
    }
}
