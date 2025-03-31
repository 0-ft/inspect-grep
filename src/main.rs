use anyhow::{anyhow, Context, Result};
use clap::Parser;
use colored::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};
use std::str::FromStr;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use walkdir::WalkDir;
use zip::ZipArchive;
use lazy_static::lazy_static;

mod inspect;
use inspect::ChatMessageRole;

lazy_static! {
    static ref SAMPLE_ID_EPOCH_RE: Regex =
        Regex::new(r"^samples/(.*)_epoch_(\d+)\.json$").expect("Failed to compile regex");
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to .eval file or directory containing .eval files
    #[arg(required = true)]
    path: PathBuf,

    /// Search pattern (regex)
    #[arg(short, long, required = true)]
    pattern: String,

    /// Filter by sample ID
    #[arg(short, long)]
    samples: Option<String>,

    /// Filter by epoch number
    #[arg(short, long, default_value = "all")]
    epochs: IntFilter,

    /// Filter by message role (comma-separated list: user,assistant,system)
    #[arg(short, long)]
    roles: Option<Vec<ChatMessageRole>>,

    /// Number of threads to use (default: number of CPU cores)
    #[arg(short, long)]
    threads: Option<usize>,
}

trait Filter<T> {
    fn filter(&self, item: &T) -> bool;
}

#[derive(Debug, Clone)]
enum IntFilter {
    All,
    Some(HashSet<u32>),
    Range(u32, u32),
}

impl FromStr for IntFilter {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "all" {
            return Ok(IntFilter::All);
        }
        if let Some((start, end)) = s.split_once('-') {
            return Ok(IntFilter::Range(
                start.parse()?,
                end.parse()?,
            ));
        }
        let nums = s.split(',')
            .map(|n| n.trim().parse::<u32>())
            .collect::<std::result::Result<HashSet<_>, _>>()?;
        Ok(IntFilter::Some(nums))
    }
}

impl Filter<u32> for IntFilter {
    fn filter(&self, item: &u32) -> bool {
        match self {
            IntFilter::All => true,
            IntFilter::Some(ids) => ids.contains(item),
            IntFilter::Range(start, end) => item >= start && item <= end,
        }
    }
}

fn read_zip_file(path: &Path) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn sample_id_and_epoch_from_filename(filename: String) -> Option<(String, u32)> {
    let caps = SAMPLE_ID_EPOCH_RE.captures(&filename);
    if let Some(caps) = caps {
        let sample_id = caps.get(1).unwrap().as_str().to_string();
        let epoch = caps.get(2).unwrap().as_str().parse::<u32>().unwrap();
        Some((sample_id, epoch))
    } else {
        None
    }
}

fn matching_samples_in_log<'a>(
    log_path: &Path,
    sample_regex: &'a Option<Regex>,
    epoch_filter: &'a IntFilter,
) -> Result<Box<dyn Iterator<Item = String> + 'a>> {
    let buffer = read_zip_file(log_path)?;
    let reader = Cursor::new(buffer);
    let archive: ZipArchive<Cursor<Vec<u8>>> = ZipArchive::new(reader)?;

    // Collect file names into owned String values

    let file_names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
    println!("file_names: {:?}", file_names);
    let file_name_matches = move |name: &String| {
        sample_id_and_epoch_from_filename(name.clone()).map_or(false, |(sample_id, epoch)| {
            (sample_regex.as_ref().map_or(true, |re| re.is_match(&sample_id))) && epoch_filter.filter(&epoch)
        })
    };

    // Create an iterator of owned Strings
    Ok(Box::new(
        file_names
            .into_iter()
            .filter(move |name| file_name_matches(name)),
    ))
}

fn process_eval_file(path: &Path, pattern: &Regex, sample_ids: &Option<Regex>, epochs: &IntFilter, roles: &Option<Vec<ChatMessageRole>>) -> Result<Vec<String>> {
    for file in matching_samples_in_log(path, &sample_ids, &epochs)? {
        println!("file: {}", file);
    }
    Ok(vec![])
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Parse filters
    let sample_ids = args.samples.map(|s| Regex::new(&s).ok()).flatten();
    let epochs = args.epochs;
    let roles = args.roles;

    // Compile regex pattern
    let pattern = Regex::new(&args.pattern)?;

    // Collect all .eval files
    let paths: Vec<PathBuf> = if args.path.is_file() {
        vec![args.path]
    } else {
        WalkDir::new(&args.path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "eval"))
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    // Setup progress bar
    let pb = ProgressBar::new(paths.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    // Process files in parallel
    // let m = MultiProgress::new();
    let results: Vec<_> = paths
        .par_iter()
        .map(|path| {
            println!("path: {}", path.display());
            let result = process_eval_file(path, &pattern, &sample_ids, &epochs, &roles);
            return false;
            // process_eval_file(path, &pattern, &sample_ids, &epochs, &roles)
        })
        .collect();
        // .collect::<Result<Vec<_>>>()?
        // .into_iter()
        // .flatten()
        // .collect();

    pb.finish_with_message("Search complete");

    // Display results
    // for (run_id, task, sample, message) in results {
    //     println!(
    //         "\n{} {} {}",
    //         format!("[{}]", run_id).cyan(),
    //         format!("[{}]", task).green(),
    //         format!("[{}]", sample).yellow()
    //     );
    //     println!("{}", message);
    // }

    Ok(())
}
