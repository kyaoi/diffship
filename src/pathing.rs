use crate::exit::{EXIT_GENERAL, ExitError};
use std::path::{Path, PathBuf};

pub fn resolve_user_path(cwd: &Path, raw: &str) -> Result<PathBuf, ExitError> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    resolve_user_path_with_home(cwd, raw, home.as_deref())
}

fn resolve_user_path_with_home(
    cwd: &Path,
    raw: &str,
    home: Option<&Path>,
) -> Result<PathBuf, ExitError> {
    if raw == "~" {
        return home
            .map(Path::to_path_buf)
            .ok_or_else(|| ExitError::new(EXIT_GENERAL, "cannot expand '~' without HOME"));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        let home =
            home.ok_or_else(|| ExitError::new(EXIT_GENERAL, "cannot expand '~/' without HOME"))?;
        return Ok(home.join(rest));
    }
    if raw.starts_with('~') {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("unsupported home shorthand path: {raw}"),
        ));
    }

    let path = PathBuf::from(raw);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(cwd.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_user_path_with_home_expands_tilde_prefix() {
        let cwd = Path::new("/work/repo");
        let home = Path::new("/home/tester");

        assert_eq!(
            resolve_user_path_with_home(cwd, "~", Some(home)).unwrap(),
            home
        );
        assert_eq!(
            resolve_user_path_with_home(cwd, "~/handoffs/out", Some(home)).unwrap(),
            home.join("handoffs/out")
        );
    }

    #[test]
    fn resolve_user_path_with_home_rejects_tilde_user_form() {
        let cwd = Path::new("/work/repo");
        let err = resolve_user_path_with_home(cwd, "~alice/out", Some(Path::new("/home/tester")))
            .unwrap_err();

        assert_eq!(err.code, EXIT_GENERAL);
        assert!(err.to_string().contains("unsupported home shorthand path"));
    }
}
