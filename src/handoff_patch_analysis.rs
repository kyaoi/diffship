use std::collections::BTreeSet;

pub(crate) fn extract_hunk_headers(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("@@ ")?;
            let (_, suffix) = rest.split_once(" @@")?;
            let suffix = suffix.trim();
            if suffix.is_empty() {
                None
            } else {
                Some(suffix.to_string())
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn collect_changed_patch_lines(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter(|line| {
            (line.starts_with('+') || line.starts_with('-'))
                && !line.starts_with("+++")
                && !line.starts_with("---")
        })
        .map(|line| line[1..].trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

pub(crate) fn extract_symbol_like_names(
    hunk_headers: &[String],
    changed_lines: &[String],
) -> Vec<String> {
    hunk_headers
        .iter()
        .chain(changed_lines.iter())
        .flat_map(|line| extract_symbol_like_names_from_line(line))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn extract_import_like_refs(changed_lines: &[String]) -> Vec<String> {
    changed_lines
        .iter()
        .filter_map(|line| normalize_import_like_ref(line))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn is_signature_change_like_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    [
        "pub fn ",
        "fn ",
        "pub struct ",
        "struct ",
        "pub enum ",
        "enum ",
        "pub trait ",
        "trait ",
        "impl ",
        "function ",
        "export function ",
        "class ",
        "export class ",
        "interface ",
        "type ",
        "export type ",
        "def ",
        "async def ",
        "func ",
    ]
    .iter()
    .any(|needle| trimmed.starts_with(needle))
}

pub(crate) fn is_api_surface_like_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("pub ")
        || trimmed.starts_with("export ")
        || trimmed.starts_with("interface ")
        || trimmed.starts_with("type ")
        || trimmed.starts_with("class ")
        || trimmed.starts_with("trait ")
    {
        return true;
    }
    trimmed.starts_with("def ") && !trimmed.starts_with("def _")
}

fn extract_symbol_like_names_from_line(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let mut symbols = BTreeSet::new();

    for keyword in [
        "fn ",
        "struct ",
        "enum ",
        "trait ",
        "impl ",
        "mod ",
        "function ",
        "class ",
        "interface ",
        "type ",
        "def ",
        "func ",
    ] {
        if let Some(name) = identifier_after_keyword(trimmed, keyword) {
            symbols.insert(name);
        }
    }

    if trimmed.contains("=>")
        && let Some(name) = identifier_after_keyword(trimmed, "const ")
            .or_else(|| identifier_after_keyword(trimmed, "let "))
            .or_else(|| identifier_after_keyword(trimmed, "var "))
    {
        symbols.insert(name);
    }

    symbols.into_iter().collect()
}

fn identifier_after_keyword(line: &str, keyword: &str) -> Option<String> {
    let idx = line.find(keyword)?;
    let rest = &line[idx + keyword.len()..];
    let candidate = rest
        .trim_start_matches(|c: char| c.is_whitespace() || c == '(' || c == '<')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == ':' || *c == '.')
        .collect::<String>();
    normalize_symbol_candidate(&candidate)
}

fn normalize_symbol_candidate(candidate: &str) -> Option<String> {
    let trimmed =
        candidate.trim_matches(|c: char| c == ':' || c == '.' || c == '<' || c == '>' || c == '(');
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "if" || trimmed == "for" || trimmed == "while" || trimmed == "match" {
        return None;
    }
    Some(trimmed.to_string())
}

fn normalize_import_like_ref(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let is_import_like = trimmed.starts_with("use ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("mod ")
        || trimmed.starts_with("#include")
        || trimmed.contains("require(");
    if !is_import_like {
        return None;
    }

    Some(
        trimmed
            .trim_end_matches(';')
            .trim_end_matches('{')
            .trim()
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_analysis_extracts_headers_symbols_and_imports() {
        let patch = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,7 @@ pub fn old_name() -> i32 {
-use crate::old_dep;
+use crate::new_dep;
+import { helper } from "./helper";
+const makeValue = () => helper();
+fn new_name() -> i32 {
  1
 }
"#;

        let headers = extract_hunk_headers(patch);
        assert_eq!(headers, vec!["pub fn old_name() -> i32 {".to_string()]);

        let changed_lines = collect_changed_patch_lines(patch);
        assert!(changed_lines.contains(&"use crate::new_dep;".to_string()));
        assert!(changed_lines.contains(&"const makeValue = () => helper();".to_string()));

        let symbols = extract_symbol_like_names(&headers, &changed_lines);
        assert!(symbols.contains(&"old_name".to_string()));
        assert!(symbols.contains(&"new_name".to_string()));
        assert!(symbols.contains(&"makeValue".to_string()));

        let imports = extract_import_like_refs(&changed_lines);
        assert_eq!(
            imports,
            vec![
                "import { helper } from \"./helper\"".to_string(),
                "use crate::new_dep".to_string(),
                "use crate::old_dep".to_string(),
            ]
        );
    }

    #[test]
    fn patch_analysis_classifies_signature_and_api_surface_lines() {
        assert!(is_signature_change_like_line(
            "pub fn value(input: i32) -> i32 {"
        ));
        assert!(is_signature_change_like_line("interface ResultShape {"));
        assert!(!is_signature_change_like_line("let value = 1;"));

        assert!(is_api_surface_like_line("pub struct Value {"));
        assert!(is_api_surface_like_line("export function buildThing() {"));
        assert!(is_api_surface_like_line("def render(request):"));
        assert!(!is_api_surface_like_line("def _helper(request):"));
    }
}
