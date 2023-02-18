use clap::Parser;

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
    source_directory: String,

    /// Target collection.
    target_directory: String,

    /// Don't do anything, just list the actions.
    dry_run: bool,
}

fn main() {
    let args = Args::parse();
    println!(
        "Synchronize photo collection from {} to {}.",
        args.source_directory, args.target_directory
    )
}
