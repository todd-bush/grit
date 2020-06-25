use crate::utils::grit_utils;
use chrono::{Date, Local};
use csv::Writer;
use git2::Repository;
use plotters::prelude::*;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

pub struct ByFileArgs {
    repo_path: String,
    full_path_filename: String,
    output_file: Option<String>,
}

impl ByFileArgs {
    pub fn new(repo_path: String, full_path_filename: String, output_file: Option<String>) -> Self {
        ByFileArgs {
            repo_path,
            full_path_filename,
            output_file,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct ByFile {
    name: String,
    day: Date<Local>,
    loc: usize,
}

impl ByFile {
    fn new(name: String, day: Date<Local>) -> Self {
        ByFile { name, day, loc: 0 }
    }
}

type GenResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn by_file(args: ByFileArgs) -> GenResult<()> {
    let output_file = args.output_file.clone();

    println!("Processing file {}", args.full_path_filename);

    let mut results = match process_file(args) {
        Ok(r) => r,
        Err(err) => panic!("Error while processing file:  {:?}", err),
    };

    results.sort_by(|a, b| b.day.cmp(&a.day));

    display_csv(results, output_file)
}

fn process_file(args: ByFileArgs) -> GenResult<Vec<ByFile>> {
    info!("Beginning to process file {}", args.full_path_filename);
    let start = Instant::now();

    let repo = Repository::open(args.repo_path)?;

    let path = Path::new(args.full_path_filename.as_str());

    let blame = repo.blame_file(path, None)?;
    let mut auth_to_loc: HashMap<ByFile, usize> = HashMap::new();

    for hunk in blame.iter() {
        let sig = hunk.final_signature();
        let signame = String::from_utf8_lossy(sig.name_bytes()).to_string();
        let commit = repo.find_commit(hunk.final_commit_id())?;
        let commit_date = grit_utils::convert_git_time(&commit.time());

        let key = ByFile::new(signame, commit_date);

        let v = match auth_to_loc.entry(key) {
            Vacant(entry) => entry.insert(0),
            Occupied(entry) => entry.into_mut(),
        };
        *v += hunk.lines_in_hunk();
    }

    let results = auth_to_loc
        .iter()
        .map(|(k, v)| {
            let mut r = k.clone();
            r.loc = *v;
            r
        })
        .collect();

    info!(
        "Completed process_file for file {} in {:?}",
        args.full_path_filename,
        start.elapsed()
    );

    Ok(results)
}

fn display_csv(data: Vec<ByFile>, file: Option<String>) -> GenResult<()> {
    let w = match file {
        Some(f) => {
            let file = File::create(f)?;
            Box::new(file) as Box<dyn Write>
        }
        None => Box::new(io::stdout()) as Box<dyn Write>,
    };

    let mut writer = Writer::from_writer(w);

    writer.write_record(&["author", "date", "loc"])?;

    data.iter().for_each(|d| {
        writer
            .serialize((d.name.clone(), grit_utils::format_date(d.day), d.loc))
            .expect("Could not write record");
    });

    writer.flush()?;

    Ok(())
}

fn display_image(data: Vec<ByFile>, file: Option<String>) -> GenResult<()> {
    let f = match file {
        Some(f) => f,
        None => panic!("Filename is manditory for images"),
    };

    let root = BitMapBackend::new(&f, (1280, 960)).into_drawing_area();
    root.fill(&WHITE)?;

    let (from_date, to_date) = (
        data[0].day,
        data.last().expect("Cannot find last entry in output").day,
    );

    let output_count = data.len();

    let max_size_obj = data.iter().max_by(|a, b| a.loc.cmp(&b.loc));
    let max_count = match max_size_obj {
        Some(m) => m.loc as f32 + 5.0,
        None => panic!("could not find max count in image creation"),
    };

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(40)
        .margin(5)
        .caption("Commits by Date", ("sans-serif", 32.0).into_font())
        .build_ranged(from_date..to_date, 0f32..max_count)?;

    chart
        .configure_mesh()
        .y_labels(output_count)
        .y_desc("Commits")
        .y_label_formatter(&|y| format!("{}", y))
        .x_label_formatter(&|x| format!("{}", x.format("%Y-%m-%d")))
        .draw()?;

    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use log::Level;
    use tempfile::TempDir;

    const LOG_LEVEL: Level = Level::Info;

    #[test]
    fn test_process_file() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();

        let args = ByFileArgs::new(
            td.path().to_str().unwrap().to_string(),
            "src/by_date.rs".to_string(),
            None,
        );

        let results: Vec<ByFile> = process_file(args).unwrap();

        assert!(results.len() > 0, "Results length was 0 len");

        info!("results: {:?}", results);
    }

    #[test]
    fn test_by_file() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();

        let args = ByFileArgs::new(
            td.path().to_str().unwrap().to_string(),
            "src/by_date.rs".to_string(),
            None,
        );

        let s = match by_file(args) {
            Ok(()) => true,
            Err(e) => {
                error!("test_by_file ended in error {:?}", e);
                false
            }
        };

        assert!(s, "See error above");
    }
}
