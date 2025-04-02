use anyhow::Result;
use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use rayon::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::str::FromStr;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
use zip::ZipArchive;
use lazy_static::lazy_static;

mod inspect;
use inspect::{deserialize_sample_filtered, ChatMessage, ChatMessageRole, EvalSample};

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
    #[arg(short, long)]
    message_regex: Option<String>,

    /// Filter by sample ID
    #[arg(short, long)]
    samples: Option<String>,

    /// Filter by epoch number
    #[arg(short, long, default_value = "all")]
    epochs: IntFilter,

    /// Filter by message role
    #[arg(short, long, value_delimiter = ',', num_args = 0..)]
    roles: Vec<ChatMessageRole>,

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
) -> Result<Vec<String>> {
    let reader = std::fs::File::open(log_path)?;
    let archive: ZipArchive<std::fs::File> = ZipArchive::new(reader)?;

    // Collect file names into owned String values

    let file_names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
    let file_name_matches = move |name: &String| {
        sample_id_and_epoch_from_filename(name.clone()).map_or(false, |(sample_id, epoch)| {
            (sample_regex.as_ref().map_or(true, |re| re.is_match(&sample_id))) && epoch_filter.filter(&epoch)
        })
    };

    // Create an iterator of owned Strings
    Ok(file_names
        .into_iter()
        .filter(move |name| file_name_matches(name))
        .collect())
}

fn read_sample_filtered<F>(log_path: &Path, sample_filename: &str, message_filter: F) -> Result<EvalSample>
where
    F: Fn(&ChatMessage) -> bool,
{
    let reader = std::fs::File::open(log_path)?;
    let mut archive: ZipArchive<std::fs::File> = ZipArchive::new(reader)?;

    let file = archive.by_name(sample_filename)?;
    let sample = deserialize_sample_filtered(file, message_filter)?;
    Ok(sample)
}

fn process_eval_file(log_path: &Path, sample_paths: &Vec<String>, roles: &Option<Vec<ChatMessageRole>>, pattern: Option<&Regex>) -> Vec<EvalSample> {
    let message_filter = move |message: &ChatMessage| {
        if let Some(roles) = roles {
            if !roles.contains(&message.role){ return false }
        }
        if let Some(pattern) = pattern {
            if !pattern.is_match(&message.content) { return false }
        }
        true
    };

    return sample_paths.par_iter()
        .map(|file| {
            read_sample_filtered(&log_path, file, &message_filter).expect(&format!("Failed to read sample {}", file))
        })
        .collect::<Vec<EvalSample>>();
        // .collect::<HashMap<String, Vec<Option<ChatMessage>>>>();

    // sample_messages
}

fn display_message(source: (&Path, &str, i64), message: &ChatMessage, highlight_regex: Option<&Regex>) {
    let (log_file, sample_id, epoch) = source;
    // let terminal_width = term_size::dimensions().map(|(w, _)| w).unwrap_or(80);
    
    // Determine role-based color
    let role_color = match message.role {
        ChatMessageRole::System => Color::Magenta,
        ChatMessageRole::User => Color::Blue,
        ChatMessageRole::Assistant => Color::Green,
        ChatMessageRole::Tool => Color::Yellow,
    };

    // Format role
    let role = format!("[{}]", message.role.to_string().to_lowercase())
        .color(role_color)
        .bold();
    
    // Create header with source info and role
    let header = format!("{} sample {} epoch {} | {}", 
        log_file.file_name().unwrap().to_string_lossy().cyan(),
        sample_id.yellow(),
        epoch.to_string().green(),
        role
    );
    
    // Process content with highlighting
    let mut content = message.content.clone();
    if let Some(regex) = highlight_regex {
        content = regex.replace_all(&content, |caps: &regex::Captures| {
            format!("{}", caps[0].red().bold())
        }).to_string();
    }

    // Print header
    println!("\n{}", header);
    
    println!("{}", content);

    println!(); // Add spacing between messages
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Parse filters
    let sample_ids = args.samples.map(|s| Regex::new(&s).ok()).flatten();
    let epochs = args.epochs;
    let roles = (!args.roles.is_empty()).then_some(args.roles);

    // Compile regex pattern
    let message_regex = args.message_regex.map(|s| Regex::new(&s).expect("Failed to compile message regex"));

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
    paths
        .par_iter()
        .map(|path| {
            let sample_paths = matching_samples_in_log(&path, &sample_ids, &epochs).unwrap();
            (path, process_eval_file(path, &sample_paths, &roles, message_regex.as_ref()))
        })
        .for_each(|(path, samples)| {
            for sample in samples {
                for message in sample.messages.iter().dedup_by(|a, b| a.is_none() && b.is_none()) {
                    if let Some(message) = message {
                        display_message((path, &sample.id, sample.epoch), message, message_regex.as_ref());
                    }
                }
            }
        });

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
