use anyhow::Result;
use colored::*;
use serde::Serialize;

use crate::core::note::{collect_all_notes, collect_note_names};
use crate::core::paths::VaultPaths;

#[derive(Serialize)]
struct ValidationResult {
    total_files: usize,
    schema_errors: usize,
    broken_wikilinks: usize,
    folder_mismatches: usize,
    files_with_errors: Vec<FileError>,
}

#[derive(Serialize)]
struct FileError {
    file: String,
    errors: Vec<String>,
}

pub fn run(schema_only: bool, wikilinks_only: bool, json: bool) -> Result<()> {
    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);
    let note_names = collect_note_names(&paths);

    let mut result = ValidationResult {
        total_files: notes.len(),
        schema_errors: 0,
        broken_wikilinks: 0,
        folder_mismatches: 0,
        files_with_errors: Vec::new(),
    };

    let check_all = !schema_only && !wikilinks_only;

    for note in &notes {
        let mut errors = Vec::new();

        if check_all || schema_only {
            let violations = note.validate_schema();
            for v in &violations {
                errors.push(format!("[SCHEMA] {}", v));
            }
            result.schema_errors += violations.len();
        }

        if check_all || wikilinks_only {
            let links = note.wikilinks();
            for link in &links {
                if !note_names.contains(link) {
                    errors.push(format!("[WIKILINK] Broken link: [[{}]]", link));
                    result.broken_wikilinks += 1;
                }
            }
        }

        if check_all {
            if !note.check_folder_type_match() {
                errors.push(format!(
                    "[FOLDER] type='{}' status='{}' should not be in {}",
                    note.note_type().unwrap_or("?"),
                    note.status().unwrap_or("?"),
                    note.folder()
                ));
                result.folder_mismatches += 1;
            }
        }

        if !errors.is_empty() {
            result.files_with_errors.push(FileError {
                file: note.name.clone(),
                errors,
            });
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_report(&result);
    }

    if result.schema_errors > 0 || result.broken_wikilinks > 0 || result.folder_mismatches > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn print_report(result: &ValidationResult) {
    println!("{}", "Vault Validation Report".bold());
    println!("{}", "=".repeat(60));
    println!();
    println!("Total files: {}", result.total_files);
    println!();

    if result.files_with_errors.is_empty() {
        println!("{}", "✓ No violations found!".green());
        return;
    }

    println!("{}", "Violations:".red().bold());
    println!("{}", "-".repeat(60));

    for file_err in &result.files_with_errors {
        println!();
        println!("{} {}", "FILE:".cyan(), file_err.file);
        for err in &file_err.errors {
            println!("  {} {}", "•".red(), err);
        }
    }

    println!();
    println!("{}", "Summary:".bold());
    println!(
        "  Schema errors: {}",
        if result.schema_errors > 0 {
            result.schema_errors.to_string().red()
        } else {
            result.schema_errors.to_string().green()
        }
    );
    println!(
        "  Broken wikilinks: {}",
        if result.broken_wikilinks > 0 {
            result.broken_wikilinks.to_string().red()
        } else {
            result.broken_wikilinks.to_string().green()
        }
    );
    println!(
        "  Folder mismatches: {}",
        if result.folder_mismatches > 0 {
            result.folder_mismatches.to_string().red()
        } else {
            result.folder_mismatches.to_string().green()
        }
    );
}
