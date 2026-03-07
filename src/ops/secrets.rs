use crate::exit::{EXIT_GENERAL, ExitError};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct SecretHit {
    pub path: String,
    pub reason: String,
}

pub fn scan_run_for_secrets(run_dir: &Path) -> Result<Vec<SecretHit>, ExitError> {
    let mut hits: Vec<SecretHit> = vec![];

    let candidates = vec![run_dir.join("bundle"), run_dir.join("verify")];
    for root in candidates {
        if !root.exists() {
            continue;
        }
        for file in walk_files(&root) {
            if let Ok(meta) = fs::metadata(&file) {
                // Avoid scanning very large blobs.
                if meta.len() > 2_000_000 {
                    continue;
                }
            }

            let Ok(bytes) = fs::read(&file) else {
                continue;
            };
            let s = String::from_utf8_lossy(&bytes);

            let mut reasons = vec![];
            if contains_private_key_block(&s) {
                reasons.push("private key block");
            }
            if contains_aws_access_key_id(&s) {
                reasons.push("AWS access key id-like");
            }
            if contains_github_token(&s) {
                reasons.push("GitHub token-like");
            }
            if contains_slack_token(&s) {
                reasons.push("Slack token-like");
            }

            if reasons.is_empty() {
                continue;
            }

            let rel = file
                .strip_prefix(run_dir)
                .unwrap_or(&file)
                .display()
                .to_string();
            for r in reasons {
                hits.push(SecretHit {
                    path: rel.clone(),
                    reason: r.to_string(),
                });
            }
        }
    }

    // Deterministic output.
    hits.sort_by(|a, b| {
        (a.path.clone(), a.reason.clone()).cmp(&(b.path.clone(), b.reason.clone()))
    });
    hits.dedup_by(|a, b| a.path == b.path && a.reason == b.reason);

    Ok(hits)
}

pub fn write_secrets_report(run_dir: &Path, hits: &[SecretHit]) -> Result<(), ExitError> {
    let bytes = serde_json::to_vec_pretty(hits).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to encode secrets report: {e}"),
        )
    })?;
    fs::write(run_dir.join("secrets.json"), bytes)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write secrets.json: {e}")))?;
    Ok(())
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let Ok(rd) = fs::read_dir(root) else {
        return out;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            out.extend(walk_files(&p));
        } else if ent.file_type().map(|t| t.is_file()).unwrap_or(false) {
            out.push(p);
        }
    }
    out
}

fn contains_private_key_block(s: &str) -> bool {
    let t = s;
    t.contains("BEGIN RSA PRIVATE KEY")
        || t.contains("BEGIN OPENSSH PRIVATE KEY")
        || t.contains("BEGIN EC PRIVATE KEY")
        || t.contains("BEGIN PGP PRIVATE KEY BLOCK")
}

fn contains_aws_access_key_id(s: &str) -> bool {
    // Common AWS access key id prefixes: AKIA / ASIA
    contains_prefixed_token(s, "AKIA", 16) || contains_prefixed_token(s, "ASIA", 16)
}

fn contains_prefixed_token(s: &str, prefix: &str, len_after: usize) -> bool {
    let bytes = s.as_bytes();
    let p = prefix.as_bytes();
    if bytes.len() < p.len() + len_after {
        return false;
    }

    let mut i = 0;
    while i + p.len() + len_after <= bytes.len() {
        if &bytes[i..i + p.len()] == p {
            let after = &bytes[i + p.len()..i + p.len() + len_after];
            if after
                .iter()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn contains_github_token(s: &str) -> bool {
    // Avoid overly broad patterns; only match obvious GitHub token prefixes.
    s.contains("ghp_") || s.contains("github_pat_")
}

fn contains_slack_token(s: &str) -> bool {
    s.contains("xoxb-") || s.contains("xoxp-") || s.contains("xoxa-")
}
