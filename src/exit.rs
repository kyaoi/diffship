use std::fmt;

pub const EXIT_OK: i32 = 0;
pub const EXIT_GENERAL: i32 = 1;
pub const EXIT_NOT_GIT_REPO: i32 = 2;
pub const EXIT_LOCK_BUSY: i32 = 10;

#[derive(Debug)]
pub struct ExitError {
    pub code: i32,
    pub message: String,
}

impl ExitError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for ExitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ExitError {}
