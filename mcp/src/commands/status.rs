use std::collections::HashMap;
use std::fs;

use anyhow::Result;
use chrono::{Duration, Local};
use colored::*;
use serde::Serialize;

use crate::core::note::collect_all_notes;
use crate::core::paths::VaultPaths;

const STALE_DAYS: i64 = 30;
const INBOX_WARN_THRESHOLD: usize = 10;

#[derive(Serialize)]
struct VaultStatus {
    timestamp: String,
    folder_counts: HashMap<String, usize>,
    total: usize,
    templates: usize,
    type_distribution: HashMap<String, usize>,
    status_distribution: HashMap<String, usize>,
    area_distribution: HashMap<String, usize>,
    inbox_memos: usize,
    stale_notes_count: usize,
    warnings: Vec<Warning>,
}

#[derive(Serialize)]
struct Warning {
    target: String,
    warning_type: String,
    message: String,
}

pub fn run(brief: bool, json: bool) -> Result<()> {
    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);

    let mut folder_counts = HashMap::new();
    folder_counts.insert("Notes".to_string(), count_files(&paths.notes));
    folder_counts.insert("Projects".to_string(), count_files(&paths.projects));
    folder_counts.insert("Archive".to_string(), count_files(&paths.archive));

    let total: usize = folder_counts.values().sum();
    let templates = count_files(&paths.templates);

    let mut type_dist: HashMap<String, usize> = HashMap::new();
    let mut status_dist: HashMap<String, usize> = HashMap::new();
    let mut area_dist: HashMap<String, usize> = HashMap::new();

    for note in &notes {
        if let Some(t) = note.note_type() {
            *type_dist.entry(t.to_string()).or_insert(0) += 1;
        }
        if let Some(s) = note.status() {
            *status_dist.entry(s.to_string()).or_insert(0) += 1;
        }
        if let Some(a) = note.area() {
            *area_dist.entry(a.to_string()).or_insert(0) += 1;
        }
    }

    let inbox_memos = count_inbox_memos(&paths.inbox);
    let stale_threshold = Local::now() - Duration::days(STALE_DAYS);
    let stale_notes: Vec<_> = notes
        .iter()
        .filter(|n| n.modified < stale_threshold)
        .collect();

    let mut warnings = Vec::new();

    if inbox_memos >= INBOX_WARN_THRESHOLD {
        warnings.push(Warning {
            target: "inbox.md".to_string(),
            warning_type: "inbox_overflow".to_string(),
            message: format!(
                "{}개 메모 누적 ({}개+ 초과)",
                inbox_memos, INBOX_WARN_THRESHOLD
            ),
        });
    }

    for note in stale_notes.iter().take(5) {
        let days = (Local::now() - note.modified).num_days();
        warnings.push(Warning {
            target: note.name.clone(),
            warning_type: "stale".to_string(),
            message: format!("{}일 미수정", days),
        });
    }

    let status = VaultStatus {
        timestamp: Local::now().to_rfc3339(),
        folder_counts,
        total,
        templates,
        type_distribution: type_dist,
        status_distribution: status_dist,
        area_distribution: area_dist,
        inbox_memos,
        stale_notes_count: stale_notes.len(),
        warnings,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        print_status(&status, brief);
    }

    if !status.warnings.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

fn count_files(path: &std::path::Path) -> usize {
    if !path.exists() {
        return 0;
    }
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
                .count()
        })
        .unwrap_or(0)
}

fn count_inbox_memos(inbox_path: &std::path::Path) -> usize {
    if !inbox_path.exists() {
        return 0;
    }
    fs::read_to_string(inbox_path)
        .map(|content| content.matches("\n---\n").count())
        .unwrap_or(0)
}

fn print_status(status: &VaultStatus, brief: bool) {
    println!("{}", "Vault Status".bold());
    println!("{}", "=".repeat(50));
    println!();
    println!("확인 시간: {}", status.timestamp);
    println!();

    println!("{}", "폴더별 노트 수".cyan());
    println!("{}", "-".repeat(30));
    for (folder, count) in &status.folder_counts {
        println!("   {:<12} {:>4}", folder, count);
    }
    println!("   {:<12} {:>4}", "Total", status.total);
    println!();

    if brief {
        println!("Status: {:?}", status.status_distribution);
        println!("Type: {:?}", status.type_distribution);
        println!("Area: {:?}", status.area_distribution);
    } else {
        print_distribution("Status 분포", &status.status_distribution, status.total);
        print_distribution("Type 분포", &status.type_distribution, status.total);
        print_distribution("Area 분포", &status.area_distribution, status.total);
    }

    if !status.warnings.is_empty() {
        println!();
        println!("{}", "⚠️  주의 필요 항목".yellow());
        println!("{}", "-".repeat(30));
        for w in &status.warnings {
            println!("   {}: {}", w.target, w.message);
        }
    }

    println!();
    println!("{}", "=".repeat(50));
    if status.templates > 0 {
        println!("*Templates/ 제외 ({}개)*", status.templates);
    }
}

fn print_distribution(title: &str, dist: &HashMap<String, usize>, total: usize) {
    println!("{}", title.cyan());
    println!("{}", "-".repeat(30));
    for (key, count) in dist {
        let pct = if total > 0 {
            (*count as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        println!("   {:<12} {:>4} ({:.0}%)", key, count, pct);
    }
    println!();
}
