#[macro_use]
use chrono::naive::{MAX_DATE, MIN_DATE};
use chrono::{Datelike, NaiveDateTime};
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
    start_date: Option<NaiveDateTime>,
    end_date: Option<NaiveDateTime>,
    file: Option<String>,
}

impl ByDateArgs {
    pub fn new(
        start_date: Option<NaiveDateTime>,
        end_date: Option<NaiveDateTime>,
        file: Option<String>,
    ) -> Self {
        ByDateArgs {
            start_date,
            end_date,
            file,
        }
    }
}

pub fn by_date(repo_path: &str, args: ByDateArgs) -> Result<(), Error> {
    let output = process_date(repo_path, args.start_date, args.end_date)?;
    match display_output(output, args.file) {
        Ok(_v) => {}
        Err(e) => error!("Error thrown in display_output {:?}", e),
    };

    Ok(())
}

fn process_date(
    repo_path: &str,
    start_date: Option<NaiveDateTime>,
    end_date: Option<NaiveDateTime>,
) -> Result<Vec<ByDate>, Error> {
    let end_date = match end_date {
        Some(d) => d,
        None => MAX_DATE.and_hms(0, 0, 0),
    };

    let start_date = match start_date {
        Some(d) => d,
        None => MIN_DATE.and_hms(0, 0, 0),
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

        if commit_time < start_date.timestamp() {
            return None;
        }

        if commit_time > end_date.timestamp() {
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

fn convert_git_time(time: &Time) -> NaiveDateTime {
    NaiveDateTime::from_timestamp(time.seconds(), 0)
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
    root.fill(&WHITE);

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(40)
        .margin(5)
        .caption("Commits by Date", ("sans-serif", 50.0).into_font())
        .build_ranged(0f32..10f32, LogRange(0.1f32..1e10f32))?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_axis()
        .y_desc("Commits")
        .y_label_formatter(&|y| format!("{:e}", y))
        .draw()?;

    chart.draw_series(LineSeries::new(
        output.iter().map(|db| (1_f32, db.count as f32)),
        &BLUE,
    ))?;

    // chart
    //     .draw_series(LineSeries::new(
    //         output.iter().map(|db| (db.date, db.count)),
    //         Into::<ShapeStyle>::into(&RED).stroke_width(3),
    //     ))
    //     .expect("Drawing Error");

    Ok(())
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

        let args = ByDateArgs::new(None, None, None);

        let _result = match by_date(".", args) {
            Ok(()) => true,
            Err(_e) => false,
        };

        println!("completed test_by_date_no_ends in {:?}", start.elapsed());
    }

    #[test]
    fn test_by_date_end_date_only() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let ed = NaiveDateTime::parse_from_str("2020-03-26 23:59:59", "%Y-%m-%d %H:%M:%S");

        let args = ByDateArgs::new(None, Some(ed.unwrap()), None);

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
}
