use std::fmt;

pub const EXIT_OK: i32 = 0;
pub const EXIT_GENERAL: i32 = 1;
pub const EXIT_NOT_GIT_REPO: i32 = 2;
#[allow(dead_code)]
pub const EXIT_PACKING_LIMITS: i32 = 3;
pub const EXIT_SECRETS_WARNING: i32 = 4;

// Ops-specific codes (see docs/SPEC_V1.md)
pub const EXIT_DIRTY_WORKTREE: i32 = 5;
pub const EXIT_BASE_COMMIT_MISMATCH: i32 = 6;
pub const EXIT_FORBIDDEN_PATH: i32 = 7;
pub const EXIT_APPLY_FAILED: i32 = 8;
pub const EXIT_VERIFY_FAILED: i32 = 9;
pub const EXIT_LOCK_BUSY: i32 = 10;

// Additional ops-specific codes (see docs/SPEC_V1.md)
pub const EXIT_SECRETS_ACK_REQUIRED: i32 = 11;
#[allow(dead_code)]
pub const EXIT_TASKS_ACK_REQUIRED: i32 = 12;
pub const EXIT_PROMOTION_FAILED: i32 = 13;

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
