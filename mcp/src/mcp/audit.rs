//! Audit check implementations for vault policy compliance

use std::collections::HashSet;

use crate::core::note::Note;
use crate::core::schema::SchemaValidator;

use super::types::{AuditCheckJson, AuditErrorJson};

/// Schema validation check
pub fn check_schema(
    notes: &[Note],
    validator: &SchemaValidator,
    schema_config: &crate::core::config::SchemaConfig,
    verbose: bool,
) -> AuditCheckJson {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for note in notes {
        let violations = note.validate_schema_with_config(validator);
        for violation in violations {
            let entry = AuditErrorJson {
                note: note.name.clone(),
                message: violation.format_with_config(schema_config),
            };

            if violation.is_warning() {
                warnings.push(entry);
            } else {
                errors.push(entry);
            }
        }
    }

    // Status: pass if no errors (warnings don't fail the check)
    let status = if errors.is_empty() {
        if warnings.is_empty() {
            "pass"
        } else {
            "warn"
        }
    } else {
        "fail"
    };

    AuditCheckJson {
        id: "schema".to_string(),
        name: "YAML Schema".to_string(),
        status: status.to_string(),
        errors: errors.len(),
        warnings: if warnings.is_empty() {
            None
        } else {
            Some(warnings.len())
        },
        details: if !warnings.is_empty() {
            Some(format!(
                "{} errors, {} warnings",
                errors.len(),
                warnings.len()
            ))
        } else {
            None
        },
        error_list: if verbose && !errors.is_empty() {
            Some(errors)
        } else {
            None
        },
        warning_list: if verbose && !warnings.is_empty() {
            Some(warnings)
        } else {
            None
        },
    }
}

/// Wikilinks validation check
pub fn check_wikilinks(
    notes: &[Note],
    note_names: &HashSet<String>,
    verbose: bool,
) -> AuditCheckJson {
    let mut errors = Vec::new();
    for note in notes {
        for link in note.wikilinks() {
            if !note_names.contains(&link) {
                errors.push(AuditErrorJson {
                    note: note.name.clone(),
                    message: format!("Broken link: [[{}]]", link),
                });
            }
        }
    }

    AuditCheckJson {
        id: "wikilinks".to_string(),
        name: "Wikilinks".to_string(),
        status: if errors.is_empty() { "pass" } else { "fail" }.to_string(),
        errors: errors.len(),
        warnings: None,
        details: None,
        error_list: if verbose && !errors.is_empty() {
            Some(errors)
        } else {
            None
        },
        warning_list: None,
    }
}

/// Gist coverage check
pub fn check_gist(notes: &[Note], verbose: bool) -> AuditCheckJson {
    let mut errors = Vec::new();
    for note in notes {
        if note.gist().is_none() {
            errors.push(AuditErrorJson {
                note: note.name.clone(),
                message: "Missing gist".to_string(),
            });
        }
    }

    let total = notes.len();
    let missing = errors.len();
    let coverage = if total > 0 {
        ((total - missing) as f64 / total as f64 * 100.0).round() as usize
    } else {
        100
    };

    AuditCheckJson {
        id: "gist".to_string(),
        name: "Gist Coverage".to_string(),
        status: if missing == 0 { "pass" } else { "fail" }.to_string(),
        errors: missing,
        warnings: None,
        details: Some(format!("{}% coverage ({} missing)", coverage, missing)),
        error_list: if verbose && !errors.is_empty() {
            Some(errors)
        } else {
            None
        },
        warning_list: None,
    }
}

/// Tag usage check
pub fn check_tags(notes: &[Note], verbose: bool) -> AuditCheckJson {
    let mut errors = Vec::new();
    for note in notes {
        if note.tags().is_empty() {
            errors.push(AuditErrorJson {
                note: note.name.clone(),
                message: "No tags".to_string(),
            });
        }
    }

    let total = notes.len();
    let without_tags = errors.len();
    let ratio = if total > 0 {
        without_tags as f64 / total as f64
    } else {
        0.0
    };

    AuditCheckJson {
        id: "tags".to_string(),
        name: "Tag Usage".to_string(),
        status: if ratio < 0.3 { "pass" } else { "fail" }.to_string(),
        errors: without_tags,
        warnings: None,
        details: Some(format!("{:.0}% notes without tags", ratio * 100.0)),
        error_list: if verbose && !errors.is_empty() {
            Some(errors)
        } else {
            None
        },
        warning_list: None,
    }
}

/// Orphan notes check
pub fn check_orphans(
    notes: &[Note],
    note_names: &HashSet<String>,
    verbose: bool,
) -> AuditCheckJson {
    let mut linked: HashSet<String> = HashSet::new();
    for note in notes {
        for link in note.wikilinks() {
            if note_names.contains(&link) {
                linked.insert(link);
            }
        }
    }

    let mut errors = Vec::new();
    for name in note_names {
        if !linked.contains(name) {
            errors.push(AuditErrorJson {
                note: name.clone(),
                message: "Orphan note (no incoming links)".to_string(),
            });
        }
    }

    let total = notes.len();
    let orphans = errors.len();
    let ratio = if total > 0 {
        orphans as f64 / total as f64
    } else {
        0.0
    };

    AuditCheckJson {
        id: "orphans".to_string(),
        name: "Orphan Notes".to_string(),
        status: if ratio < 0.3 { "pass" } else { "fail" }.to_string(),
        errors: orphans,
        warnings: None,
        details: Some(format!("{} orphan notes ({:.0}%)", orphans, ratio * 100.0)),
        error_list: if verbose && !errors.is_empty() {
            Some(errors)
        } else {
            None
        },
        warning_list: None,
    }
}

/// Stale gists check
pub fn check_stale_gists(notes: &[Note], verbose: bool) -> AuditCheckJson {
    let mut errors = Vec::new();
    let gist_date_re = regex::Regex::new(r"(?m)^elysium_gist_date:\s*(\d{4}-\d{2}-\d{2})").unwrap();

    for note in notes {
        let gist_date = note
            .frontmatter
            .as_ref()
            .and_then(|fm| gist_date_re.captures(&fm.raw))
            .and_then(|caps| caps.get(1))
            .and_then(|m| chrono::NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d").ok());

        if let Some(gist_date) = gist_date {
            if let Ok(metadata) = std::fs::metadata(&note.path) {
                if let Ok(modified) = metadata.modified() {
                    let modified_date =
                        chrono::DateTime::<chrono::Local>::from(modified).date_naive();
                    if gist_date < modified_date {
                        errors.push(AuditErrorJson {
                            note: note.name.clone(),
                            message: format!("Stale gist: {} < {}", gist_date, modified_date),
                        });
                    }
                }
            }
        }
    }

    AuditCheckJson {
        id: "stale_gists".to_string(),
        name: "Stale Gists".to_string(),
        status: if errors.is_empty() { "pass" } else { "warn" }.to_string(),
        errors: errors.len(),
        warnings: None,
        details: Some(format!("{} notes with outdated gists", errors.len())),
        error_list: if verbose && !errors.is_empty() {
            Some(errors)
        } else {
            None
        },
        warning_list: None,
    }
}
