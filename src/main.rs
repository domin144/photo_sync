use clap::Parser;
use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Simple program to update target photo collection to have a folder structure
/// of the source photo collection.
/// The program tries to avoid copying files and moves files, which match in
/// file name and size.
/// The source collection is never modified.
/// The files in the target collection are never deleted, but may be moved to
/// new location to match the source structure.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Source collection
    source_directory: PathBuf,

    /// Target collection.
    target_directory: PathBuf,

    /// Don't do anything, just list the actions.
    #[arg(short, long)]
    dry_run: bool,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SizeAndName {
    size: u64,
    name: OsString,
}

struct AnalyzedDirectory {
    map: BTreeMap<SizeAndName, Vec<PathBuf>>,
}

fn analyze_directory(path: &Path) -> io::Result<AnalyzedDirectory> {
    let mut result = AnalyzedDirectory {
        map: BTreeMap::new(),
    };

    for entry in path.read_dir()? {
        let entry = entry?;
        let path = &entry.path();
        if path.is_dir() {
            let mut partial_result = analyze_directory(path)?;
            result.map.append(&mut partial_result.map);
        } else if path.is_file() {
            let size = path.metadata()?.len();
            let name = path
                .file_name()
                .ok_or(io::Error::new(
                    io::ErrorKind::Other,
                    "path did not end with a file name",
                ))?
                .to_owned();
            let key = SizeAndName { size, name };
            let entry = result.map.entry(key).or_insert(Vec::new());
            entry.push(path.clone());
        }
    }

    Ok(result)
}

struct Move {
    source: PathBuf,
    target: PathBuf,
}

struct RemoveDuplicate {
    duplicate: PathBuf,
    original: PathBuf,
}

enum Operatrion {
    Move(Move),
    RemoveDuplicate(RemoveDuplicate),
    RemoveEmptyDirectory(PathBuf),
}

fn display_analyzed_directory(analyzed_directory: &AnalyzedDirectory) {
    for (key, value) in &analyzed_directory.map {
        println!("size : {}, name : {}", key.size, key.name.to_string_lossy());
        for path in value {
            println!("    path : {}", path.to_string_lossy());
        }
    }
}

// fn sync(sourceDir: &AnalyzedDirectory, targetDir: &AnalyzedDirectory) -> Result<(), String> {}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    println!(
        "Synchronize photo collection from {} to {}.",
        args.source_directory.to_string_lossy(),
        args.target_directory.to_string_lossy()
    );
    let analyzed_source = analyze_directory(&args.source_directory)?;
    let analyzed_target = analyze_directory(&args.target_directory)?;

    println!("Analyzed source:");
    display_analyzed_directory(&analyzed_source);
    println!("Analyzed target:");
    display_analyzed_directory(&analyzed_target);

    Ok(())
}
