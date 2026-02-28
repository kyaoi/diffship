use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "diffship")]
#[command(version)]
#[command(about = "diffship: AI-assisted development OS for Git repos")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate a ChatGPT Project kit under .diffship/
    Init(InitArgs),

    /// Show lock state and recent runs
    Status(StatusArgs),

    /// List recent runs
    Runs(RunsArgs),

    /// Internal test helper: acquire the lock and hold it for a duration.
    #[command(name = "__test_hold_lock", hide = true)]
    __TestHoldLock(TestHoldLockArgs),
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Overwrite existing files under .diffship/
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Emit machine-readable JSON
    #[arg(long)]
    pub json: bool,

    /// Number of runs to show
    #[arg(long, default_value_t = 5)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct RunsArgs {
    /// Emit machine-readable JSON
    #[arg(long)]
    pub json: bool,

    /// Number of runs to show
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct TestHoldLockArgs {
    /// How long to hold the lock for (milliseconds)
    #[arg(long, default_value_t = 1000)]
    pub ms: u64,
}
