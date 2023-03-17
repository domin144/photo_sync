use clap::Parser;
use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsString;
use std::fs::create_dir_all;
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
) -> Result<(), String> {
    for entry in path.read_dir().or(Err(format!(
        "Failed to read dir {}",
        path.to_string_lossy()
    )))? {
        let entry = entry.or(Err(format!(
            "Faulty entry in dir {}",
            path.to_string_lossy()
        )))?;
        let path = &entry.path();
        if path.is_dir() {
            analyze_sub_directory(path, base_path, result)?;
        } else if path.is_file() {
            let size = path
                .metadata()
                .or(Err(format!(
                    "Could not get metadata for {}",
                    path.to_string_lossy()
                )))?
                .len();
            let name = path
                .file_name()
                .ok_or(format!(
                    "Path {} did not end with a file name.",
                    path.to_string_lossy()
                ))?
                .to_owned();
            let key = SizeAndName { size, name };
            let entry: &mut Vec<PathBuf> = result.map.entry(key).or_insert(Vec::new());
            let relative_path: &Path = path.strip_prefix(base_path).or(Err(format!(
                "Prefix {} not in path {}.",
                base_path.to_string_lossy(),
                path.to_string_lossy()
            )))?;
            entry.push(relative_path.to_path_buf());
        }
    }
    Ok(())
}

fn analyze_directory(path: &Path) -> Result<AnalyzedDirectory, String> {
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

fn create_parent(full_target_path: &Path) -> Result<(), String> {
    let full_target_directory: &Path = full_target_path.parent().ok_or(format!(
        "Failed to get parent of path \"{}\".",
        full_target_path.to_string_lossy()
    ))?;
    if !full_target_directory.exists() {
        create_dir_all(full_target_directory).or(Err(format!(
            "Failed to create directory \"{}\".",
            full_target_directory.to_string_lossy()
        )))?;
    }
    Ok(())
}

fn execute_copy(source: &Path, target: &Path, operation: &Copy) -> Result<(), String> {
    let full_source_path: PathBuf = source.join(&operation.source);
    let full_target_path: PathBuf = target.join(&operation.target);

    if full_target_path.exists() {
        return Err(format!(
            "The target file \"{}\" already exists, copy would overwrite.",
            full_target_path.to_string_lossy()
        ));
    }

    create_parent(&full_target_path)?;

    std::fs::copy(&full_source_path, &full_target_path).or(Err(format!(
        "Copy from \"{}\" to \"{}\" failed.",
        full_source_path.to_string_lossy(),
        full_target_path.to_string_lossy()
    )))?;

    Ok(())
}

fn execute_move(target: &Path, operation: &Move) -> Result<(), String> {
    let full_source_path: PathBuf = target.join(&operation.source);
    let full_target_path: PathBuf = target.join(&operation.target);

    if full_target_path.exists() {
        return Err(format!(
            "The target file \"{}\" already exists, move would overwrite.",
            full_target_path.to_string_lossy()
        ));
    }

    create_parent(&full_target_path)?;

    std::fs::rename(&full_source_path, &full_target_path).or(Err(format!(
        "Move from \"{}\" to \"{}\" failed.",
        full_source_path.to_string_lossy(),
        full_target_path.to_string_lossy()
    )))
}

fn execute_remove_duplicate(target: &Path, operation: &RemoveDuplicate) -> Result<(), String> {
    let full_target_path: PathBuf = target.join(&operation.duplicate);
    std::fs::remove_file(&full_target_path).or(Err(format!(
        "Failed to remove \"{}\".",
        full_target_path.to_string_lossy()
    )))
}

fn execute(source: &Path, target: &Path, script: &Vec<Operation>) -> Result<(), String> {
    for operation in script.iter() {
        match operation {
            Operation::Copy(operation) => {
                execute_copy(source, target, &operation)?;
            }
            Operation::Move(operation) => {
                execute_move(target, &operation)?;
            }
            Operation::RemoveDuplicate(operation) => {
                execute_remove_duplicate(target, &operation)?;
            }
        }
    }

    Ok(())
}

fn main2() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    println!(
        "Synchronize photo collection from {} to {}.",
        args.source_directory.to_string_lossy(),
        args.target_directory.to_string_lossy()
    );
    println!("Dry run: {}", args.dry_run);

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

    if !args.dry_run {
        execute(&args.source_directory, &args.target_directory, &operations)?;
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
