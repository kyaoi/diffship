pub(crate) fn row_part_name(part: &str) -> Option<String> {
    match part.trim() {
        "" | "-" => None,
        other => Some(other.to_string()),
    }
}

pub(crate) fn nonempty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

pub(crate) fn part_context_path(part_name: &str) -> String {
    let stem = part_name.strip_suffix(".patch").unwrap_or(part_name);
    format!("parts/{stem}.context.json")
}

pub(crate) fn handoff_context_xml_path() -> &'static str {
    "handoff.context.xml"
}

pub(crate) fn sum_opt<I>(vals: I) -> Option<u64>
where
    I: IntoIterator<Item = Option<u64>>,
{
    let mut seen = false;
    let mut sum = 0_u64;
    for v in vals.into_iter().flatten() {
        seen = true;
        sum += v;
    }
    if seen { Some(sum) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_names_and_strings_are_normalized() {
        assert_eq!(
            row_part_name("part_01.patch").as_deref(),
            Some("part_01.patch")
        );
        assert_eq!(row_part_name("-"), None);
        assert_eq!(nonempty("value").as_deref(), Some("value"));
        assert_eq!(nonempty(""), None);
    }

    #[test]
    fn context_paths_and_optional_sums_are_stable() {
        assert_eq!(
            part_context_path("part_01.patch"),
            "parts/part_01.context.json"
        );
        assert_eq!(handoff_context_xml_path(), "handoff.context.xml");
        assert_eq!(sum_opt([Some(2), None, Some(3)]), Some(5));
        assert_eq!(sum_opt([None, None]), None);
    }
}
