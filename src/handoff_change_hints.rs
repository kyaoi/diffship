#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangeHintFields {
    pub(crate) new_file: bool,
    pub(crate) deleted_file: bool,
    pub(crate) rename_or_copy: bool,
    pub(crate) previous_path: Option<String>,
    pub(crate) stored_as_attachment: bool,
    pub(crate) excluded: bool,
    pub(crate) reduced_context: bool,
}

pub(crate) fn derive(status: &str, note: &str, part: &str) -> ChangeHintFields {
    let previous_path = previous_path_from_note(note);
    ChangeHintFields {
        new_file: status == "A" && previous_path.is_none(),
        deleted_file: status == "D",
        rename_or_copy: previous_path.is_some(),
        previous_path,
        stored_as_attachment: stored_as_attachment(note, part),
        excluded: excluded_from_bundle(note, part),
        reduced_context: note_has_reduced_context(note),
    }
}

pub(crate) fn note_has_reduced_context(note: &str) -> bool {
    note.contains("packing fallback reduced diff context to U")
}

fn previous_path_from_note(note: &str) -> Option<String> {
    note.trim().strip_prefix("from ").map(ToOwned::to_owned)
}

fn stored_as_attachment(note: &str, part: &str) -> bool {
    part == "attachments.zip" || note.contains("stored in attachments.zip")
}

fn excluded_from_bundle(note: &str, part: &str) -> bool {
    part == "-" || note.contains("see excluded.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_classifies_rename_attachment_exclusion_and_reduced_context() {
        let renamed = derive("R", "from old.txt", "part_01.patch");
        assert!(renamed.rename_or_copy);
        assert_eq!(renamed.previous_path.as_deref(), Some("old.txt"));
        assert!(!renamed.new_file);

        let attachment = derive("A", "stored in attachments.zip", "attachments.zip");
        assert!(attachment.stored_as_attachment);
        assert!(attachment.new_file);

        let excluded = derive("A", "excluded (meta only; see excluded.md)", "-");
        assert!(excluded.excluded);
        assert!(excluded.new_file);

        let reduced = derive(
            "M",
            "packing fallback reduced diff context to U0",
            "part_01.patch",
        );
        assert!(reduced.reduced_context);
        assert!(!reduced.excluded);
    }
}
