use predicates::prelude::*;
use predicates::str::contains;
use std::time::Duration;

#[test]
fn no_args_non_tty_prints_help_and_exits_quickly() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.timeout(Duration::from_secs(2));

    cmd.assert()
        .success()
        // clap help should be printed on stdout
        .stdout(
            contains("Commands")
                .or(contains("USAGE"))
                .or(contains("Usage")),
        );
}

#[test]
fn tui_subcommand_requires_tty_in_tests() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.arg("tui");
    cmd.timeout(Duration::from_secs(2));

    cmd.assert().failure().stderr(contains("requires a TTY"));
}
