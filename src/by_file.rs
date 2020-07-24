use crate::utils::grit_utils;
use anyhow::Result;
use charts::{
    AxisPosition, BarDatum, BarLabelPosition, Chart, ScaleBand, ScaleLinear, VerticalBarView,
};
use chrono::{Date, Local};
use csv::Writer;
use git2::Repository;
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
    image: bool,
    html: bool,
}

impl ByFileArgs {
    pub fn new(
        repo_path: String,
        full_path_filename: String,
        output_file: Option<String>,
        image: bool,
        html: bool,
    ) -> Self {
        ByFileArgs {
            repo_path,
            full_path_filename,
            output_file,
            image,
            html,
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

impl BarDatum for ByFile {
    fn get_category(&self) -> String {
        grit_utils::format_date(self.day)
    }
    fn get_value(&self) -> f32 {
        self.loc as f32
    }
    fn get_key(&self) -> String {
        self.name.clone()
    }
}

type GenResult<T> = Result<T>;

pub fn by_file(args: ByFileArgs) -> GenResult<()> {
    let output_file = args.output_file.clone();
    let image = args.image;
    let file_to_blame = args.full_path_filename.clone();
    let html = args.html;

    println!("Processing file {}", args.full_path_filename);

    let mut results = match process_file(args) {
        Ok(r) => r,
        Err(err) => panic!("Error while processing file:  {:?}", err),
    };

    results.sort_by(|a, b| b.day.cmp(&a.day));

    if image {
        display_image(results, output_file, file_to_blame, html)
    } else {
        display_csv(results, output_file)
    }
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

fn display_image(
    data: Vec<ByFile>,
    file: Option<String>,
    file_to_blame: String,
    html: bool,
) -> GenResult<()> {
    let f = match file {
        Some(f) => f,
        None => panic!("Filename is manditory for images"),
    };

    let (width, height) = if data.len() > 60 {
        (1920, 960)
    } else if data.len() > 35 {
        (1280, 960)
    } else {
        (1027, 768)
    };

    let (top, right, bottom, left) = (90, 40, 50, 60);

    let max_size_obj = data.iter().max_by(|a, b| a.loc.cmp(&b.loc));
    let max_count = match max_size_obj {
        Some(m) => m.loc as f32 + 5.0,
        None => panic!("could not find max count in image creation"),
    };

    let mut authors: Vec<String> = data.iter().map(|d| d.name.clone()).collect();
    authors.sort();
    authors.dedup();

    let dates: Vec<String> = data
        .iter()
        .map(|d| grit_utils::format_date(d.day))
        .collect();

    let x_sb = ScaleBand::new()
        .set_domain(dates)
        .set_range(vec![0, width - left - right]);

    let y_sb = ScaleLinear::new()
        .set_domain(vec![0.0, max_count])
        .set_range(vec![height - top - bottom, 0]);

    let view = VerticalBarView::new()
        .set_x_scale(&x_sb)
        .set_y_scale(&y_sb)
        .set_keys(authors)
        .set_label_position(BarLabelPosition::Center)
        .load_data(&data)
        .expect("Failed to create Vertical View");

    let _chart = Chart::new()
        .set_width(width)
        .set_height(height)
        .set_margins(top, right, bottom, left)
        .add_title(file_to_blame)
        .add_view(&view)
        .add_axis_bottom(&x_sb)
        .add_axis_left(&y_sb)
        .add_legend_at(AxisPosition::Top)
        .set_bottom_axis_tick_label_rotation(-45)
        .save(Path::new(&f))
        .expect("Failed to create Chart");

    if html {
        grit_utils::create_html(&f).expect("Failed to create HTML file");
    }

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
            false,
            false,
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
            false,
            false,
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

    #[test]
    fn test_by_file_with_image() {
        simple_logger::init_with_level(LOG_LEVEL).unwrap_or(());

        let td: TempDir = crate::grit_test::init_repo();

        let args = ByFileArgs::new(
            td.path().to_str().unwrap().to_string(),
            "README.md".to_string(),
            Some(String::from("target/to_file.svg")),
            true,
            true,
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
