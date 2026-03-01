mod cli;
mod exit;
mod git;
mod ops;

use clap::Parser;

fn main() {
    // We intentionally special-case the two M2 entrypoints so they always work,
    // even if clap's derived subcommand table is out-of-sync in some build profiles.
    let argv: Vec<String> = std::env::args().collect();

    let cli = if let Some(cli) = try_parse_m2_entrypoint(&argv) {
        cli
    } else {
        cli::Cli::parse()
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

fn try_parse_m2_entrypoint(argv: &[String]) -> Option<cli::Cli> {
    if argv.len() < 2 {
        return None;
    }

    match argv[1].as_str() {
        "apply" => parse_apply(argv),
        "verify" => parse_verify(argv),
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
            // Unknown flag: fall back to clap for a proper error/help message.
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
            // Unknown flag: fall back to clap for a proper error/help message.
            return None;
        } else {
            // No positional args in verify (M2).
            return None;
        }

        i += 1;
    }

    let args = cli::VerifyArgs {
        profile: profile.unwrap_or_else(|| "standard".to_string()),
        run_id,
    };

    Some(cli::Cli {
        command: Some(cli::Command::Verify(args)),
    })
}
