#[macro_use]
use chrono::DateTime;
use chrono::naive::{MAX_DATE, MIN_DATE};
use chrono::offset::{Local, TimeZone};
use chrono::{Date, Datelike, NaiveDate, NaiveDateTime};
use csv::Writer;
use git2::{Error, Repository, Time};
use plotters::prelude::*;
use std::boxed::Box;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Write;

#[derive(Ord, Debug, PartialEq, Eq, PartialOrd)]
struct ByDate {
    date: String,
    count: i32,
}

impl ByDate {
    pub fn new(date: String, count: i32) -> Self {
        ByDate { date, count }
    }
}

pub struct ByDateArgs {
    start_date: Option<Date<Local>>,
    end_date: Option<Date<Local>>,
    file: Option<String>,
    image: bool,
}

impl ByDateArgs {
    pub fn new(
        start_date: Option<Date<Local>>,
        end_date: Option<Date<Local>>,
        file: Option<String>,
        image: bool,
    ) -> Self {
        ByDateArgs {
            start_date,
            end_date,
            file,
            image,
        }
    }
}

pub fn by_date(repo_path: &str, args: ByDateArgs) -> Result<(), Error> {
    let output = process_date(repo_path, args.start_date, args.end_date)?;

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

    let mut output_map: HashMap<String, i32> = HashMap::new();

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

        if commit_time < start_date.naive_local().and_hms(0, 0, 0).timestamp() {
            return None;
        }

        if commit_time > end_date.naive_local().and_hms(0, 0, 0).timestamp() {
            return None;
        }

        Some(Ok(commit))
    });

    debug!("filtering completed");

    for commit in revwalk {
        let commit = commit?;
        let commit_time = &commit.time();
        let dt = convert_git_time(commit_time);
        let date_string = format!("{}-{:0>2}-{:0>2}", dt.year(), dt.month(), dt.day());

        info!("commit time {}", date_string);

        let v = match output_map.entry(date_string) {
            Vacant(entry) => entry.insert(0),
            Occupied(entry) => entry.into_mut(),
        };
        *v += 1;
    }

    let mut output: Vec<ByDate> = output_map
        .iter()
        .map(|(key, val)| ByDate::new(key.to_string(), *val))
        .collect();

    output.sort();

    Ok(output)
}

fn convert_git_time(time: &Time) -> DateTime<Local> {
    let local_now = Local::now();
    local_now
        .timezone()
        .from_utc_datetime(&NaiveDateTime::from_timestamp(time.seconds(), 0))
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

    output.iter().for_each(|r| {
        wtr.serialize((r.date.to_string(), r.count)).unwrap();
    });

    wtr.flush()?;

    Ok(())
}

fn create_output_image(
    output: Vec<ByDate>,
    file: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&file, (1024, 768)).into_drawing_area();
    root.fill(&WHITE)?;

    let (from_date, to_date) = (
        parse_time(&output[0].date),
        parse_time(&output.last().unwrap().date),
    );

    let max_count_obj = output.iter().max_by(|x, y| x.count.cmp(&y.count));

    let max_count = max_count_obj.unwrap().count as f32 + 5.0;

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(40)
        .margin(5)
        .caption("Commits by Date", ("sans-serif", 50.0).into_font())
        .build_ranged(from_date..to_date, 0f32..max_count)?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_axis()
        .y_desc("Commits")
        .y_label_formatter(&|y| format!("{}", y))
        .draw()?;

    chart.draw_series(LineSeries::new(
        output
            .iter()
            .map(|db| (parse_time(&db.date), db.count as f32)),
        &BLUE,
    ))?;

    Ok(())
}

fn parse_time(t: &str) -> Date<Local> {
    Local
        .datetime_from_str(&format!("{} 0:0", t), "%Y-%m-%d %H:%M")
        .unwrap()
        .date()
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;
    use std::time::Instant;

    const LOG_LEVEL: Level = Level::Warn;

    #[test]
    fn test_by_date_no_ends() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());
        let start = Instant::now();

        let args = ByDateArgs::new(None, None, None, false);

        let _result = match by_date(".", args) {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!("completed test_by_date_no_ends in {:?}", start.elapsed());
    }

    #[test]
    fn test_by_date_end_date_only() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let dt_local = Local::now();

        let utc_dt = NaiveDate::parse_from_str("2020-03-26", "%Y-%m-%d").unwrap();

        let ed = dt_local
            .timezone()
            .from_local_date(&utc_dt)
            .single()
            .unwrap();

        let args = ByDateArgs::new(None, Some(ed), None, false);

        let start = Instant::now();

        let _result = match by_date(".", args) {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!(
            "completed test_by_date_end_date_only in {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn test_by_date_end_date_only_image() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let dt_local = Local::now();

        let utc_dt = NaiveDate::parse_from_str("2020-03-26", "%Y-%m-%d").unwrap();

        let ed = dt_local
            .timezone()
            .from_local_date(&utc_dt)
            .single()
            .unwrap();

        let start = Instant::now();

        let output = process_date(".", None, Some(ed));

        create_output_image(output.unwrap(), "test_image.png".to_string());

        println!(
            "completed test_by_date_end_date_only_image in {:?}",
            start.elapsed()
        );
    }
}
