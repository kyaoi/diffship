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
    /// Start the interactive TUI (status/runs viewer + loop launcher)
    Tui(TuiArgs),

    /// Build an AI handoff bundle from committed and optional uncommitted diffs
    Build(BuildArgs),

    /// Generate a ChatGPT Project kit under .diffship/
    Init(InitArgs),

    /// Show lock state and recent runs
    Status(StatusArgs),

    /// List recent runs
    Runs(RunsArgs),

    /// Apply a patch bundle safely (in an isolated sandbox)
    Apply(ApplyArgs),

    /// Run verification (profile: fast|standard|full) in the latest sandbox
    Verify(VerifyArgs),

    /// Create a reprompt zip for a run (M2-06)
    #[command(name = "pack-fix")]
    PackFix(PackFixArgs),

    /// Promote a verified sandbox result back to a target branch (default: develop)
    Promote(PromoteArgs),

    /// Orchestrate apply → verify → promote (commit)
    Loop(LoopArgs),

    /// Internal test helper: acquire the lock and hold it for a duration.
    #[command(name = "__test_hold_lock", hide = true)]
    __TestHoldLock(TestHoldLockArgs),

    /// Internal test helper: create/reuse a session and create a sandbox for a new run.
    #[command(name = "__test_m1_setup", hide = true)]
    __TestM1Setup(TestM1SetupArgs),

    /// Internal test helper: advance a session ref to a sandbox HEAD.
    #[command(name = "__test_m1_advance_session", hide = true)]
    __TestM1AdvanceSession(TestM1AdvanceSessionArgs),

    /// Internal test helper: remove a sandbox worktree for a run.
    #[command(name = "__test_m1_cleanup_sandbox", hide = true)]
    __TestM1CleanupSandbox(TestM1CleanupSandboxArgs),
}

#[derive(Debug, Args, Default)]
pub struct TuiArgs {}

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Committed range mode (direct|merge-base|last|root)
    #[arg(long, default_value = "last", value_name = "MODE")]
    pub range_mode: String,

    /// Range start revision (for range-mode=direct)
    #[arg(long)]
    pub from: Option<String>,

    /// Range end revision (for range-mode=direct/root; default: HEAD)
    #[arg(long)]
    pub to: Option<String>,

    /// Range A revision (for range-mode=merge-base)
    #[arg(long)]
    pub a: Option<String>,

    /// Range B revision (for range-mode=merge-base)
    #[arg(long)]
    pub b: Option<String>,

    /// Exclude committed range from the bundle (default: committed included)
    #[arg(long, default_value_t = false)]
    pub no_committed: bool,

    /// Include staged changes (based on current HEAD)
    #[arg(long, default_value_t = false)]
    pub include_staged: bool,

    /// Include unstaged changes (based on current HEAD / working tree)
    #[arg(long, default_value_t = false)]
    pub include_unstaged: bool,

    /// Include untracked files according to --untracked-mode
    #[arg(long, default_value_t = false)]
    pub include_untracked: bool,

    /// Split output by auto|file|commit (commit applies to committed range only)
    #[arg(long, default_value = "auto")]
    pub split_by: Option<String>,

    /// How to handle untracked files: auto|patch|raw|meta
    #[arg(long, default_value = "auto")]
    pub untracked_mode: String,

    /// Output directory path (default: ./diffship_<timestamp>/)
    #[arg(long)]
    pub out: Option<String>,

    /// Also create a zip bundle next to the output directory
    #[arg(long, default_value_t = false)]
    pub zip: bool,

    /// Automatically continue when secrets-like content is detected
    #[arg(long, default_value_t = false)]
    pub yes: bool,

    /// Refuse the build when secrets-like content is detected
    #[arg(long, default_value_t = false)]
    pub fail_on_secrets: bool,
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
pub struct ApplyArgs {
    /// Patch bundle path (directory or .zip)
    pub bundle: String,

    /// Session name (default: "default")
    #[arg(long, default_value = "default")]
    pub session: String,

    /// Keep the sandbox worktree for later verification/promotion (default: true)
    #[arg(long, default_value_t = true)]
    pub keep_sandbox: bool,
}

#[derive(Debug, Args)]
pub struct VerifyArgs {
    /// Verification profile (fast|standard|full)
    #[arg(long)]
    pub profile: Option<String>,

    /// Run id to verify (defaults to the latest run that has a sandbox)
    #[arg(long)]
    pub run_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct PackFixArgs {
    /// Run id to pack (defaults to the latest run)
    #[arg(long)]
    pub run_id: Option<String>,

    /// Output zip path (default: .diffship/runs/<run-id>/pack-fix.zip)
    #[arg(long)]
    pub out: Option<String>,
}

#[derive(Debug, Args)]
pub struct PromoteArgs {
    /// Run id to promote (defaults to the latest run that has a successful verify)
    #[arg(long)]
    pub run_id: Option<String>,

    /// Target branch to promote into (default: develop; falls back to current branch if develop doesn't exist)
    #[arg(long)]
    pub target_branch: Option<String>,

    /// Acknowledge secrets warnings (required if secrets are detected)
    #[arg(long)]
    pub ack_secrets: bool,

    /// Acknowledge required user tasks (required if tasks are present)
    #[arg(long)]
    pub ack_tasks: bool,

    /// Keep the sandbox worktree after promotion (default: false)
    #[arg(long, default_value_t = false)]
    pub keep_sandbox: bool,
    /// Promotion mode override (none|working-tree|commit)
    #[arg(long, value_name = "MODE")]
    pub promotion: Option<String>,

    /// Commit policy override (auto|manual)
    #[arg(long, value_name = "POLICY")]
    pub commit_policy: Option<String>,
}

#[derive(Debug, Args)]
pub struct LoopArgs {
    /// Patch bundle path (directory or .zip)
    pub bundle: String,

    /// Session name (default: "default")
    #[arg(long, default_value = "default")]
    pub session: String,

    /// Verification profile (fast|standard|full)
    #[arg(long)]
    pub profile: Option<String>,

    /// Target branch to promote into (default: develop; falls back to current branch if develop doesn't exist)
    #[arg(long)]
    pub target_branch: Option<String>,

    /// Acknowledge secrets warnings (required if secrets are detected)
    #[arg(long)]
    pub ack_secrets: bool,

    /// Acknowledge required user tasks (required if tasks are present)
    #[arg(long)]
    pub ack_tasks: bool,
    /// Promotion mode override (none|working-tree|commit)
    #[arg(long, value_name = "MODE")]
    pub promotion: Option<String>,

    /// Commit policy override (auto|manual)
    #[arg(long, value_name = "POLICY")]
    pub commit_policy: Option<String>,
}

#[derive(Debug, Args)]
pub struct TestHoldLockArgs {
    /// How long to hold the lock for (milliseconds)
    #[arg(long, default_value_t = 1000)]
    pub ms: u64,
}

#[derive(Debug, Args)]
pub struct TestM1SetupArgs {
    /// Session name (default: "default")
    #[arg(long, default_value = "default")]
    pub session: String,
}

#[derive(Debug, Args)]
pub struct TestM1AdvanceSessionArgs {
    /// Session name (default: "default")
    #[arg(long, default_value = "default")]
    pub session: String,

    /// Run id whose sandbox HEAD should become the new session HEAD.
    #[arg(long)]
    pub run_id: String,
}

#[derive(Debug, Args)]
pub struct TestM1CleanupSandboxArgs {
    /// Run id whose sandbox worktree should be removed.
    #[arg(long)]
    pub run_id: String,
}
