use crate::cli::TestHoldLockArgs;
use crate::exit::{EXIT_GENERAL, EXIT_LOCK_BUSY, ExitError};
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub held: bool,
    pub pid: u32,
    pub started_at: String,
    pub released_at: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub git_root: String,
    pub version: String,
}

pub struct LockGuard {
    file: File,
    info: LockInfo,
}

impl LockGuard {
    pub fn acquire(lock_path: &Path, info: LockInfo) -> Result<Self, ExitError> {
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ExitError::new(EXIT_GENERAL, format!("failed to create lock dir: {e}"))
            })?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to open lock file: {e}")))?;

        // Non-blocking: refuse concurrent runs.
        if file.try_lock_exclusive().is_err() {
            let meta = read_lock_info(lock_path);
            let hint = if let Some(m) = meta {
                format!(
                    "lock is busy (pid={}, started_at={}, cmd={})",
                    m.pid, m.started_at, m.command
                )
            } else {
                "lock is busy".to_string()
            };
            return Err(ExitError::new(EXIT_LOCK_BUSY, hint));
        }

        let mut guard = Self { file, info };

        guard.info.held = true;
        guard.info.released_at = None;
        guard.write_info();

        Ok(guard)
    }

    fn write_info(&mut self) {
        if let Ok(bytes) = serde_json::to_vec_pretty(&self.info) {
            // Best-effort; ignore errors.
            let _ = self.file.seek(SeekFrom::Start(0));
            let _ = self.file.set_len(0);
            let _ = self.file.write_all(&bytes);
            let _ = self.file.write_all(b"\n");
            let _ = self.file.sync_all();
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        self.info.held = false;
        self.info.released_at = Some(now_rfc3339());
        self.write_info();
        let _ = self.file.unlock();
    }
}

pub fn default_lock_path(git_root: &Path) -> std::path::PathBuf {
    git_root.join(".diffship").join("lock")
}

pub fn now_rfc3339() -> String {
    let now = time::OffsetDateTime::now_utc();
    now.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

pub fn read_lock_info(lock_path: &Path) -> Option<LockInfo> {
    let mut f = File::open(lock_path).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    serde_json::from_str::<LockInfo>(&s).ok()
}

/// Returns true if another process currently holds the lock.
pub fn is_lock_held(lock_path: &Path) -> Option<bool> {
    if !lock_path.exists() {
        return Some(false);
    }

    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path)
        .ok()?;
    match f.try_lock_exclusive() {
        Ok(()) => {
            let _ = f.unlock();
            Some(false)
        }
        Err(_) => Some(true),
    }
}

pub fn make_lock_info(git_root: &Path, command: &str, args: &[String]) -> LockInfo {
    LockInfo {
        held: true,
        pid: std::process::id(),
        started_at: now_rfc3339(),
        released_at: None,
        command: command.to_string(),
        args: args.to_vec(),
        git_root: git_root.display().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

pub fn test_hold_lock(git_root: &Path, args: TestHoldLockArgs) -> Result<(), ExitError> {
    let lock_path = default_lock_path(git_root);
    let info = make_lock_info(git_root, "__test_hold_lock", &[format!("--ms={}", args.ms)]);
    let _guard = LockGuard::acquire(&lock_path, info)?;
    thread::sleep(Duration::from_millis(args.ms));
    Ok(())
}
