mod apply;
mod config;
mod init;
pub(crate) mod lock;
mod loop_cmd;
mod pack_fix;
mod patch_bundle;
mod promote;
pub(crate) mod run;
mod runs;
mod secrets;
pub(crate) mod session;
mod status;
pub(crate) mod tasks;
mod verify;
pub(crate) mod worktree;

use crate::cli::{Cli, Command};
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::git;
use clap::CommandFactory;

pub fn dispatch(cli: Cli) -> Result<(), ExitError> {
    let Some(cmd) = cli.command else {
        if crate::tui::should_start_tui() {
            let git_root = git::git_root()?;
            return crate::tui::run(&git_root);
        }

        // Non-TTY (or explicitly disabled): preserve the classic CLI behavior and just show help.
        let mut c = crate::cli::Cli::command();
        let _ = c.print_help();
        println!();
        return Ok(());
    };

    let git_root = git::git_root()?;

    match cmd {
        Command::Tui(_args) => {
            if !crate::tui::is_tty() {
                return Err(ExitError::new(
                    EXIT_GENERAL,
                    "diffship tui requires a TTY (try running it in an interactive terminal)",
                ));
            }
            crate::tui::run(&git_root)
        }
        Command::Preview(args) => crate::preview::cmd(args),
        Command::Compare(args) => crate::bundle_compare::cmd(args),
        Command::Build(args) => crate::handoff::cmd(&git_root, args),
        Command::Init(args) => init::cmd(&git_root, args),
        Command::Status(args) => status::cmd(&git_root, args),
        Command::Runs(args) => runs::cmd(&git_root, args),
        Command::Apply(args) => apply::cmd(&git_root, args),
        Command::Verify(args) => verify::cmd(&git_root, args),
        Command::PackFix(args) => pack_fix::cmd(&git_root, args),
        Command::Promote(args) => promote::cmd(&git_root, args),
        Command::Loop(args) => loop_cmd::cmd(&git_root, args),
        Command::__TestHoldLock(args) => lock::test_hold_lock(&git_root, args),
        Command::__TestM1Setup(args) => worktree::test_m1_setup(&git_root, args),
        Command::__TestM1AdvanceSession(args) => session::test_m1_advance_session(&git_root, args),
        Command::__TestM1CleanupSandbox(args) => worktree::test_m1_cleanup_sandbox(&git_root, args),
    }
}
