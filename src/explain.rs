use crate::cli::ExplainArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::run;
use crate::ops::strategy::{self, StrategyResolution};
use crate::pathing::resolve_user_path;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(tag = "target", rename_all = "snake_case")]
enum ExplainPayload {
    Run(Box<RunExplain>),
    Bundle(BundleExplain),
}

#[derive(Debug, Serialize)]
struct RunExplain {
    run_id: String,
    command: String,
    created_at: String,
    run_dir: String,
    state_label: Option<String>,
    failure_category: Option<String>,
    next_command: Option<String>,
    summary_files: Vec<String>,
    strategy: Option<StrategyResolution>,
}

#[derive(Debug, Serialize)]
struct BundleExplain {
    bundle: String,
    current_head: Option<String>,
    workflow_profile: Option<String>,
    part_count: usize,
    task_group_count: usize,
    project_context: bool,
    reading_order: Vec<String>,
    next_read: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RunMetaLite {
    run_id: String,
    created_at: String,
    command: String,
}

pub fn cmd(git_root: &Path, args: ExplainArgs) -> Result<(), ExitError> {
    let payload = if let Some(bundle) = args.bundle.as_deref() {
        ExplainPayload::Bundle(explain_bundle(bundle)?)
    } else {
        ExplainPayload::Run(Box::new(explain_run(git_root, args.run_id.as_deref())?))
    };

    if args.json {
        let json = serde_json::to_string_pretty(&payload)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode json: {e}")))?;
        println!("{json}");
        return Ok(());
    }

    match payload {
        ExplainPayload::Run(run) => print_run_explain(&run),
        ExplainPayload::Bundle(bundle) => print_bundle_explain(&bundle),
    }
    Ok(())
}

fn explain_run(git_root: &Path, requested_run_id: Option<&str>) -> Result<RunExplain, ExitError> {
    let run_id = match requested_run_id {
        Some(run_id) => run_id.to_string(),
        None => run::latest_run_id(git_root)?.ok_or_else(|| {
            ExitError::new(
                EXIT_GENERAL,
                "no runs found (run diffship apply first, or pass --run-id)",
            )
        })?,
    };
    let summary = run::list_runs(git_root, usize::MAX)?
        .into_iter()
        .find(|summary| summary.run_id == run_id)
        .ok_or_else(|| ExitError::new(EXIT_GENERAL, format!("run not found: {run_id}")))?;
    let run_dir = run::run_dir(git_root, &run_id);
    let meta_bytes = fs::read(run_dir.join("run.json"))
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read run.json: {e}")))?;
    let meta = serde_json::from_slice::<RunMetaLite>(&meta_bytes)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("invalid run.json: {e}")))?;
    let strategy = strategy::resolve_for_run(git_root, &run_dir)?;

    Ok(RunExplain {
        run_id: meta.run_id,
        command: meta.command,
        created_at: meta.created_at,
        run_dir: summary.run_dir,
        state_label: summary.state_label,
        failure_category: summary.failure_category,
        next_command: summary.next_command,
        summary_files: [
            "apply.json",
            "verify.json",
            "promotion.json",
            "commands.json",
        ]
        .into_iter()
        .filter_map(|name| {
            let path = run_dir.join(name);
            path.exists().then(|| path.display().to_string())
        })
        .collect(),
        strategy,
    })
}

fn explain_bundle(bundle: &str) -> Result<BundleExplain, ExitError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to detect current dir: {e}")))?;
    let bundle_path = resolve_user_path(&cwd, bundle)?;
    let summary = crate::preview::load_bundle_summary_value(&bundle_path)?;
    let manifest = crate::preview::load_bundle_entry_text(&bundle_path, "handoff.manifest.json")
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let workflow = crate::preview::load_bundle_entry_text(&bundle_path, "workflow.context.json")
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());

    let parts = summary
        .get("parts")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let reading_order = summary
        .get("structured_context")
        .and_then(|value| value.get("reading_order"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let next_read = if reading_order.is_empty() {
        let mut defaults = vec!["HANDOFF.md".to_string()];
        if let Some(first_part) = parts.first().and_then(|value| value.as_str()) {
            defaults.push(first_part.to_string());
        }
        defaults
    } else {
        reading_order.iter().take(4).cloned().collect()
    };

    Ok(BundleExplain {
        bundle: bundle_path.display().to_string(),
        current_head: manifest
            .as_ref()
            .and_then(|value| value.get("current_head"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        workflow_profile: workflow
            .as_ref()
            .and_then(|value| value.get("profile"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        part_count: parts.len(),
        task_group_count: summary
            .get("structured_context")
            .and_then(|value| value.get("task_groups"))
            .and_then(|value| value.as_array())
            .map(|items| items.len())
            .unwrap_or(0),
        project_context: summary
            .get("structured_context")
            .and_then(|value| value.get("project_context"))
            .is_some(),
        reading_order,
        next_read,
    })
}

fn print_run_explain(run: &RunExplain) {
    println!("diffship explain");
    println!("target: run");
    println!("run_id: {}", run.run_id);
    println!("command: {}", run.command);
    println!("created_at: {}", run.created_at);
    println!("run_dir: {}", run.run_dir);
    println!(
        "state: {}",
        run.state_label.as_deref().unwrap_or("(unknown)")
    );
    if let Some(category) = run.failure_category.as_deref() {
        println!("failure_category: {category}");
    }
    if let Some(next) = run.next_command.as_deref() {
        println!("next: {next}");
    }
    if let Some(strategy) = run.strategy.as_ref() {
        println!("strategy: {}", strategy.selected_profile);
        if !strategy.alternatives.is_empty() {
            println!("alternatives: {}", strategy.alternatives.join(", "));
        }
    }
    if run.summary_files.is_empty() {
        println!("inspect: (no summary files)");
    } else {
        println!("inspect: {}", run.summary_files.join(", "));
    }
}

fn print_bundle_explain(bundle: &BundleExplain) {
    println!("diffship explain");
    println!("target: bundle");
    println!("bundle: {}", bundle.bundle);
    if let Some(head) = bundle.current_head.as_deref() {
        println!("current_head: {head}");
    }
    if let Some(profile) = bundle.workflow_profile.as_deref() {
        println!("workflow_profile: {profile}");
    }
    println!("parts: {}", bundle.part_count);
    println!("task_groups: {}", bundle.task_group_count);
    println!(
        "project_context: {}",
        if bundle.project_context { "yes" } else { "no" }
    );
    if bundle.next_read.is_empty() {
        println!("next_read: (none)");
    } else {
        println!("next_read: {}", bundle.next_read.join(" -> "));
    }
}
