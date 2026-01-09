use anyhow::Result;
use colored::*;
use serde::Serialize;

use crate::core::note::{collect_all_notes, collect_note_names};
use crate::core::paths::VaultPaths;

#[derive(Serialize)]
struct AuditResult {
    timestamp: String,
    total_checks: usize,
    passed: usize,
    warned: usize,
    failed: usize,
    checks: Vec<CheckResult>,
}

#[derive(Serialize)]
struct CheckResult {
    id: String,
    name: String,
    status: String,
    errors: usize,
    details: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    files: Vec<String>,
}

pub fn run(quick: bool, json: bool, strict: bool) -> Result<()> {
    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);
    let note_names = collect_note_names(&paths);

    let mut checks = Vec::new();

    let schema_result = check_schema(&notes);
    checks.push(schema_result);

    let wikilink_result = check_wikilinks(&notes, &note_names);
    checks.push(wikilink_result);

    if !quick {
        let gist_result = check_gist(&notes);
        checks.push(gist_result);

        let tag_result = check_tags(&notes);
        checks.push(tag_result);

        let orphan_result = check_orphans(&notes, &note_names);
        checks.push(orphan_result);
    }

    let passed = checks.iter().filter(|c| c.status == "pass").count();
    let warned = checks.iter().filter(|c| c.status == "warn").count();
    let failed = checks.iter().filter(|c| c.status == "fail").count();

    let result = AuditResult {
        timestamp: chrono::Local::now().to_rfc3339(),
        total_checks: checks.len(),
        passed,
        warned,
        failed,
        checks,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_report(&result);
    }

    if strict && failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn check_schema(notes: &[crate::core::note::Note]) -> CheckResult {
    let mut errors = 0;
    for note in notes {
        errors += note.validate_schema().len();
    }

    CheckResult {
        id: "schema".to_string(),
        name: "YAML Schema".to_string(),
        status: if errors == 0 { "pass" } else { "fail" }.to_string(),
        errors,
        details: None,
        files: Vec::new(),
    }
}

fn check_wikilinks(
    notes: &[crate::core::note::Note],
    note_names: &std::collections::HashSet<String>,
) -> CheckResult {
    let mut errors = 0;
    for note in notes {
        for link in note.wikilinks() {
            if !note_names.contains(&link) {
                errors += 1;
            }
        }
    }

    CheckResult {
        id: "wikilinks".to_string(),
        name: "Wikilinks".to_string(),
        status: if errors == 0 { "pass" } else { "fail" }.to_string(),
        errors,
        details: None,
        files: Vec::new(),
    }
}

fn check_gist(notes: &[crate::core::note::Note]) -> CheckResult {
    let missing_notes: Vec<String> = notes
        .iter()
        .filter(|n| match n.gist() {
            None => true,
            Some(g) => g.trim().is_empty(),
        })
        .map(|n| n.name.clone())
        .collect();

    let missing = missing_notes.len();

    // Use "warn" status instead of "fail" - empty gist is not critical
    // AI or human can fill these later
    CheckResult {
        id: "gist".to_string(),
        name: "Gist Quality".to_string(),
        status: if missing == 0 { "pass" } else { "warn" }.to_string(),
        errors: missing,
        details: Some(format!("{} notes missing gist", missing)),
        files: missing_notes,
    }
}

fn check_tags(notes: &[crate::core::note::Note]) -> CheckResult {
    let without_tags = notes.iter().filter(|n| n.tags().is_empty()).count();
    let ratio = without_tags as f64 / notes.len() as f64;

    CheckResult {
        id: "tags".to_string(),
        name: "Tag Usage".to_string(),
        status: if ratio < 0.3 { "pass" } else { "fail" }.to_string(),
        errors: without_tags,
        details: Some(format!("{:.0}% notes without tags", ratio * 100.0)),
        files: Vec::new(),
    }
}

fn check_orphans(
    notes: &[crate::core::note::Note],
    note_names: &std::collections::HashSet<String>,
) -> CheckResult {
    use std::collections::HashSet;

    let mut linked: HashSet<String> = HashSet::new();
    for note in notes {
        for link in note.wikilinks() {
            if note_names.contains(&link) {
                linked.insert(link);
            }
        }
    }

    let orphans = note_names.difference(&linked).count();
    let ratio = orphans as f64 / notes.len() as f64;

    CheckResult {
        id: "orphans".to_string(),
        name: "Orphan Notes".to_string(),
        status: if ratio < 0.3 { "pass" } else { "fail" }.to_string(),
        errors: orphans,
        details: Some(format!("{} orphan notes ({:.0}%)", orphans, ratio * 100.0)),
        files: Vec::new(),
    }
}

fn print_report(result: &AuditResult) {
    println!("{}", "Vault Full Audit Report".bold());
    println!("{}", "=".repeat(60));
    println!();
    println!("Timestamp: {}", result.timestamp);
    println!(
        "Checks: {} | Pass: {} | Warn: {} | Fail: {}",
        result.total_checks,
        result.passed.to_string().green(),
        if result.warned > 0 {
            result.warned.to_string().yellow()
        } else {
            result.warned.to_string().green()
        },
        if result.failed > 0 {
            result.failed.to_string().red()
        } else {
            result.failed.to_string().green()
        }
    );
    println!();
    println!("{}", "-".repeat(60));

    for check in &result.checks {
        let icon = match check.status.as_str() {
            "pass" => "✅",
            "warn" => "⚠️",
            "fail" => "❌",
            _ => "?",
        };
        println!(
            "{} {:<25} [{}]",
            icon,
            check.name,
            check.status.to_uppercase()
        );

        if check.status == "fail" || check.status == "warn" {
            println!("   Count: {}", check.errors);
        }
        if let Some(details) = &check.details {
            println!("   {}", details);
        }
        // Show files list for warn status (empty gist notes)
        if check.status == "warn" && !check.files.is_empty() {
            let display_files: Vec<&str> = check.files.iter().take(5).map(|s| s.as_str()).collect();
            println!("   Files: {}", display_files.join(", "));
            if check.files.len() > 5 {
                println!("   ... and {} more", check.files.len() - 5);
            }
        }
    }

    println!("{}", "-".repeat(60));
    println!();

    if result.failed == 0 && result.warned == 0 {
        println!("{}", "✅ All checks passed!".green());
    } else if result.failed == 0 {
        println!(
            "{}",
            format!(
                "⚠️  {} warning(s) - not critical, can be fixed later",
                result.warned
            )
            .yellow()
        );
    } else {
        println!(
            "{}",
            format!("⚠️  {} check(s) failed", result.failed).yellow()
        );
    }
}
