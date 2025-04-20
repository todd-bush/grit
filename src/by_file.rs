use super::Processable;
use crate::utils::grit_utils;
use anyhow::{Context, Result};
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

/// Configuration for the ByFile analysis
#[derive(Debug)]
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
    ) -> Self {
        Self {
            path,
            full_path_filename,
            output_file,
            image,
            html,
            restrict_authors,
        }
    }
}

/// Represents a single author's contribution to a file
#[derive(PartialEq, Clone, Debug)]
struct FileContribution {
    author: String,
    date: DateTime<Local>,
    lines: f32,
}

impl FileContribution {
    fn new(author: String, date: DateTime<Local>) -> Self {
        Self {
            author,
            date,
            lines: 0.0,
        }
    }
}

/// Converts a collection of FileContributions into a BTreeMap for charting
impl FromIterator<FileContribution> for BTreeMap<String, Vec<f32>> {
    fn from_iter<T: IntoIterator<Item = FileContribution>>(iter: T) -> Self {
        let mut map = BTreeMap::new();
        for item in iter {
            map.entry(grit_utils::format_date(item.date))
                .or_insert_with(Vec::new)
                .push(item.lines);
        }
        map
    }
}

/// Main ByFile analysis struct
pub struct ByFile {
    args: ByFileArgs,
}

impl ByFile {
    pub fn new(args: ByFileArgs) -> Self {
        Self { args }
    }

    /// Processes git blame for a single file and returns contributions by author
    fn process_blame(&self) -> Result<Vec<FileContribution>> {
        let repo = Repository::open(&self.args.path)
            .with_context(|| format!("Failed to open repository at {}", self.args.path))?;

        let path = Path::new(&self.args.full_path_filename);
        let restrict_authors = grit_utils::convert_string_list_to_vec(self.args.restrict_authors.clone());
        let mut author_contributions: HashMap<String, FileContribution> = HashMap::new();

        let blame = repo.blame_file(path, None)
            .with_context(|| format!("Failed to blame file {}", self.args.full_path_filename))?;

        for hunk in blame.iter() {
            let sig = hunk.final_signature();
            let author = String::from_utf8_lossy(sig.name_bytes()).to_string();
            let commit = repo.find_commit(hunk.final_commit_id())?;
            let commit_date = grit_utils::convert_git_time(&commit.time());

            if let Some(ref authors) = restrict_authors {
                if authors.iter().any(|a| a == &author) {
                    continue;
                }
            }

            let key = format!("{}-{}", author, grit_utils::format_date(commit_date));
            let contribution = match author_contributions.entry(key) {
                Vacant(entry) => entry.insert(FileContribution::new(author, commit_date)),
                Occupied(entry) => entry.into_mut(),
            };

            contribution.lines += hunk.lines_in_hunk() as f32;
        }

        let mut results: Vec<FileContribution> = author_contributions.into_values().collect();
        results.sort_by(|a, b| b.date.cmp(&a.date));

        Ok(results)
    }

    /// Displays results in CSV format
    fn display_csv(&self, data: Vec<FileContribution>) -> Result<()> {
        let writer: Box<dyn Write> = match &self.args.output_file {
            Some(f) => Box::new(File::create(f)?),
            None => Box::new(io::stdout()),
        };

        let mut csv_writer = Writer::from_writer(writer);
        csv_writer.write_record(&["author", "date", "lines"])?;

        for contribution in data {
            csv_writer.serialize((
                contribution.author,
                grit_utils::format_date(contribution.date),
                contribution.lines,
            ))?;
        }

        csv_writer.flush()?;
        Ok(())
    }

    /// Creates and displays a chart of the results
    fn display_image(&self, data: Vec<FileContribution>) -> Result<()> {
        let output_file = self.args.output_file.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Output file is required for image generation"))?;

        let (width, height) = self.calculate_chart_dimensions(data.len());
        let margins = (90, 40, 50, 60);

        let dates: Vec<String> = data
            .iter()
            .map(|d| grit_utils::format_date(d.date))
            .collect();

        let chart_data: Vec<Series> = BTreeMap::from_iter(data)
            .iter()
            .map(|(k, v)| Series::new(k.clone(), v.clone()))
            .collect();

        let mut chart = LineChart::new_with_theme(chart_data, dates, "chaulk");
        self.configure_chart(&mut chart, width, height, margins);

        std::fs::write(output_file, chart.svg()?)?;

        if self.args.html {
            grit_utils::create_html(output_file)?;
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
    fn configure_chart(&self, chart: &mut LineChart, width: u32, height: u32, margins: (u32, u32, u32, u32)) {
        chart.width = width as f32;
        chart.height = height as f32;
        chart.margin.top = margins.0 as f32;
        chart.margin.right = margins.1 as f32;
        chart.margin.bottom = margins.2 as f32;
        chart.margin.left = margins.3 as f32;
        chart.title_text = self.args.full_path_filename.clone();
    }
}

impl Processable<()> for ByFile {
    fn process(&self) -> Result<()> {
        let results = self.process_blame()?;

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

        let result = match bf.process() {
            Ok(()) => true,
            Err(e) => {
                error!("test_by_file ended in error: {:?}", e);
                false
            }
        };

        assert!(result, "See error above");
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

        let result = match bf.process() {
            Ok(()) => true,
            Err(e) => {
                error!("test_by_file_with_image ended in error: {:?}", e);
                false
            }
        };

        assert!(result, "See error above");
    }
}
