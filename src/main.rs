mod scanner;

use std::env;
use std::path::PathBuf;

fn main() {
    let mut args = env::args_os().skip(1);

    let Some(command) = args.next() else {
        print_usage();
        return;
    };

    if command != "scan" {
        eprintln!("Unknown command.");
        print_usage();
        std::process::exit(1);
    }

    let directories: Vec<PathBuf> = args.map(PathBuf::from).collect();

    if directories.is_empty() {
        eprintln!("Please provide at least one directory to scan.");
        print_usage();
        std::process::exit(1);
    }

    let mut total_files = 0;
    let mut total_errors = 0;

    for directory in directories {
        println!("\nScanning: {}", directory.display());

        let result = scanner::scan_directory(&directory);

        for file in &result.files {
            let extension = file.extension.as_deref().unwrap_or("[no extension]");

            println!("  {} ({})", file.path.display(), extension);
        }

        for error in &result.errors {
            eprintln!("  ERROR: {}: {}", error.path.display(), error.message);
        }

        println!(
            "\nFound {} files and {} errors.",
            result.files.len(),
            result.errors.len()
        );

        total_files += result.files.len();
        total_errors += result.errors.len();
    }

    println!("\n--- Scan summary ---");
    println!("Total files: {total_files}");
    println!("Total errors: {total_errors}");

    if total_errors > 0 {
        std::process::exit(1);
    }
}

fn print_usage() {
    println!("Marquee - Local-first media server");
    println!();
    println!("Usage:");
    println!("  marquee scan <directory> [directory ...]");
}
