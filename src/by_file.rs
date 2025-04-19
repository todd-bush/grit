use super::Processable;
use crate::utils::grit_utils;
use anyhow::Result;
use charts_rs::{LineChart, Series};
use chrono::DateTime;
use chrono::offset::Local;
use csv::Writer;
use git2::Repository;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;

pub struct ByFileArgs {
    path: String,
    full_path_filename: String,
    output_file: Option<String>,
    image: bool,
    html: bool,
    restrict_authors: Option<String>,
}

impl ByFileArgs {
    pub fn new(
        path: String,
        full_path_filename: String,
        output_file: Option<String>,
        image: bool,
        html: bool,
        restrict_authors: Option<String>,
    ) -> ByFileArgs {
        ByFileArgs {
            path: path,
            full_path_filename: full_path_filename,
            output_file: output_file,
            image: image,
            html: html,
            restrict_authors: restrict_authors,
        }
    }
}

#[derive(PartialEq, Clone)]
struct ByFileOutput {
    name: String,
    day: DateTime<Local>,
    loc: f32,
}

impl ByFileOutput {
    fn new(name: String, day: DateTime<Local>) -> ByFileOutput {
        ByFileOutput {
            name: name,
            day: day,
            loc: 0.0,
        }
    }
}

impl FromIterator<ByFileOutput> for BTreeMap<String, Vec<f32>> {
    fn from_iter<T: IntoIterator<Item = ByFileOutput>>(iter: T) -> Self
    where
        T: IntoIterator<Item = ByFileOutput>,
    {
        let mut map = BTreeMap::new();
        for item in iter {
            map.entry(grit_utils::format_date(item.day))
                .or_insert_with(Vec::new)
                .push(item.loc as f32);
        }
        map
    }
}

pub struct ByFile {
    args: ByFileArgs,
}

impl ByFile {
    pub fn new(args: ByFileArgs) -> ByFile {
        ByFile { args: args }
    }

    fn display_csv(&self, data: Vec<ByFileOutput>) -> Result<()> {
        let w = match &self.args.output_file {
            Some(f) => {
                let file = File::create(f)?;
                Box::new(file) as Box<dyn Write>
            }
            None => Box::new(io::stdout()) as Box<dyn Write>,
        };

        let mut writer = Writer::from_writer(w);

        writer
            .write_record(&["author", "date", "loc"])
            .expect("Could not write csv header");

        data.iter().for_each(|d| {
            writer
                .serialize((d.name.clone(), grit_utils::format_date(d.day), d.loc))
                .expect("Could not write csv row");
        });

        writer.flush().expect("Could not flush csv writer");

        Ok(())
    }

    fn display_image(&self, data: Vec<ByFileOutput>) -> Result<()> {
        let f = match &self.args.output_file {
            Some(f) => f,
            None => panic!("File name is manditory for images"),
        };

        let (width, height) = if data.len() > 60 {
            (1920, 960)
        } else if data.len() > 35 {
            (1280, 960)
        } else {
            (1028, 768)
        };

        let (top, right, bottom, left) = (90, 40, 50, 60);

        let chart_map: BTreeMap<String, Vec<f32>> = BTreeMap::from_iter(data.clone());

        let chart_data: Vec<Series> = chart_map
            .iter()
            .map(|(k, v)| (Series::new(k.clone(), v.clone())))
            .collect();

        let dates: Vec<String> = data
            .iter()
            .map(|d| grit_utils::format_date(d.day))
            .collect();

        let mut line_chart = LineChart::new_with_theme(chart_data, dates, "chaulk");

        line_chart.width = width as f32;
        line_chart.height = height as f32;
        line_chart.margin.top = top as f32;
        line_chart.margin.right = right as f32;
        line_chart.margin.bottom = bottom as f32;
        line_chart.margin.left = left as f32;

        line_chart.title_text = self.args.full_path_filename.clone();

        std::fs::write(Path::new(&f), line_chart.svg().unwrap()).expect("Failed to create chart");

        if self.args.html {
            grit_utils::create_html(&f).expect("failed to creat HTML page");
        }

        Ok(())
    }
}

impl Processable<()> for ByFile {
    fn process(&self) -> Result<()> {
        let repo = Repository::open(&self.args.path)?;

        let path = Path::new(&self.args.full_path_filename);

        let mut auth_to_loc: HashMap<String, ByFileOutput> = HashMap::new();

        let restrict_authors: Option<Vec<String>> =
            grit_utils::convert_string_list_to_vec(self.args.restrict_authors.clone());

        let blame = repo.blame_file(path, None)?;

        for hunk in blame.iter() {
            let sig = hunk.final_signature();
            let signame = String::from_utf8_lossy(sig.name_bytes()).to_string();
            let commit = repo.find_commit(hunk.final_commit_id())?;
            let commit_date = grit_utils::convert_git_time(&commit.time());

            if let Some(ref v) = restrict_authors {
                if v.iter().any(|a| a == &signame) {
                    break;
                }
            }

            let commit_date_str = grit_utils::format_date(commit_date);

            let key = &[&signame, "-", &commit_date_str].join("");

            let v = match auth_to_loc.entry(key.to_string()) {
                Vacant(entry) => entry.insert(ByFileOutput::new(signame, commit_date)),
                Occupied(entry) => entry.into_mut(),
            };

            v.loc += hunk.lines_in_hunk() as f32;
        }

        let mut results: Vec<ByFileOutput> = auth_to_loc.values().cloned().collect();

        results.sort_by(|a, b| b.day.cmp(&a.day));

        if self.args.image {
            self.display_image(results)?;
        } else {
            self.display_csv(results)?;
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
    fn test_by_file() {
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();

        let args = ByFileArgs::new(
            td.path().to_str().unwrap().to_string(),
            "src/by_date.rs".to_string(),
            None,
            false,
            false,
            None,
        );

        let bf = ByFile::new(args);

        let s = match bf.process() {
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
        crate::grit_test::set_test_logging(LOG_LEVEL);

        let td: TempDir = crate::grit_test::init_repo();

        let args = ByFileArgs::new(
            td.path().to_str().unwrap().to_string(),
            "README.md".to_string(),
            Some(String::from("target/to_file.svg")),
            true,
            true,
            None,
        );

        let bf = ByFile::new(args);

        let s = match bf.process() {
            Ok(()) => true,
            Err(e) => {
                error!("test_by_file ended in error {:?}", e);
                false
            }
        };

        assert!(s, "See error above");
    }
}
