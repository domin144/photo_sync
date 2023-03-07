use clap::Parser;
use std::collections::{btree_map, BTreeMap};
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

fn analyze_sub_directory(path: &Path, base_path: &Path) -> io::Result<AnalyzedDirectory> {
    let mut result = AnalyzedDirectory {
        map: BTreeMap::new(),
    };

    for entry in path.read_dir()? {
        let entry = entry?;
        let path = &entry.path();
        if path.is_dir() {
            let mut partial_result = analyze_sub_directory(path, base_path)?;
            /* TODO: do not append! It overwrites existing entries. */
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

    Ok(result)
}

fn analyze_directory(path: &Path) -> io::Result<AnalyzedDirectory> {
    analyze_sub_directory(path, path)
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
    for list in duplicates {
        println!("[");
        for item in list {
            println!("    {}", item.to_string_lossy());
        }
        println!("]");
    }
}

// fn sync(
//     source_directory: &AnalyzedDirectory,
//     target_directory: &AnalyzedDirectory,
// ) -> Result<Vec<Operation>, String> {
//     for (key, value) in &target_directory.map {
//         // let SizeAndName{size, name} = key;
//         let source_entry = source_directory.map.get(&key);
//         if let Some(paths) = source_entry {
//             let source_path: Path = paths.get(0).ok_or("no path for source entry");
//         }
//         for path in value {
//             println!("    path : {}", path.to_string_lossy());
//         }
//     }

//     Err(String::from("TODO"))
// }

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
