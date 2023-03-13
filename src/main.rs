use clap::Parser;
use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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

fn analyze_sub_directory(
    path: &Path,
    base_path: &Path,
    result: &mut AnalyzedDirectory,
) -> io::Result<()> {
    for entry in path.read_dir()? {
        let entry = entry?;
        let path = &entry.path();
        if path.is_dir() {
            analyze_sub_directory(path, base_path, result)?;
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
            let entry: &mut Vec<PathBuf> = result.map.entry(key).or_insert(Vec::new());
            let relative_path: &Path =
                path.strip_prefix(base_path)
                    .or(io::Result::Err(io::Error::new(
                        io::ErrorKind::Other,
                        "invalid prefix!",
                    )))?;
            entry.push(relative_path.to_path_buf());
        }
    }
    Ok(())
}

fn analyze_directory(path: &Path) -> io::Result<AnalyzedDirectory> {
    let mut result = AnalyzedDirectory {
        map: BTreeMap::new(),
    };
    analyze_sub_directory(path, path, &mut result)?;
    Ok(result)
}

struct Copy {
    source: PathBuf,
    target: PathBuf,
}
struct Move {
    source: PathBuf,
    target: PathBuf,
}

struct RemoveDuplicate {
    duplicate: PathBuf,
    original: PathBuf,
}

enum Operation {
    Copy(Copy),
    Move(Move),
    RemoveDuplicate(RemoveDuplicate),
}

fn display_analyzed_directory(analyzed_directory: &AnalyzedDirectory) {
    for (key, value) in &analyzed_directory.map {
        println!("size : {}, name : {}", key.size, key.name.to_string_lossy());
        for path in value {
            println!("    path : {}", path.to_string_lossy());
        }
    }
}

fn get_duplicates(analyzed_directory: &AnalyzedDirectory) -> Result<Vec<Vec<&Path>>, String> {
    let mut result = Vec::new();
    for (_, value) in &analyzed_directory.map {
        if value.len() > 1 {
            let mut paths = Vec::new();
            for path in value {
                paths.push(path.as_path());
            }
            result.push(paths);
        }
    }
    Ok(result)
}

fn display_duplicates(duplicates: &Vec<Vec<&Path>>) {
    println!("duplicates:");
    for list in duplicates {
        println!("[");
        for item in list {
            println!("    {}", item.to_string_lossy());
        }
        println!("]");
    }
}

fn sync(
    source_directory: &AnalyzedDirectory,
    target_directory: &AnalyzedDirectory,
) -> Result<Vec<Operation>, String> {
    let mut result = Vec::new();
    for (key, value) in &source_directory.map {
        let source_path = value.first().ok_or("no path for source")?;
        // let SizeAndName{size, name} = key;
        let target_entry = target_directory.map.get(&key);
        match target_entry {
            Some(target_paths) => {
                let mut chosen_target_path: &PathBuf = source_path;
                if !target_paths.contains(&source_path) {
                    chosen_target_path =
                        target_paths.first().ok_or("empty list of target paths")?;
                    result.push(Operation::Move(Move {
                        source: chosen_target_path.clone(),
                        target: source_path.clone(),
                    }));
                }
                for target_path in target_paths.iter() {
                    if target_path != chosen_target_path {
                        result.push(Operation::RemoveDuplicate(RemoveDuplicate {
                            duplicate: target_path.clone(),
                            original: chosen_target_path.clone(),
                        }));
                    }
                }
            }
            None => result.push(Operation::Copy(Copy {
                source: source_path.clone(),
                target: source_path.clone(),
            })),
        }
    }

    Ok(result)
}

fn print_operation(operation: &Operation) {
    match operation {
        Operation::Copy(Copy { source, target }) => {
            println!(
                "copy {} to {}",
                source.to_string_lossy(),
                target.to_string_lossy()
            );
        }
        Operation::Move(Move { source, target }) => {
            println!(
                "move {} to {}",
                source.to_string_lossy(),
                target.to_string_lossy()
            )
        }
        Operation::RemoveDuplicate(RemoveDuplicate {
            duplicate,
            original,
        }) => {
            println!(
                "remove duplicate {} of {}",
                duplicate.to_string_lossy(),
                original.to_string_lossy()
            )
        }
    }
}

fn main2() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    println!(
        "Synchronize photo collection from {} to {}.",
        args.source_directory.to_string_lossy(),
        args.target_directory.to_string_lossy()
    );

    let analyzed_source = analyze_directory(&args.source_directory)?;
    let duplicates_in_source = get_duplicates(&analyzed_source)?;
    if !duplicates_in_source.is_empty() {
        display_duplicates(&duplicates_in_source);
        return Err("The source has duplicates".into());
    }

    let analyzed_target = analyze_directory(&args.target_directory)?;

    println!("Analyzed source:");
    display_analyzed_directory(&analyzed_source);
    println!("Analyzed target:");
    display_analyzed_directory(&analyzed_target);

    let operations = sync(&analyzed_source, &analyzed_target)?;

    for operation in operations.iter() {
        print_operation(operation);
    }

    Ok(())
}

fn main() -> ExitCode {
    let result = main2();
    match result {
        Err(e) => {
            println!("Error: {}", e);
            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}
