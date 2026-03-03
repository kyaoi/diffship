mod apply;
mod config;
mod init;
mod lock;
mod loop_cmd;
mod pack_fix;
mod patch_bundle;
mod promote;
mod run;
mod runs;
mod secrets;
mod session;
mod status;
mod tasks;
mod verify;
mod worktree;

use crate::cli::{Cli, Command};
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::git;

pub fn dispatch(cli: Cli) -> Result<(), ExitError> {
    let Some(cmd) = cli.command else {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "no command specified (TUI is not implemented yet); try: diffship init | status | runs | apply | verify | promote | loop",
        ));
    };

    let git_root = git::git_root()?;

    match cmd {
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
