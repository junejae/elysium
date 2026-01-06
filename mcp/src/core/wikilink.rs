use lazy_static::lazy_static;
use regex::Regex;
use std::collections::{HashMap, HashSet};

lazy_static! {
    // [[target]] or [[target|display]]
    static ref WIKILINK_RE: Regex = Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
}

pub fn extract_wikilinks(content: &str) -> Vec<String> {
    WIKILINK_RE
        .captures_iter(content)
        .map(|c| c[1].trim().to_string())
        .collect()
}

#[derive(Debug, Default)]
pub struct WikilinkReport {
    pub total_links: usize,
    pub valid_links: usize,
    pub broken_links: usize,
    pub broken_by_file: HashMap<String, Vec<String>>,
    pub orphan_notes: Vec<String>,
}

pub fn analyze_wikilinks(
    notes: &[(String, String)],
    existing_names: &HashSet<String>,
) -> WikilinkReport {
    let mut report = WikilinkReport::default();
    let mut incoming_links: HashMap<String, usize> = HashMap::new();

    for (filename, content) in notes {
        let links = extract_wikilinks(content);
        report.total_links += links.len();

        let mut broken = Vec::new();
        for link in links {
            if existing_names.contains(&link) {
                report.valid_links += 1;
                *incoming_links.entry(link).or_insert(0) += 1;
            } else {
                broken.push(link);
            }
        }

        if !broken.is_empty() {
            report.broken_links += broken.len();
            report.broken_by_file.insert(filename.clone(), broken);
        }
    }

    for name in existing_names {
        if !incoming_links.contains_key(name) {
            report.orphan_notes.push(name.clone());
        }
    }
    report.orphan_notes.sort();

    report
}
