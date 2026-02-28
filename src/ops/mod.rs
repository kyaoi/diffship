mod init;
mod lock;
mod run;
mod runs;
mod status;

use crate::cli::{Cli, Command};
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::git;

pub fn dispatch(cli: Cli) -> Result<(), ExitError> {
    let Some(cmd) = cli.command else {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "no command specified (TUI is not implemented yet); try: diffship init | status | runs",
        ));
    };

    let git_root = git::git_root()?;

    match cmd {
        Command::Init(args) => init::cmd(&git_root, args),
        Command::Status(args) => status::cmd(&git_root, args),
        Command::Runs(args) => runs::cmd(&git_root, args),
        Command::__TestHoldLock(args) => lock::test_hold_lock(&git_root, args),
    }
}
