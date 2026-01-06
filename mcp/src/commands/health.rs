use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::{Duration, Local};
use colored::*;
use serde::Serialize;

use crate::core::note::collect_all_notes;
use crate::core::paths::VaultPaths;

const WEIGHT_CONNECTIVITY: u32 = 25;
const WEIGHT_TAG_HEALTH: u32 = 20;
const WEIGHT_GROWTH: u32 = 20;
const WEIGHT_MAINTENANCE: u32 = 15;
const WEIGHT_SCHEMA: u32 = 20;

#[derive(Serialize)]
struct HealthResult {
    total_score: f64,
    grade: String,
    total_notes: usize,
    breakdown: HashMap<String, CategoryScore>,
}

#[derive(Serialize)]
struct CategoryScore {
    score: u32,
    weight: u32,
    details: HashMap<String, serde_json::Value>,
}

pub fn run(details: bool, json: bool) -> Result<()> {
    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);

    let mut breakdown = HashMap::new();

    let (conn_score, conn_details) = calculate_connectivity(&notes);
    breakdown.insert(
        "connectivity".to_string(),
        CategoryScore {
            score: conn_score,
            weight: WEIGHT_CONNECTIVITY,
            details: conn_details,
        },
    );

    let (tag_score, tag_details) = calculate_tag_health(&notes);
    breakdown.insert(
        "tag_health".to_string(),
        CategoryScore {
            score: tag_score,
            weight: WEIGHT_TAG_HEALTH,
            details: tag_details,
        },
    );

    let (growth_score, growth_details) = calculate_growth(&notes);
    breakdown.insert(
        "growth".to_string(),
        CategoryScore {
            score: growth_score,
            weight: WEIGHT_GROWTH,
            details: growth_details,
        },
    );

    let (maint_score, maint_details) = calculate_maintenance(&notes);
    breakdown.insert(
        "maintenance".to_string(),
        CategoryScore {
            score: maint_score,
            weight: WEIGHT_MAINTENANCE,
            details: maint_details,
        },
    );

    let (schema_score, schema_details) = calculate_schema_compliance(&notes);
    breakdown.insert(
        "schema_compliance".to_string(),
        CategoryScore {
            score: schema_score,
            weight: WEIGHT_SCHEMA,
            details: schema_details,
        },
    );

    let weighted_score: f64 = breakdown
        .values()
        .map(|c| (c.score as f64 * c.weight as f64) / 100.0)
        .sum();

    let grade = match weighted_score as u32 {
        90..=100 => "A",
        80..=89 => "B+",
        70..=79 => "B",
        60..=69 => "C",
        _ => "D",
    }
    .to_string();

    let result = HealthResult {
        total_score: (weighted_score * 10.0).round() / 10.0,
        grade,
        total_notes: notes.len(),
        breakdown,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_results(&result, details);
    }

    if result.total_score < 60.0 {
        std::process::exit(1);
    }

    Ok(())
}

fn calculate_connectivity(
    notes: &[crate::core::note::Note],
) -> (u32, HashMap<String, serde_json::Value>) {
    let note_names: HashSet<_> = notes.iter().map(|n| n.name.clone()).collect();
    let mut incoming: HashMap<String, usize> = HashMap::new();
    let mut total_outgoing = 0;

    for note in notes {
        for link in note.wikilinks() {
            if note_names.contains(&link) {
                *incoming.entry(link).or_insert(0) += 1;
                total_outgoing += 1;
            }
        }
    }

    let orphan_count = notes
        .iter()
        .filter(|n| !incoming.contains_key(&n.name))
        .count();
    let orphan_ratio = orphan_count as f64 / notes.len() as f64;
    let avg_links = total_outgoing as f64 / notes.len() as f64;

    let mut score: i32 = 100;
    if orphan_ratio > 0.3 {
        score -= 40;
    } else if orphan_ratio > 0.15 {
        score -= 20;
    }
    if avg_links < 1.0 {
        score -= 30;
    } else if avg_links < 2.0 {
        score -= 15;
    }

    let mut details = HashMap::new();
    details.insert("orphan_count".to_string(), orphan_count.into());
    details.insert(
        "orphan_ratio".to_string(),
        ((orphan_ratio * 100.0).round() / 100.0).into(),
    );
    details.insert(
        "avg_outgoing_links".to_string(),
        ((avg_links * 100.0).round() / 100.0).into(),
    );

    (score.max(0) as u32, details)
}

fn calculate_tag_health(
    notes: &[crate::core::note::Note],
) -> (u32, HashMap<String, serde_json::Value>) {
    let mut tag_counter: HashMap<String, usize> = HashMap::new();
    let mut notes_without_tags = 0;

    for note in notes {
        let tags = note.tags();
        if tags.is_empty() {
            notes_without_tags += 1;
        }
        for tag in tags {
            *tag_counter.entry(tag).or_insert(0) += 1;
        }
    }

    let unique_tags = tag_counter.len();
    let low_usage = tag_counter.values().filter(|&&c| c == 1).count();

    let mut score: i32 = 100;
    if notes_without_tags as f64 / notes.len() as f64 > 0.3 {
        score -= 25;
    }
    if unique_tags > 0 && low_usage as f64 / unique_tags as f64 > 0.5 {
        score -= 15;
    }

    let mut details = HashMap::new();
    details.insert("unique_tags".to_string(), unique_tags.into());
    details.insert("low_usage_tags".to_string(), low_usage.into());
    details.insert("notes_without_tags".to_string(), notes_without_tags.into());

    (score.max(0) as u32, details)
}

fn calculate_growth(
    notes: &[crate::core::note::Note],
) -> (u32, HashMap<String, serde_json::Value>) {
    let threshold = Local::now() - Duration::days(30);
    let recent_modified = notes.iter().filter(|n| n.modified > threshold).count();
    let recent_created = notes.iter().filter(|n| n.created > threshold).count();
    let activity_ratio = recent_modified as f64 / notes.len() as f64;

    let mut score: i32 = 100;
    if activity_ratio < 0.1 {
        score -= 40;
    } else if activity_ratio < 0.2 {
        score -= 20;
    }

    let mut details = HashMap::new();
    details.insert("recent_modified_30d".to_string(), recent_modified.into());
    details.insert("recent_created_30d".to_string(), recent_created.into());
    details.insert(
        "activity_ratio".to_string(),
        ((activity_ratio * 100.0).round() / 100.0).into(),
    );

    (score.max(0) as u32, details)
}

fn calculate_maintenance(
    notes: &[crate::core::note::Note],
) -> (u32, HashMap<String, serde_json::Value>) {
    let stale_threshold = Local::now() - Duration::days(30);
    let archive_threshold = Local::now() - Duration::days(60);

    let stale_count = notes
        .iter()
        .filter(|n| n.modified < stale_threshold)
        .count();
    let archive_candidates = notes
        .iter()
        .filter(|n| n.status() == Some("done") && n.modified < archive_threshold)
        .count();

    let stale_ratio = stale_count as f64 / notes.len() as f64;

    let mut score: i32 = 100;
    if stale_ratio > 0.5 {
        score -= 35;
    } else if stale_ratio > 0.3 {
        score -= 20;
    }
    if archive_candidates > 5 {
        score -= 15;
    }

    let mut details = HashMap::new();
    details.insert("stale_count".to_string(), stale_count.into());
    details.insert(
        "stale_ratio".to_string(),
        ((stale_ratio * 100.0).round() / 100.0).into(),
    );
    details.insert("archive_candidates".to_string(), archive_candidates.into());

    (score.max(0) as u32, details)
}

fn calculate_schema_compliance(
    notes: &[crate::core::note::Note],
) -> (u32, HashMap<String, serde_json::Value>) {
    let valid = notes.iter().filter(|n| n.gist().is_some()).count();
    let missing_gist = notes.len() - valid;
    let ratio = valid as f64 / notes.len() as f64;

    let mut details = HashMap::new();
    details.insert("valid_schema".to_string(), valid.into());
    details.insert("missing_gist".to_string(), missing_gist.into());
    details.insert(
        "compliance_ratio".to_string(),
        ((ratio * 100.0).round() / 100.0).into(),
    );

    ((ratio * 100.0) as u32, details)
}

fn print_results(result: &HealthResult, show_details: bool) {
    println!("{}", "=".repeat(50));
    println!(
        "Vault Health Score: {} ({}/100)",
        result.grade.bold(),
        result.total_score
    );
    println!("{}", "=".repeat(50));
    println!();
    println!("Total Notes: {}", result.total_notes);
    println!();
    println!("{}", "Score Breakdown:".cyan());
    println!("{}", "-".repeat(40));

    let order = [
        "connectivity",
        "tag_health",
        "growth",
        "maintenance",
        "schema_compliance",
    ];

    for key in order {
        if let Some(cat) = result.breakdown.get(key) {
            let icon = if cat.score >= 70 {
                "✅"
            } else if cat.score >= 50 {
                "⚠️"
            } else {
                "❌"
            };
            println!(
                "   {} {:<20} {:>3}/100 (weight: {}%)",
                icon, key, cat.score, cat.weight
            );

            if show_details {
                for (k, v) in &cat.details {
                    println!("      - {}: {}", k, v);
                }
            }
        }
    }

    println!();
    println!("{}", "=".repeat(50));
}
