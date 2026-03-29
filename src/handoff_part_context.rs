#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CategoryCounts {
    pub(crate) docs: usize,
    pub(crate) config: usize,
    pub(crate) source: usize,
    pub(crate) tests: usize,
    pub(crate) other: usize,
}

pub(crate) fn title(part_name: &str, file_paths: &[String], counts: CategoryCounts) -> String {
    if let Some(path) = file_paths.first()
        && file_paths.len() == 1
    {
        return format!("{part_name}: {path}");
    }
    if file_paths.is_empty() {
        format!("{part_name}: no file changes")
    } else {
        format!(
            "{part_name}: {} files in {}",
            file_paths.len(),
            primary_category_label(counts)
        )
    }
}

pub(crate) fn summary(segments: &[String], counts: CategoryCounts, file_count: usize) -> String {
    if file_count == 0 {
        return "This part contains no file-level changes.".to_string();
    }
    format!(
        "This part updates {} across {} segment(s): {}.",
        summarize_category_counts(counts),
        segments.len(),
        segments.join(", ")
    )
}

pub(crate) fn intent(counts: CategoryCounts) -> String {
    if category_total(counts) == 0 {
        "Primary area: no file-level changes were recorded for this part.".to_string()
    } else {
        format!("Primary area: {} changes.", primary_category_label(counts))
    }
}

pub(crate) fn acceptance_criteria(
    part_name: &str,
    file_paths: &[String],
    reduced_context: bool,
) -> Vec<String> {
    let mut items = vec![
        format!(
            "Apply or review `parts/{part_name}` as the canonical change payload for this part."
        ),
        "Keep edits scoped to the listed files unless a new handoff bundle expands the scope."
            .to_string(),
    ];
    if file_paths.is_empty() {
        items.push(
            "Confirm whether this no-op part can be ignored or removed in a future build."
                .to_string(),
        );
    }
    if reduced_context {
        items.push(
            "Reduced diff context is present; review affected paths carefully before editing further."
                .to_string(),
        );
    }
    items
}

fn category_total(counts: CategoryCounts) -> usize {
    counts.docs + counts.config + counts.source + counts.tests + counts.other
}

fn primary_category_label(counts: CategoryCounts) -> &'static str {
    [
        ("documentation", counts.docs),
        ("config/tooling", counts.config),
        ("source", counts.source),
        ("tests", counts.tests),
        ("other", counts.other),
    ]
    .into_iter()
    .max_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(b.0)))
    .map(|(label, _)| label)
    .unwrap_or("other")
}

fn summarize_category_counts(counts: CategoryCounts) -> String {
    let mut items = Vec::new();
    for (label, n) in [
        ("documentation file", counts.docs),
        ("config/tooling file", counts.config),
        ("source file", counts.source),
        ("test file", counts.tests),
        ("other file", counts.other),
    ] {
        if n == 0 {
            continue;
        }
        let suffix = if n == 1 { "" } else { "s" };
        items.push(format!("{n} {label}{suffix}"));
    }
    if items.is_empty() {
        "0 files".to_string()
    } else {
        items.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts() -> CategoryCounts {
        CategoryCounts {
            docs: 1,
            config: 0,
            source: 2,
            tests: 0,
            other: 0,
        }
    }

    #[test]
    fn title_prefers_single_file_when_possible() {
        let file_paths = vec!["src/lib.rs".to_string()];
        assert_eq!(
            title("part_01.patch", &file_paths, counts()),
            "part_01.patch: src/lib.rs"
        );
    }

    #[test]
    fn summary_and_intent_use_category_counts() {
        let segments = vec!["committed".to_string(), "staged".to_string()];
        assert_eq!(
            summary(&segments, counts(), 3),
            "This part updates 1 documentation file, 2 source files across 2 segment(s): committed, staged."
        );
        assert_eq!(intent(counts()), "Primary area: source changes.");
    }

    #[test]
    fn acceptance_criteria_mentions_empty_and_reduced_context() {
        let items = acceptance_criteria("part_01.patch", &[], true);
        assert!(items.iter().any(|item| item.contains("no-op part")));
        assert!(
            items
                .iter()
                .any(|item| item.contains("Reduced diff context"))
        );
    }
}
