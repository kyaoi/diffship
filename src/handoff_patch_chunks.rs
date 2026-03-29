use std::collections::BTreeSet;

pub(crate) fn patch_chunk_path(chunk: &str) -> Option<String> {
    let first = chunk.lines().next()?;
    let rest = first.strip_prefix("diff --git ")?;
    let mut parts = rest.split_whitespace();
    let a = parts.next()?.strip_prefix("a/").unwrap_or("");
    let b = parts.next()?.strip_prefix("b/").unwrap_or("");
    let path = if b.is_empty() { a } else { b };
    Some(path.to_string())
}

pub(crate) fn collect_patch_chunks(patch: &str) -> Vec<(String, String)> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in patch.lines() {
        if line.starts_with("diff --git ") {
            if !current.is_empty() {
                if let Some(path) = patch_chunk_path(&current) {
                    chunks.push((path, current.clone()));
                }
                current.clear();
            }
            current.push_str(line);
            continue;
        }

        if current.is_empty() {
            continue;
        }
        current.push('\n');
        current.push_str(line);
    }

    if !current.is_empty()
        && let Some(path) = patch_chunk_path(&current)
    {
        chunks.push((path, current));
    }

    chunks
}

pub(crate) fn merge_unique_sorted(existing: &[String], incoming: &[String]) -> Vec<String> {
    existing
        .iter()
        .chain(incoming.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_chunk_helpers_extract_paths_and_chunks() {
        let patch = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1 @@
-old
+new
diff --git a/tests/lib_test.rs b/tests/lib_test.rs
index 3333333..4444444 100644
--- a/tests/lib_test.rs
+++ b/tests/lib_test.rs
@@ -1 +1 @@
-before
+after
"#;

        let chunks = collect_patch_chunks(patch);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, "src/lib.rs");
        assert_eq!(chunks[1].0, "tests/lib_test.rs");
        assert_eq!(
            patch_chunk_path(&chunks[0].1).as_deref(),
            Some("src/lib.rs")
        );
    }

    #[test]
    fn patch_chunk_helpers_merge_unique_sorted_strings() {
        let merged = merge_unique_sorted(
            &["b".to_string(), "a".to_string()],
            &["c".to_string(), "a".to_string()],
        );
        assert_eq!(
            merged,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
