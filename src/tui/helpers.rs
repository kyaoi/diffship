use super::EditTarget;

pub(super) fn display_opt(s: Option<&str>) -> &str {
    s.filter(|v| !v.is_empty()).unwrap_or("(auto)")
}

pub(super) fn empty_to_none(s: String) -> Option<String> {
    if s.trim().is_empty() { None } else { Some(s) }
}

pub(super) fn parse_pattern_list(s: &str) -> Vec<String> {
    s.split([',', '\n'])
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(super) fn parse_optional_usize(label: &str, s: &str) -> Result<Option<usize>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<usize>()
        .map(Some)
        .map_err(|e| format!("invalid {label}: {trimmed} ({e})"))
}

pub(super) fn parse_optional_u64(label: &str, s: &str) -> Result<Option<u64>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<u64>()
        .map(Some)
        .map_err(|e| format!("invalid {label}: {trimmed} ({e})"))
}

pub(super) fn opt_usize_to_string(value: Option<usize>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

pub(super) fn opt_u64_to_string(value: Option<u64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

pub(super) fn cycle_value(current: &str, values: &[&str]) -> String {
    let idx = values.iter().position(|v| *v == current).unwrap_or(0);
    values[(idx + 1) % values.len()].to_string()
}

pub(super) fn cycle_named_value(current: Option<&str>, values: &[String]) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let current = current.unwrap_or(values[0].as_str());
    let idx = values.iter().position(|v| v == current).unwrap_or(0);
    Some(values[(idx + 1) % values.len()].clone())
}

pub(super) fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

pub(super) fn next_edit_target(target: EditTarget, reverse: bool) -> Option<EditTarget> {
    const COMPARE_ORDER: &[EditTarget] = &[EditTarget::CompareBundleA, EditTarget::CompareBundleB];
    const ORDER: &[EditTarget] = &[
        EditTarget::HandoffFrom,
        EditTarget::HandoffTo,
        EditTarget::HandoffA,
        EditTarget::HandoffB,
        EditTarget::HandoffInclude,
        EditTarget::HandoffExclude,
        EditTarget::HandoffOut,
        EditTarget::HandoffPlanPath,
        EditTarget::HandoffMaxParts,
        EditTarget::HandoffMaxBytes,
    ];

    let order = if COMPARE_ORDER.contains(&target) {
        COMPARE_ORDER
    } else {
        ORDER
    };
    let idx = order.iter().position(|entry| *entry == target)?;
    let next = if reverse {
        idx.checked_sub(1).unwrap_or(order.len() - 1)
    } else {
        (idx + 1) % order.len()
    };
    Some(order[next])
}

#[cfg(test)]
mod tests {
    use super::{
        cycle_named_value, cycle_value, next_edit_target, parse_optional_u64, parse_optional_usize,
        parse_pattern_list,
    };
    use crate::tui::EditTarget;

    #[test]
    fn cycle_value_wraps() {
        assert_eq!(cycle_value("auto", &["auto", "file", "commit"]), "file");
        assert_eq!(cycle_value("commit", &["auto", "file", "commit"]), "auto");
    }

    #[test]
    fn cycle_named_value_wraps() {
        let values = vec!["20x512".to_string(), "10x100".to_string()];
        assert_eq!(
            cycle_named_value(Some("20x512"), &values),
            Some("10x100".to_string())
        );
        assert_eq!(
            cycle_named_value(Some("10x100"), &values),
            Some("20x512".to_string())
        );
    }

    #[test]
    fn next_edit_target_cycles_handoff_fields() {
        assert_eq!(
            next_edit_target(EditTarget::HandoffFrom, false),
            Some(EditTarget::HandoffTo)
        );
        assert_eq!(
            next_edit_target(EditTarget::HandoffMaxBytes, false),
            Some(EditTarget::HandoffFrom)
        );
        assert_eq!(
            next_edit_target(EditTarget::HandoffFrom, true),
            Some(EditTarget::HandoffMaxBytes)
        );
        assert_eq!(next_edit_target(EditTarget::LoopBundle, false), None);
    }

    #[test]
    fn next_edit_target_cycles_compare_fields() {
        assert_eq!(
            next_edit_target(EditTarget::CompareBundleA, false),
            Some(EditTarget::CompareBundleB)
        );
        assert_eq!(
            next_edit_target(EditTarget::CompareBundleB, false),
            Some(EditTarget::CompareBundleA)
        );
        assert_eq!(
            next_edit_target(EditTarget::CompareBundleA, true),
            Some(EditTarget::CompareBundleB)
        );
    }

    #[test]
    fn numeric_edit_parsers_accept_empty_and_reject_invalid_values() {
        assert_eq!(parse_optional_usize("max parts", "").unwrap(), None);
        assert_eq!(parse_optional_usize("max parts", "12").unwrap(), Some(12));
        assert!(parse_optional_usize("max parts", "abc").is_err());

        assert_eq!(parse_optional_u64("max bytes", "").unwrap(), None);
        assert_eq!(parse_optional_u64("max bytes", "1024").unwrap(), Some(1024));
        assert!(parse_optional_u64("max bytes", "oops").is_err());
    }

    #[test]
    fn parse_pattern_list_accepts_commas_and_newlines() {
        assert_eq!(
            parse_pattern_list("src/*.rs, docs/*.md\nnotes.txt"),
            vec!["src/*.rs", "docs/*.md", "notes.txt"]
        );
    }
}
