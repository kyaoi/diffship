mod bundle_compare;
mod cli;
mod exit;
mod filter;
mod git;
mod handoff;
mod ops;
mod plan;
mod preview;
mod tui;

use clap::Parser;

fn main() {
    // We normally rely on clap's derived parsing from src/cli.rs.
    // However, integration tests (and some build profiles) may end up running a binary where the
    // clap subcommand table is stale. In that case, clap returns "unrecognized subcommand 'apply'".
    //
    // To keep the OS stable, we provide a narrow fallback for the two M2 entrypoints:
    // - diffship apply <bundle>
    // - diffship verify [--profile <p>] [--run-id <id>]
    //
    // For anything else, we keep clap's default behavior and exit with code 2.
    let argv: Vec<String> = std::env::args().collect();

    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            if let Some(cli) = try_parse_m2_fallback(&argv) {
                cli
            } else {
                // Preserve clap's UX for normal errors.
                let _ = e.print();
                std::process::exit(2);
            }
        }
    };

    let code = match ops::dispatch(cli) {
        Ok(()) => exit::EXIT_OK,
        Err(e) => {
            eprintln!("{}", e);
            e.code
        }
    };

    std::process::exit(code);
}

fn try_parse_m2_fallback(argv: &[String]) -> Option<cli::Cli> {
    if argv.len() < 2 {
        return None;
    }
    match argv[1].as_str() {
        "apply" => parse_apply(argv),
        "verify" => parse_verify(argv),
        "promote" => parse_promote(argv),
        "loop" => parse_loop(argv),
        _ => None,
    }
}

fn parse_apply(argv: &[String]) -> Option<cli::Cli> {
    // argv: [bin, "apply", <bundle>, ...flags]
    let mut session: Option<String> = None;
    let mut keep_sandbox: Option<bool> = None;
    let mut bundle: Option<String> = None;

    let mut i = 2;
    while i < argv.len() {
        let a = &argv[i];
        if a == "--session" {
            i += 1;
            if i >= argv.len() {
                return None;
            }
            session = Some(argv[i].clone());
        } else if let Some(v) = a.strip_prefix("--session=") {
            session = Some(v.to_string());
        } else if a == "--keep-sandbox" {
            keep_sandbox = Some(true);
        } else if a == "--no-keep-sandbox" {
            keep_sandbox = Some(false);
        } else if let Some(v) = a.strip_prefix("--keep-sandbox=") {
            keep_sandbox = Some(matches!(v, "1" | "true" | "yes" | "on"));
        } else if a.starts_with('-') {
            // Unknown flag: let clap handle it (caller will print clap error).
            return None;
        } else if bundle.is_none() {
            bundle = Some(a.clone());
        } else {
            // Extra positional args are unexpected.
            return None;
        }
        i += 1;
    }

    let bundle = bundle?;
    let args = cli::ApplyArgs {
        bundle,
        session: session.unwrap_or_else(|| "default".to_string()),
        keep_sandbox: keep_sandbox.unwrap_or(true),
    };

    Some(cli::Cli {
        command: Some(cli::Command::Apply(args)),
    })
}

fn parse_verify(argv: &[String]) -> Option<cli::Cli> {
    // argv: [bin, "verify", ...flags]
    let mut profile: Option<String> = None;
    let mut run_id: Option<String> = None;

    let mut i = 2;
    while i < argv.len() {
        let a = &argv[i];
        if a == "--profile" {
            i += 1;
            if i >= argv.len() {
                return None;
            }
            profile = Some(argv[i].clone());
        } else if let Some(v) = a.strip_prefix("--profile=") {
            profile = Some(v.to_string());
        } else if a == "--run-id" {
            i += 1;
            if i >= argv.len() {
                return None;
            }
            run_id = Some(argv[i].clone());
        } else if let Some(v) = a.strip_prefix("--run-id=") {
            run_id = Some(v.to_string());
        } else if a.starts_with('-') {
            return None;
        } else {
            // No positional args in verify (M2).
            return None;
        }
        i += 1;
    }

    let args = cli::VerifyArgs { profile, run_id };

    Some(cli::Cli {
        command: Some(cli::Command::Verify(args)),
    })
}

fn parse_promote(argv: &[String]) -> Option<cli::Cli> {
    // argv: [bin, "promote", ...flags]
    let mut run_id: Option<String> = None;
    let mut target_branch: Option<String> = None;
    let mut ack_secrets = false;
    let mut ack_tasks = false;
    let mut keep_sandbox = false;

    let mut i = 2;
    while i < argv.len() {
        let a = &argv[i];
        if a == "--run-id" {
            i += 1;
            if i >= argv.len() {
                return None;
            }
            run_id = Some(argv[i].clone());
        } else if let Some(v) = a.strip_prefix("--run-id=") {
            run_id = Some(v.to_string());
        } else if a == "--target-branch" {
            i += 1;
            if i >= argv.len() {
                return None;
            }
            target_branch = Some(argv[i].clone());
        } else if let Some(v) = a.strip_prefix("--target-branch=") {
            target_branch = Some(v.to_string());
        } else if a == "--ack-secrets" {
            ack_secrets = true;
        } else if a == "--ack-tasks" {
            ack_tasks = true;
        } else if a == "--keep-sandbox" {
            keep_sandbox = true;
        } else if a.starts_with('-') {
            return None;
        } else {
            // No positional args.
            return None;
        }
        i += 1;
    }

    let args = cli::PromoteArgs {
        promotion: None,
        commit_policy: None,
        run_id,
        target_branch,
        ack_secrets,
        ack_tasks,
        keep_sandbox,
    };
    Some(cli::Cli {
        command: Some(cli::Command::Promote(args)),
    })
}

fn parse_loop(argv: &[String]) -> Option<cli::Cli> {
    // argv: [bin, "loop", <bundle>, ...flags]
    let mut bundle: Option<String> = None;
    let mut session: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut target_branch: Option<String> = None;
    let mut ack_secrets = false;
    let mut ack_tasks = false;

    let mut i = 2;
    while i < argv.len() {
        let a = &argv[i];
        if a == "--session" {
            i += 1;
            if i >= argv.len() {
                return None;
            }
            session = Some(argv[i].clone());
        } else if let Some(v) = a.strip_prefix("--session=") {
            session = Some(v.to_string());
        } else if a == "--profile" {
            i += 1;
            if i >= argv.len() {
                return None;
            }
            profile = Some(argv[i].clone());
        } else if let Some(v) = a.strip_prefix("--profile=") {
            profile = Some(v.to_string());
        } else if a == "--target-branch" {
            i += 1;
            if i >= argv.len() {
                return None;
            }
            target_branch = Some(argv[i].clone());
        } else if let Some(v) = a.strip_prefix("--target-branch=") {
            target_branch = Some(v.to_string());
        } else if a == "--ack-secrets" {
            ack_secrets = true;
        } else if a == "--ack-tasks" {
            ack_tasks = true;
        } else if a.starts_with('-') {
            return None;
        } else if bundle.is_none() {
            bundle = Some(a.clone());
        } else {
            return None;
        }
        i += 1;
    }

    let bundle = bundle?;
    let args = cli::LoopArgs {
        promotion: None,
        commit_policy: None,
        bundle,
        session: session.unwrap_or_else(|| "default".to_string()),
        profile,
        target_branch,
        ack_secrets,
        ack_tasks,
    };
    Some(cli::Cli {
        command: Some(cli::Command::Loop(args)),
    })
}
