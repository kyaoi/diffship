use crate::exit::{EXIT_GENERAL, ExitError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandLogRecord {
    pub phase: String,
    pub name: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub status: i32,
    pub duration_ms: u128,
    pub stdout_path: String,
    pub stderr_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandLogIndex {
    commands: Vec<CommandLogRecord>,
}

pub struct CommandOutput {
    pub record: CommandLogRecord,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn run_and_log(
    run_dir: &Path,
    phase: &str,
    name: &str,
    cwd: &Path,
    argv: &[String],
    stdin_bytes: Option<&[u8]>,
) -> Result<CommandOutput, ExitError> {
    let out_dir = run_dir.join(phase);
    fs::create_dir_all(&out_dir).map_err(|e| {
        ExitError::new(EXIT_GENERAL, format!("failed to create {} dir: {e}", phase))
    })?;

    let stem = sanitize_name(name);
    let stdout_path = out_dir.join(format!("{}.stdout", stem));
    let stderr_path = out_dir.join(format!("{}.stderr", stem));

    let start = Instant::now();

    let (status, stdout, stderr) = if argv.is_empty() {
        (
            1,
            Vec::new(),
            b"failed to spawn command: empty argv\n".to_vec(),
        )
    } else {
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..])
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if stdin_bytes.is_some() {
            cmd.stdin(Stdio::piped());
        }

        match cmd.spawn() {
            Ok(mut child) => {
                if let Some(bytes) = stdin_bytes
                    && let Some(stdin) = child.stdin.as_mut()
                    && let Err(e) = stdin.write_all(bytes)
                {
                    let _ = child.kill();
                    (
                        1,
                        Vec::new(),
                        format!("failed to write stdin: {e}\n").into_bytes(),
                    )
                } else {
                    match child.wait_with_output() {
                        Ok(out) => (out.status.code().unwrap_or(1), out.stdout, out.stderr),
                        Err(e) => (
                            1,
                            Vec::new(),
                            format!("failed to wait for command: {e}\n").into_bytes(),
                        ),
                    }
                }
            }
            Err(e) => (
                1,
                Vec::new(),
                format!("failed to spawn command: {e}\n").into_bytes(),
            ),
        }
    };

    let duration_ms = start.elapsed().as_millis();
    fs::write(&stdout_path, &stdout)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write stdout: {e}")))?;
    fs::write(&stderr_path, &stderr)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write stderr: {e}")))?;

    let record = CommandLogRecord {
        phase: phase.to_string(),
        name: name.to_string(),
        argv: argv.to_vec(),
        cwd: cwd.display().to_string(),
        status,
        duration_ms,
        stdout_path: stdout_path
            .strip_prefix(run_dir)
            .unwrap_or(&stdout_path)
            .display()
            .to_string(),
        stderr_path: stderr_path
            .strip_prefix(run_dir)
            .unwrap_or(&stderr_path)
            .display()
            .to_string(),
    };
    append_record(run_dir, record.clone())?;

    Ok(CommandOutput {
        record,
        stdout,
        stderr,
    })
}

pub fn read_records(run_dir: &Path) -> Result<Vec<CommandLogRecord>, ExitError> {
    let index_path = run_dir.join("commands.json");
    if !index_path.is_file() {
        return Ok(vec![]);
    }
    let bytes = fs::read(&index_path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read {}: {e}", index_path.display()),
        )
    })?;
    let index = serde_json::from_slice::<CommandLogIndex>(&bytes).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to parse {}: {e}", index_path.display()),
        )
    })?;
    Ok(index.commands)
}

fn append_record(run_dir: &Path, record: CommandLogRecord) -> Result<(), ExitError> {
    let index_path = run_dir.join("commands.json");
    let mut index = if index_path.is_file() {
        let bytes = fs::read(&index_path).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to read {}: {e}", index_path.display()),
            )
        })?;
        serde_json::from_slice::<CommandLogIndex>(&bytes)
            .unwrap_or(CommandLogIndex { commands: vec![] })
    } else {
        CommandLogIndex { commands: vec![] }
    };

    index.commands.push(record);
    let bytes = serde_json::to_vec_pretty(&index).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to encode command log index: {e}"),
        )
    })?;
    fs::write(index_path, bytes)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write commands.json: {e}")))
}

pub fn sanitize_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
