#[macro_use]
use chrono::naive::{MAX_DATE, MIN_DATE};
use chrono::offset::{Local, TimeZone};
use chrono::{Date, Datelike, Duration, NaiveDateTime, Weekday};
use csv::Writer;
use git2::{Error, Repository, Time};
use plotters::prelude::*;
use std::boxed::Box;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Write;
use std::ops::Add;

#[derive(Ord, Debug, PartialEq, Eq, PartialOrd)]
struct ByDate {
    date: Date<Local>,
    count: i32,
}

impl ByDate {
    pub fn new(date: Date<Local>, count: i32) -> Self {
        ByDate { date, count }
    }
}

pub struct ByDateArgs {
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    file: Option<String>,
    image: bool,
    ignore_weekends: bool,
}

impl ByDateArgs {
    pub fn new(
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        file: Option<String>,
        image: bool,
        ignore_weekends: bool,
    ) -> Self {
        ByDateArgs {
            start_date,
            end_date,
            file,
            image,
            ignore_weekends,
        }
    }
}

pub fn by_date(repo_path: &str, args: ByDateArgs) -> Result<(), Error> {
    let output = process_date(
        repo_path,
        args.start_date,
        args.end_date,
        args.ignore_weekends,
    )?;

    if args.image {
        match create_output_image(
            output,
            args.file.unwrap_or_else(|| "commits.png".to_string()),
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
) -> Result<Vec<ByDate>, Error> {
    let local_now = Local::now();
    let end_date = match end_date {
        Some(d) => d,
        None => local_now
            .timezone()
            .from_local_date(&MAX_DATE)
            .single()
            .unwrap(),
    };

    let start_date = match start_date {
        Some(d) => d,
        None => local_now
            .timezone()
            .from_local_date(&MIN_DATE)
            .single()
            .unwrap(),
    };

    let end_date_sec = end_date.naive_local().and_hms(23, 59, 59).timestamp();
    let start_date_sec = start_date.naive_local().and_hms(0, 0, 0).timestamp();

    let mut output_map: HashMap<Date<Local>, i32> = HashMap::new();

    let repo = Repository::open(repo_path).expect("Could not open repository");

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::NONE | git2::Sort::TIME);
    revwalk.push_head()?;

    macro_rules! filter_try {
        ($e:expr) => {
            match $e {
                Ok(t) => t,
                Err(e) => return Some(Err(e)),
            }
        };
    }

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
        let dt = convert_git_time(commit_time);

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

    let mut last_date: Date<Local> = output[0].date;

    let mut i = 0;

    loop {
        if output[i].date != last_date {
            info!(
                "missing date {}, compare date {}",
                format_date(last_date),
                format_date(output[i].date)
            );
            output.insert(i, ByDate::new(last_date, 0));
        }

        last_date = last_date.add(Duration::days(1));
        i += 1;

        if i >= output.len() {
            break;
        }
    }

    Ok(output)
}

fn convert_git_time(time: &Time) -> Date<Local> {
    let local_now = Local::now();
    local_now
        .timezone()
        .from_utc_datetime(&NaiveDateTime::from_timestamp(time.seconds(), 0))
        .date()
}

fn is_weekend(ts: i64) -> bool {
    let local_now = Local::now();
    let d = local_now
        .timezone()
        .from_utc_datetime(&NaiveDateTime::from_timestamp(ts, 0));

    d.weekday() == Weekday::Sun || d.weekday() == Weekday::Sat
}

fn display_output(
    output: Vec<ByDate>,
    file: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
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
        wtr.serialize((format_date(r.date), r.count)).unwrap();
        total_count += r.count;
    });

    wtr.serialize(("Total", total_count)).unwrap();

    wtr.flush()?;

    Ok(())
}

fn create_output_image(
    output: Vec<ByDate>,
    file: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&file, (1024, 768)).into_drawing_area();
    root.fill(&WHITE)?;

    let (from_date, to_date) = (output[0].date, output.last().unwrap().date);

    let output_count = output.len();

    let max_count_obj = output.iter().max_by(|x, y| x.count.cmp(&y.count));

    let max_count = max_count_obj.unwrap().count as f32 + 5.0;

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(40)
        .margin(5)
        .caption("Commits by Date", ("sans-serif", 32.0).into_font())
        .build_ranged(from_date..to_date, 0f32..max_count)?;

    chart
        .configure_mesh()
        .x_labels(output_count)
        .y_labels(output_count)
        .y_desc("Commits")
        .y_label_formatter(&|y| format!("{}", y))
        .x_label_formatter(&|x| format!("{}", x.format("%Y-%m-%d")))
        .draw()?;

    chart.draw_series(PointSeries::of_element(
        output.iter().map(|db| (db.date, db.count as f32)),
        5,
        ShapeStyle::from(&RED).filled(),
        &|cord, size, style| {
            EmptyElement::at(cord)
                + Circle::new((0, 0), size, style)
                + Text::new(
                    format!("{}", cord.1),
                    (0, -15),
                    ("sans-serif", 12).into_font(),
                )
        },
    ))?;

    chart.draw_series(LineSeries::new(
        output.iter().map(|db| (db.date, db.count as f32)),
        &BLUE,
    ))?;

    Ok(())
}

fn format_date(d: Date<Local>) -> String {
    format!("{}-{:0>2}-{:0>2}", d.year(), d.month(), d.day())
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
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let start = Instant::now();

        let args = ByDateArgs::new(None, None, None, false, false);

        let result = match by_date(path, args) {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!("completed test_by_date_no_ends in {:?}", start.elapsed());

        assert!(result, "test_by_date_no_ends resut {}", result);
    }

    #[test]
    fn test_by_date_no_weekends() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let start = Instant::now();

        let args = ByDateArgs::new(None, None, None, false, true);

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
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let dt_local = Local::now();

        let utc_dt = NaiveDate::parse_from_str("2020-03-26", "%Y-%m-%d").unwrap();

        let ed = dt_local
            .timezone()
            .from_local_date(&utc_dt)
            .single()
            .unwrap();

        let args = ByDateArgs::new(None, Some(ed), None, false, false);

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
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();
        let path = td.path().to_str().unwrap();

        let start = Instant::now();

        let output = process_date(path, None, None, false);

        let result = match create_output_image(output.unwrap(), "test_image.png".to_string()) {
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
    fn test_is_weekday() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let dt_local = Local::now();

        let utc_weekday =
            NaiveDateTime::parse_from_str("2020-04-20 0:0", "%Y-%m-%d %H:%M").unwrap();

        let start = Instant::now();
        let weekday = dt_local
            .timezone()
            .from_local_datetime(&utc_weekday)
            .unwrap();

        let duration = start.elapsed();

        assert!(!is_weekend(weekday.timestamp()), "test_is_weekday");

        println!("test_is_weekday done in {:?}", duration);

        let utc_weekend =
            NaiveDateTime::parse_from_str("2020-04-19 0:0", "%Y-%m-%d %H:%M").unwrap();
        let weekend = dt_local
            .timezone()
            .from_local_datetime(&utc_weekend)
            .unwrap();

        assert!(is_weekend(weekend.timestamp()), "test_is_weekday");
    }
}
