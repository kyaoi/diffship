use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::{command_log, lock, run, session, tasks, worktree};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::style::Stylize;
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use serde_json::Value;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Runs,
    Status,
    Loop,
    Handoff,
    Compare,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InputMode {
    Normal,
    Editing(EditTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditTarget {
    LoopBundle,
    CompareBundleA,
    CompareBundleB,
    HandoffFrom,
    HandoffTo,
    HandoffA,
    HandoffB,
    HandoffInclude,
    HandoffExclude,
    HandoffOut,
    HandoffPlanPath,
    HandoffMaxParts,
    HandoffMaxBytes,
}

#[derive(Debug, Clone)]
struct StatusSnapshot {
    git_root: String,
    lock_path: String,
    lock_held: bool,
    lock_info: Option<lock::LockInfo>,
    sessions: Vec<session::SessionState>,
    sandboxes: Vec<SandboxRow>,
    recent_runs: Vec<run::RunSummary>,
}

#[derive(Debug, Clone)]
struct SandboxRow {
    run_id: String,
    path: String,
    exists: bool,
}

#[derive(Debug, Clone)]
struct RunDetail {
    meta: run::RunMeta,
    run_dir: PathBuf,
    sandbox: Option<worktree::SandboxMeta>,
    apply: Option<Value>,
    verify: Option<Value>,
    promotion: Option<Value>,
    commands: Vec<command_log::CommandLogRecord>,
    user_tasks_path: PathBuf,
}

#[derive(Debug, Clone)]
struct HandoffState {
    plan: crate::plan::HandoffPlan,
    plan_path: String,
    message: String,
    preview_title: String,
    preview_lines: Vec<String>,
    preview_scroll: usize,
}

#[derive(Debug, Clone)]
struct CompareState {
    bundle_a: String,
    bundle_b: String,
    strict: bool,
    message: String,
    report_title: String,
    report_lines: Vec<String>,
    report_scroll: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewLineStyle {
    Plain,
    Added,
    Removed,
    Hunk,
}

#[derive(Debug)]
struct App {
    screen: Screen,
    input_mode: InputMode,
    edit_buffer: String,

    // Runs
    runs: Vec<run::RunSummary>,
    selected_run: usize,
    show_detail: bool,
    run_detail: Option<RunDetail>,

    // Status
    status: Option<StatusSnapshot>,

    // Loop
    bundle_input: String,
    loop_message: String,
    loop_scroll: usize,

    // Handoff
    handoff_config: crate::handoff_config::HandoffConfig,
    handoff_profile_names: Vec<String>,
    handoff: HandoffState,

    // Compare
    compare: CompareState,

    should_exit: bool,
}

pub fn is_tty() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn should_start_tui_impl(is_tty: bool, env_v: &str) -> bool {
    if !is_tty {
        return false;
    }

    // Safety escape hatch for scripts.
    let v = env_v.trim().to_ascii_lowercase();
    !matches!(v.as_str(), "1" | "true" | "yes" | "on")
}

pub fn should_start_tui() -> bool {
    let v = std::env::var("DIFFSHIP_NO_TUI").unwrap_or_default();
    should_start_tui_impl(is_tty(), &v)
}

pub fn run(git_root: &Path) -> Result<(), ExitError> {
    let mut guard = TerminalGuard::enter()?;

    let mut app = App::new(git_root)?;

    loop {
        app.draw()?;

        if app.should_exit {
            break;
        }

        if event::poll(Duration::from_millis(200))
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("tui event poll failed: {e}")))?
        {
            match event::read()
                .map_err(|e| ExitError::new(EXIT_GENERAL, format!("tui event read failed: {e}")))?
            {
                Event::Key(k) => app.handle_key(git_root, &mut guard, k)?,
                Event::Resize(_, _) => {
                    // Redraw on next loop.
                }
                _ => {}
            }
        }
    }

    // guard drop restores terminal.
    Ok(())
}

impl App {
    fn new(git_root: &Path) -> Result<Self, ExitError> {
        let runs = run::list_runs(git_root, 20)?;
        let status = Some(load_status(git_root, 8)?);
        let handoff_config = crate::handoff_config::HandoffConfig::load(git_root)?;
        let handoff_profile_names = handoff_config.available_profile_names();
        let default_profile = handoff_config.resolve_selection(None, None, None)?;
        let handoff_plan = crate::plan::HandoffPlan {
            profile: Some(default_profile.selected_name.clone()),
            max_parts: Some(default_profile.max_parts),
            max_bytes_per_part: Some(default_profile.max_bytes_per_part),
            out_dir: handoff_config.default_output_dir().map(str::to_string),
            ..crate::plan::HandoffPlan::default()
        };

        Ok(Self {
            screen: Screen::Runs,
            input_mode: InputMode::Normal,
            edit_buffer: String::new(),

            runs,
            selected_run: 0,
            show_detail: false,
            run_detail: None,

            status,

            bundle_input: String::new(),
            loop_message: "Bundle path is empty. Press i to type, Enter to run.".to_string(),
            loop_scroll: 0,

            handoff_config,
            handoff_profile_names,
            handoff: HandoffState {
                plan: handoff_plan,
                plan_path: "diffship_plan.toml".to_string(),
                message: "Press v to preview the current handoff selection, or g to build."
                    .to_string(),
                preview_title: "(no preview yet)".to_string(),
                preview_lines: vec![],
                preview_scroll: 0,
            },

            compare: CompareState {
                bundle_a: String::new(),
                bundle_b: String::new(),
                strict: false,
                message: "Type two bundle paths, then press Enter to compare.".to_string(),
                report_title: "(no compare yet)".to_string(),
                report_lines: vec![],
                report_scroll: 0,
            },

            should_exit: false,
        })
    }

    fn refresh_runs(&mut self, git_root: &Path) -> Result<(), ExitError> {
        self.runs = run::list_runs(git_root, 20)?;
        if self.selected_run >= self.runs.len() {
            self.selected_run = self.runs.len().saturating_sub(1);
        }
        Ok(())
    }

    fn refresh_status(&mut self, git_root: &Path) -> Result<(), ExitError> {
        self.status = Some(load_status(git_root, 8)?);
        Ok(())
    }

    fn refresh_all(&mut self, git_root: &Path) -> Result<(), ExitError> {
        self.refresh_runs(git_root)?;
        self.refresh_status(git_root)?;
        if self.show_detail
            && let Some(rd) = self.run_detail.as_ref().map(|d| d.meta.run_id.clone())
        {
            self.run_detail = load_run_detail(git_root, &rd)?;
        }
        Ok(())
    }

    fn draw(&mut self) -> Result<(), ExitError> {
        let mut out = io::stdout();
        let (w, h) = terminal::size().map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to query terminal size: {e}"))
        })?;

        execute!(out, MoveTo(0, 0), Clear(ClearType::All))
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("tui draw failed: {e}")))?;

        writeln_trunc(
            &mut out,
            "diffship TUI  |  [1]Runs [2]Status [3]Loop [4]Handoff [5]Compare  |  r=refresh  q/Esc=quit",
            w,
        )?;
        writeln_trunc(&mut out, &line('-', w), w)?;

        match self.screen {
            Screen::Runs => self.draw_runs(&mut out, w, h)?,
            Screen::Status => self.draw_status(&mut out, w, h)?,
            Screen::Loop => self.draw_loop(&mut out, w, h)?,
            Screen::Handoff => self.draw_handoff(&mut out, w, h)?,
            Screen::Compare => self.draw_compare(&mut out, w, h)?,
        }

        out.flush()
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to flush terminal: {e}")))?;
        Ok(())
    }

    fn draw_runs(&self, out: &mut impl Write, w: u16, h: u16) -> Result<(), ExitError> {
        if self.show_detail {
            return self.draw_run_detail(out, w, h);
        }

        writeln_trunc(out, "Runs (↑/↓ to select, Enter=detail)", w)?;

        if self.runs.is_empty() {
            writeln_trunc(out, "(no runs)", w)?;
            return Ok(());
        }

        let max_rows = h.saturating_sub(4) as usize; // header lines already
        let start = self.selected_run.saturating_sub(max_rows / 2);
        let end = (start + max_rows).min(self.runs.len());

        for (idx, r) in self.runs[start..end].iter().enumerate() {
            let real_idx = start + idx;
            let mark = if real_idx == self.selected_run {
                ">"
            } else {
                " "
            };
            let s = format!("{} {}  {}  {}", mark, r.created_at, r.run_id, r.command);
            writeln_trunc(out, &s, w)?;
        }

        Ok(())
    }

    fn draw_run_detail(&self, out: &mut impl Write, w: u16, _h: u16) -> Result<(), ExitError> {
        writeln_trunc(out, "Run detail (b/Backspace=back)", w)?;

        let Some(d) = &self.run_detail else {
            writeln_trunc(out, "(detail unavailable)", w)?;
            return Ok(());
        };

        writeln_trunc(out, &format!("run_id    : {}", d.meta.run_id), w)?;
        writeln_trunc(out, &format!("created_at: {}", d.meta.created_at), w)?;
        writeln_trunc(out, &format!("command   : {}", d.meta.command), w)?;
        writeln_trunc(out, &format!("run_dir   : {}", d.run_dir.display()), w)?;

        if let Some(sb) = &d.sandbox {
            writeln_trunc(out, &format!("sandbox   : {}", sb.path), w)?;
            writeln_trunc(out, &format!("base      : {}", sb.base_commit), w)?;
            writeln_trunc(out, &format!("session   : {}", sb.session), w)?;
        } else {
            writeln_trunc(out, "sandbox   : (none)", w)?;
        }

        // Summaries
        writeln_trunc(out, "", w)?;
        writeln_trunc(
            out,
            &format!("apply     : {}", summarize_step(d.apply.as_ref())),
            w,
        )?;
        writeln_trunc(
            out,
            &format!("verify    : {}", summarize_step(d.verify.as_ref())),
            w,
        )?;
        writeln_trunc(
            out,
            &format!("promotion : {}", summarize_step(d.promotion.as_ref())),
            w,
        )?;
        if d.commands.is_empty() {
            writeln_trunc(out, "commands  : (none)", w)?;
        } else {
            let phases = d
                .commands
                .iter()
                .map(|record| record.phase.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(",");
            writeln_trunc(
                out,
                &format!("commands  : {} ({})", d.commands.len(), phases),
                w,
            )?;
        }

        // Paths / tasks
        let tasks_present = d.user_tasks_path.is_file();
        writeln_trunc(out, "", w)?;
        writeln_trunc(out, "Artifacts:", w)?;
        writeln_trunc(
            out,
            &format!(
                "  - run.json        : {}",
                d.run_dir.join("run.json").display()
            ),
            w,
        )?;
        writeln_trunc(
            out,
            &format!(
                "  - apply.json      : {}",
                d.run_dir.join("apply.json").display()
            ),
            w,
        )?;
        writeln_trunc(
            out,
            &format!(
                "  - verify.json     : {}",
                d.run_dir.join("verify.json").display()
            ),
            w,
        )?;
        writeln_trunc(
            out,
            &format!(
                "  - promotion.json  : {}",
                d.run_dir.join("promotion.json").display()
            ),
            w,
        )?;
        writeln_trunc(
            out,
            &format!(
                "  - commands.json   : {}",
                d.run_dir.join("commands.json").display()
            ),
            w,
        )?;
        writeln_trunc(
            out,
            &format!(
                "  - tasks           : {}{}",
                d.user_tasks_path.display(),
                if tasks_present { "" } else { " (missing)" }
            ),
            w,
        )?;
        for phase in ["apply", "post-apply", "verify", "promote"] {
            let phase_path = d.run_dir.join(phase);
            if phase_path.exists() {
                writeln_trunc(
                    out,
                    &format!("  - {:<15}: {}", format!("{phase}/"), phase_path.display()),
                    w,
                )?;
            }
        }

        Ok(())
    }

    fn draw_status(&self, out: &mut impl Write, w: u16, _h: u16) -> Result<(), ExitError> {
        writeln_trunc(out, "Status", w)?;

        let Some(s) = &self.status else {
            writeln_trunc(out, "(status unavailable)", w)?;
            return Ok(());
        };

        writeln_trunc(out, &format!("git_root : {}", s.git_root), w)?;
        writeln_trunc(out, &format!("lock     : {}", s.lock_path), w)?;

        match (s.lock_held, s.lock_info.as_ref()) {
            (true, Some(i)) => writeln_trunc(
                out,
                &format!(
                    "lock_held: yes (pid={}, started_at={}, cmd={})",
                    i.pid, i.started_at, i.command
                ),
                w,
            )?,
            (true, None) => writeln_trunc(out, "lock_held: yes (metadata unreadable)", w)?,
            (false, Some(i)) => writeln_trunc(
                out,
                &format!(
                    "lock_held: no (last pid={}, started_at={}, cmd={})",
                    i.pid, i.started_at, i.command
                ),
                w,
            )?,
            (false, None) => writeln_trunc(out, "lock_held: no", w)?,
        }

        writeln_trunc(out, "", w)?;
        writeln_trunc(out, "Recent runs:", w)?;
        if s.recent_runs.is_empty() {
            writeln_trunc(out, "  (none)", w)?;
        } else {
            for r in &s.recent_runs {
                writeln_trunc(
                    out,
                    &format!("  - {}  {}  {}", r.created_at, r.run_id, r.command),
                    w,
                )?;
            }
        }

        writeln_trunc(out, "", w)?;
        writeln_trunc(out, "Sessions:", w)?;
        if s.sessions.is_empty() {
            writeln_trunc(out, "  (none)", w)?;
        } else {
            for ss in &s.sessions {
                writeln_trunc(
                    out,
                    &format!("  - {}  head={}  wt={}", ss.name, ss.head, ss.worktree_path),
                    w,
                )?;
            }
        }

        writeln_trunc(out, "", w)?;
        writeln_trunc(out, "Sandboxes:", w)?;
        if s.sandboxes.is_empty() {
            writeln_trunc(out, "  (none)", w)?;
        } else {
            for sb in &s.sandboxes {
                let hint = if sb.exists { "" } else { " (missing on disk)" };
                writeln_trunc(out, &format!("  - {}  {}{}", sb.run_id, sb.path, hint), w)?;
            }
            writeln_trunc(
                out,
                "  hint: you can remove a sandbox via: git worktree remove --force <path>",
                w,
            )?;
        }

        Ok(())
    }

    fn draw_loop(&self, out: &mut impl Write, w: u16, h: u16) -> Result<(), ExitError> {
        writeln_trunc(out, "Loop", w)?;

        writeln_trunc(
            out,
            "Type a patch bundle path, then press Enter to run: diffship loop <bundle>.",
            w,
        )?;
        writeln_trunc(
            out,
            "Handoff preview is available via CLI: diffship preview <bundle> [--list|--part ...]",
            w,
        )?;
        writeln_trunc(
            out,
            "Keys: i=edit path, Enter=run, c=clear message, ↑/↓ scroll",
            w,
        )?;

        writeln_trunc(out, "", w)?;
        let mode = match &self.input_mode {
            InputMode::Normal => "normal".to_string(),
            InputMode::Editing(target) => format!("editing {}", edit_target_label(*target)),
        };
        writeln_trunc(out, &format!("mode   : {}", mode), w)?;
        writeln_trunc(out, &format!("bundle : {}", self.bundle_input), w)?;

        writeln_trunc(out, "", w)?;
        writeln_trunc(out, "Message:", w)?;

        let lines: Vec<&str> = self.loop_message.lines().collect();
        let max_rows = h.saturating_sub(10) as usize;
        let start = self.loop_scroll.min(lines.len());
        let end = (start + max_rows).min(lines.len());
        for l in &lines[start..end] {
            writeln_trunc(out, l, w)?;
        }

        Ok(())
    }

    fn draw_handoff(&self, out: &mut impl Write, w: u16, h: u16) -> Result<(), ExitError> {
        for line in handoff_overview_lines(&self.input_mode, &self.handoff) {
            writeln_trunc(out, &line, w)?;
        }
        let reserved = 28usize;
        let max_rows = h as usize;
        let preview_rows = max_rows.saturating_sub(reserved).max(4);
        if self.handoff.preview_lines.is_empty() {
            writeln_trunc(out, "  (no preview yet)", w)?;
        } else {
            let start = self
                .handoff
                .preview_scroll
                .min(self.handoff.preview_lines.len());
            let end = (start + preview_rows).min(self.handoff.preview_lines.len());
            for line in &self.handoff.preview_lines[start..end] {
                writeln_preview(out, line, w)?;
            }
        }
        writeln_trunc(out, "", w)?;
        writeln_trunc(out, "6) Message", w)?;
        writeln_trunc(out, &self.handoff.message, w)?;
        writeln_trunc(out, "", w)?;
        for line in edit_status_lines(&self.input_mode, &self.edit_buffer) {
            writeln_trunc(out, &line, w)?;
        }
        Ok(())
    }

    fn draw_compare(&self, out: &mut impl Write, w: u16, h: u16) -> Result<(), ExitError> {
        for line in compare_overview_lines(&self.input_mode, &self.compare) {
            writeln_trunc(out, &line, w)?;
        }
        let reserved = 14usize;
        let max_rows = h as usize;
        let report_rows = max_rows.saturating_sub(reserved).max(4);
        if self.compare.report_lines.is_empty() {
            writeln_trunc(out, "  (no compare report yet)", w)?;
        } else {
            let start = self
                .compare
                .report_scroll
                .min(self.compare.report_lines.len());
            let end = (start + report_rows).min(self.compare.report_lines.len());
            for line in &self.compare.report_lines[start..end] {
                writeln_trunc(out, line, w)?;
            }
        }
        writeln_trunc(out, "", w)?;
        writeln_trunc(out, "4) Message", w)?;
        writeln_trunc(out, &self.compare.message, w)?;
        writeln_trunc(out, "", w)?;
        for line in edit_status_lines(&self.input_mode, &self.edit_buffer) {
            writeln_trunc(out, &line, w)?;
        }
        Ok(())
    }

    fn handle_key(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
        key: KeyEvent,
    ) -> Result<(), ExitError> {
        // Global exit.
        if matches!(key.code, KeyCode::Esc) && self.input_mode == InputMode::Normal {
            self.should_exit = true;
            return Ok(());
        }
        if matches!(key.code, KeyCode::Char('q')) && self.input_mode == InputMode::Normal {
            self.should_exit = true;
            return Ok(());
        }

        // Input mode: simple line editor.
        if let InputMode::Editing(target) = self.input_mode.clone() {
            return self.handle_edit_input(git_root, guard, target, key);
        }

        match key.code {
            KeyCode::Char('1') => {
                self.screen = Screen::Runs;
                self.show_detail = false;
            }
            KeyCode::Char('2') => {
                self.screen = Screen::Status;
            }
            KeyCode::Char('3') => {
                self.screen = Screen::Loop;
            }
            KeyCode::Char('4') => {
                self.screen = Screen::Handoff;
            }
            KeyCode::Char('5') => {
                self.screen = Screen::Compare;
            }
            KeyCode::Char('r') => {
                self.refresh_all(git_root)?;
                self.loop_message = "refreshed".to_string();
                self.handoff.message = "refreshed".to_string();
                self.compare.message = "refreshed".to_string();
            }
            _ => {}
        }

        match self.screen {
            Screen::Runs => self.handle_runs_keys(git_root, key),
            Screen::Status => Ok(()),
            Screen::Loop => self.handle_loop_keys(git_root, guard, key),
            Screen::Handoff => self.handle_handoff_keys(git_root, guard, key),
            Screen::Compare => self.handle_compare_keys(git_root, guard, key),
        }
    }

    fn handle_runs_keys(&mut self, git_root: &Path, key: KeyEvent) -> Result<(), ExitError> {
        if self.show_detail {
            match key.code {
                KeyCode::Char('b') | KeyCode::Backspace => {
                    self.show_detail = false;
                    self.run_detail = None;
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected_run = self.selected_run.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.runs.is_empty() {
                    self.selected_run = (self.selected_run + 1).min(self.runs.len() - 1);
                }
            }
            KeyCode::Enter => {
                if let Some(r) = self.runs.get(self.selected_run) {
                    self.run_detail = load_run_detail(git_root, &r.run_id)?;
                    self.show_detail = true;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_loop_keys(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
        key: KeyEvent,
    ) -> Result<(), ExitError> {
        match key.code {
            KeyCode::Char('i') => {
                self.start_edit(EditTarget::LoopBundle, self.bundle_input.clone());
            }
            KeyCode::Char('c') => {
                self.loop_message.clear();
                self.loop_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.loop_scroll = self.loop_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.loop_scroll = self.loop_scroll.saturating_add(1);
            }
            KeyCode::Enter => {
                if self.bundle_input.trim().is_empty() {
                    self.loop_message = "Bundle path is empty. Press i to type.".to_string();
                } else {
                    self.run_loop_now(git_root, guard)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_compare_keys(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
        key: KeyEvent,
    ) -> Result<(), ExitError> {
        match key.code {
            KeyCode::Char('a') => {
                self.start_edit(EditTarget::CompareBundleA, self.compare.bundle_a.clone());
            }
            KeyCode::Char('b') => {
                self.start_edit(EditTarget::CompareBundleB, self.compare.bundle_b.clone());
            }
            KeyCode::Char('s') => {
                self.compare.strict = !self.compare.strict;
            }
            KeyCode::Char('c') => {
                self.compare.message.clear();
                self.compare.report_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.compare.report_scroll = self.compare.report_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.compare.report_scroll = self.compare.report_scroll.saturating_add(1);
            }
            KeyCode::Enter => {
                self.run_compare_now(git_root, guard)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_edit_input(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
        target: EditTarget,
        key: KeyEvent,
    ) -> Result<(), ExitError> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.edit_buffer.clear();
            }
            KeyCode::Tab => {
                if let Some(next) = next_edit_target(target, false) {
                    self.start_edit(next, self.current_edit_value(next));
                }
            }
            KeyCode::BackTab => {
                if let Some(next) = next_edit_target(target, true) {
                    self.start_edit(next, self.current_edit_value(next));
                }
            }
            KeyCode::Enter => {
                let value = self.edit_buffer.trim().to_string();
                if let Err(err) = self.apply_edit_value(target, value) {
                    self.handoff.message = err;
                    self.input_mode = InputMode::Normal;
                    self.edit_buffer.clear();
                    return Ok(());
                }
                self.input_mode = InputMode::Normal;
                self.edit_buffer.clear();
                if target == EditTarget::LoopBundle && !self.bundle_input.trim().is_empty() {
                    self.run_loop_now(git_root, guard)?;
                }
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.edit_buffer.clear();
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                trim_last_word(&mut self.edit_buffer);
            }
            KeyCode::Char(c) => {
                // Only accept printable characters.
                if !c.is_control() {
                    self.edit_buffer.push(c);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn run_loop_now(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
    ) -> Result<(), ExitError> {
        let bundle = self.bundle_input.trim().to_string();

        guard.suspend()?;

        println!("Running: diffship loop {}", bundle);
        println!("(Ctrl+C to abort)\n");

        let st = run_diffship_child(git_root, &["loop".to_string(), bundle.clone()])?;
        println!("\nDone. exit={}", st);
        println!("Press Enter to return to TUI...");
        let mut _buf = String::new();
        let _ = io::stdin().read_line(&mut _buf);

        guard.resume()?;

        self.loop_message = format!(
            "Last loop: bundle={}\nexit={}\n(see your terminal scrollback for full logs)",
            bundle, st
        );
        self.loop_scroll = 0;

        // Refresh views so the run list updates.
        self.refresh_all(git_root)?;
        Ok(())
    }

    fn handle_handoff_keys(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
        key: KeyEvent,
    ) -> Result<(), ExitError> {
        match key.code {
            KeyCode::Char('m') => {
                self.handoff.plan.range_mode = cycle_value(
                    &self.handoff.plan.range_mode,
                    &["last", "root", "direct", "merge-base"],
                );
            }
            KeyCode::Char('f') => {
                self.start_edit(
                    EditTarget::HandoffFrom,
                    self.handoff.plan.from.clone().unwrap_or_default(),
                );
            }
            KeyCode::Char('t') => {
                self.start_edit(
                    EditTarget::HandoffTo,
                    self.handoff.plan.to.clone().unwrap_or_default(),
                );
            }
            KeyCode::Char('a') => {
                self.start_edit(
                    EditTarget::HandoffA,
                    self.handoff.plan.a.clone().unwrap_or_default(),
                );
            }
            KeyCode::Char('b') => {
                self.start_edit(
                    EditTarget::HandoffB,
                    self.handoff.plan.b.clone().unwrap_or_default(),
                );
            }
            KeyCode::Char('l') => {
                self.start_edit(
                    EditTarget::HandoffInclude,
                    self.handoff.plan.include.join(", "),
                );
            }
            KeyCode::Char('e') => {
                self.start_edit(
                    EditTarget::HandoffExclude,
                    self.handoff.plan.exclude.join(", "),
                );
            }
            KeyCode::Char('o') => {
                self.start_edit(
                    EditTarget::HandoffOut,
                    self.handoff.plan.out.clone().unwrap_or_default(),
                );
            }
            KeyCode::Char('O') => {
                self.start_edit(EditTarget::HandoffPlanPath, self.handoff.plan_path.clone());
            }
            KeyCode::Char('c') => {
                self.handoff.plan.include_committed = !self.handoff.plan.include_committed;
            }
            KeyCode::Char('s') => {
                self.handoff.plan.include_staged = !self.handoff.plan.include_staged;
            }
            KeyCode::Char('u') => {
                self.handoff.plan.include_unstaged = !self.handoff.plan.include_unstaged;
            }
            KeyCode::Char('n') => {
                self.handoff.plan.include_untracked = !self.handoff.plan.include_untracked;
            }
            KeyCode::Char('p') => {
                self.handoff.plan.split_by =
                    cycle_value(&self.handoff.plan.split_by, &["auto", "file", "commit"]);
            }
            KeyCode::Char('M') => {
                self.start_edit(
                    EditTarget::HandoffMaxParts,
                    opt_usize_to_string(self.handoff.plan.max_parts),
                );
            }
            KeyCode::Char('B') => {
                self.start_edit(
                    EditTarget::HandoffMaxBytes,
                    opt_u64_to_string(self.handoff.plan.max_bytes_per_part),
                );
            }
            KeyCode::Char('h') => {
                if let Some(next) = cycle_named_value(
                    self.handoff.plan.profile.as_deref(),
                    &self.handoff_profile_names,
                ) {
                    let resolved =
                        self.handoff_config
                            .resolve_selection(Some(&next), None, None)?;
                    self.handoff.plan.profile = Some(resolved.selected_name);
                    self.handoff.plan.max_parts = Some(resolved.max_parts);
                    self.handoff.plan.max_bytes_per_part = Some(resolved.max_bytes_per_part);
                }
            }
            KeyCode::Char('w') => {
                self.handoff.plan.untracked_mode = cycle_value(
                    &self.handoff.plan.untracked_mode,
                    &["auto", "patch", "raw", "meta"],
                );
            }
            KeyCode::Char('i') => {
                self.handoff.plan.include_binary = !self.handoff.plan.include_binary;
            }
            KeyCode::Char('y') => {
                self.handoff.plan.binary_mode =
                    cycle_value(&self.handoff.plan.binary_mode, &["raw", "patch", "meta"]);
            }
            KeyCode::Char('z') => {
                self.handoff.plan.zip = !self.handoff.plan.zip;
                if !self.handoff.plan.zip {
                    self.handoff.plan.zip_only = false;
                }
            }
            KeyCode::Char('Z') => {
                self.handoff.plan.zip_only = !self.handoff.plan.zip_only;
                if self.handoff.plan.zip_only {
                    self.handoff.plan.zip = true;
                }
            }
            KeyCode::Char('v') => {
                self.run_handoff_preview(git_root, guard)?;
            }
            KeyCode::Char('P') => {
                self.export_handoff_plan(git_root)?;
            }
            KeyCode::Char('g') | KeyCode::Enter => {
                self.run_handoff_build(git_root, guard)?;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.handoff.preview_scroll = self.handoff.preview_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.handoff.preview_scroll = self.handoff.preview_scroll.saturating_add(1);
            }
            _ => {}
        }
        Ok(())
    }

    fn run_handoff_preview(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
    ) -> Result<(), ExitError> {
        let preview_dir = preview_output_dir(git_root);
        let mut preview_plan = self.handoff.plan.clone();
        preview_plan.out = Some(preview_dir.display().to_string());
        preview_plan.zip = false;
        preview_plan.zip_only = false;
        preview_plan.yes = true;
        let args = preview_plan.to_build_args();

        guard.suspend()?;
        let run = run_diffship_child_output(git_root, &args)?;
        guard.resume()?;

        if run.code != 0 {
            self.handoff.message = summarize_child_failure("preview", &run);
            return Ok(());
        }

        let (title, lines) = read_preview_lines(&preview_dir)?;
        self.handoff.preview_title = title;
        self.handoff.preview_lines = lines;
        self.handoff.preview_scroll = 0;
        self.handoff.message = "Preview refreshed from a temporary handoff bundle.".to_string();

        let _ = fs::remove_dir_all(&preview_dir);
        Ok(())
    }

    fn run_handoff_build(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
    ) -> Result<(), ExitError> {
        guard.suspend()?;
        let run = run_diffship_child_output(git_root, &self.handoff.plan.to_build_args())?;
        guard.resume()?;

        if run.code == 0 {
            self.handoff.message = summarize_child_success("build", &run);
        } else {
            self.handoff.message = summarize_child_failure("build", &run);
        }
        Ok(())
    }

    fn run_compare_now(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
    ) -> Result<(), ExitError> {
        if self.compare.bundle_a.trim().is_empty() || self.compare.bundle_b.trim().is_empty() {
            self.compare.message =
                "Both bundle paths are required. Press a / b to edit.".to_string();
            return Ok(());
        }

        let mut args = vec![
            "compare".to_string(),
            self.compare.bundle_a.trim().to_string(),
            self.compare.bundle_b.trim().to_string(),
        ];
        if self.compare.strict {
            args.push("--strict".to_string());
        }
        args.push("--json".to_string());

        guard.suspend()?;
        let run = run_diffship_child_output(git_root, &args)?;
        guard.resume()?;

        match render_compare_report(&run) {
            Ok((title, lines)) => {
                self.compare.report_title = title;
                self.compare.report_lines = lines;
                self.compare.report_scroll = 0;
                self.compare.message = if run.code == 0 {
                    "Compare report refreshed.".to_string()
                } else {
                    "Compare report refreshed (differences detected).".to_string()
                };
            }
            Err(message) => {
                self.compare.message = message;
            }
        }
        Ok(())
    }

    fn start_edit(&mut self, target: EditTarget, initial: String) {
        self.edit_buffer = initial;
        self.input_mode = InputMode::Editing(target);
    }

    fn export_handoff_plan(&mut self, git_root: &Path) -> Result<(), ExitError> {
        let path = git_root.join(current_plan_export_path(&self.handoff));
        self.handoff
            .plan
            .write_to_path(&path)
            .map_err(|e| ExitError::new(EXIT_GENERAL, e))?;
        self.handoff.message = format!(
            "Plan exported: {} (profile={} + resolved limits)\nReplay: {}",
            path.display(),
            self.handoff.plan.profile.as_deref().unwrap_or("none"),
            crate::plan::HandoffPlan::replay_shell_command_with_overrides(
                &path.display().to_string(),
                &self.handoff.plan,
            )
        );
        Ok(())
    }

    fn current_edit_value(&self, target: EditTarget) -> String {
        match target {
            EditTarget::LoopBundle => self.bundle_input.clone(),
            EditTarget::CompareBundleA => self.compare.bundle_a.clone(),
            EditTarget::CompareBundleB => self.compare.bundle_b.clone(),
            EditTarget::HandoffFrom => self.handoff.plan.from.clone().unwrap_or_default(),
            EditTarget::HandoffTo => self.handoff.plan.to.clone().unwrap_or_default(),
            EditTarget::HandoffA => self.handoff.plan.a.clone().unwrap_or_default(),
            EditTarget::HandoffB => self.handoff.plan.b.clone().unwrap_or_default(),
            EditTarget::HandoffInclude => self.handoff.plan.include.join(", "),
            EditTarget::HandoffExclude => self.handoff.plan.exclude.join(", "),
            EditTarget::HandoffOut => self.handoff.plan.out.clone().unwrap_or_default(),
            EditTarget::HandoffPlanPath => self.handoff.plan_path.clone(),
            EditTarget::HandoffMaxParts => opt_usize_to_string(self.handoff.plan.max_parts),
            EditTarget::HandoffMaxBytes => opt_u64_to_string(self.handoff.plan.max_bytes_per_part),
        }
    }

    fn apply_edit_value(&mut self, target: EditTarget, value: String) -> Result<(), String> {
        match target {
            EditTarget::LoopBundle => {
                self.bundle_input = value;
            }
            EditTarget::CompareBundleA => {
                self.compare.bundle_a = value;
            }
            EditTarget::CompareBundleB => {
                self.compare.bundle_b = value;
            }
            EditTarget::HandoffFrom => {
                self.handoff.plan.from = empty_to_none(value);
            }
            EditTarget::HandoffTo => {
                self.handoff.plan.to = empty_to_none(value);
            }
            EditTarget::HandoffA => {
                self.handoff.plan.a = empty_to_none(value);
            }
            EditTarget::HandoffB => {
                self.handoff.plan.b = empty_to_none(value);
            }
            EditTarget::HandoffInclude => {
                self.handoff.plan.include = parse_pattern_list(&value);
            }
            EditTarget::HandoffExclude => {
                self.handoff.plan.exclude = parse_pattern_list(&value);
            }
            EditTarget::HandoffOut => {
                self.handoff.plan.out = empty_to_none(value);
            }
            EditTarget::HandoffPlanPath => {
                self.handoff.plan_path = if value.trim().is_empty() {
                    "diffship_plan.toml".to_string()
                } else {
                    value
                };
            }
            EditTarget::HandoffMaxParts => {
                self.handoff.plan.max_parts = parse_optional_usize("max parts", &value)?;
            }
            EditTarget::HandoffMaxBytes => {
                self.handoff.plan.max_bytes_per_part =
                    parse_optional_u64("max bytes per part", &value)?;
            }
        }
        Ok(())
    }
}

struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    fn enter() -> Result<Self, ExitError> {
        enable_raw_mode()
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to enable raw mode: {e}")))?;
        execute!(io::stdout(), EnterAlternateScreen, Hide).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to enter alternate screen: {e}"),
            )
        })?;
        Ok(Self { active: true })
    }

    fn suspend(&mut self) -> Result<(), ExitError> {
        if !self.active {
            return Ok(());
        }
        execute!(io::stdout(), Show, LeaveAlternateScreen).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to leave alternate screen: {e}"),
            )
        })?;
        disable_raw_mode().map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to disable raw mode: {e}"))
        })?;
        self.active = false;
        Ok(())
    }

    fn resume(&mut self) -> Result<(), ExitError> {
        if self.active {
            return Ok(());
        }
        enable_raw_mode()
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to enable raw mode: {e}")))?;
        execute!(io::stdout(), EnterAlternateScreen, Hide).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to re-enter alternate screen: {e}"),
            )
        })?;
        self.active = true;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort restoration.
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}

fn run_diffship_child(git_root: &Path, args: &[String]) -> Result<i32, ExitError> {
    let exe = std::env::current_exe()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to resolve current exe: {e}")))?;

    let status = Command::new(exe)
        .current_dir(git_root)
        .args(args)
        .status()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to spawn diffship: {e}")))?;

    Ok(status.code().unwrap_or(1))
}

struct ChildRunResult {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run_diffship_child_output(
    git_root: &Path,
    args: &[String],
) -> Result<ChildRunResult, ExitError> {
    let exe = std::env::current_exe()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to resolve current exe: {e}")))?;

    let output = Command::new(exe)
        .current_dir(git_root)
        .args(args)
        .output()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to spawn diffship: {e}")))?;

    Ok(ChildRunResult {
        code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn preview_output_dir(git_root: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    git_root.join(".diffship").join("tmp").join(format!(
        "handoff_preview_{}_{}",
        std::process::id(),
        stamp
    ))
}

fn read_preview_lines(dir: &Path) -> Result<(String, Vec<String>), ExitError> {
    let parts_dir = dir.join("parts");
    let mut names = fs::read_dir(&parts_dir)
        .map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!(
                    "failed to read preview parts dir {}: {e}",
                    parts_dir.display()
                ),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read preview entry: {e}")))?;
    names.sort_by_key(|e| e.file_name());

    let Some(first_part) = names
        .into_iter()
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("patch"))
    else {
        return Ok((
            "parts/(none)".to_string(),
            vec!["# (no patch parts)".to_string()],
        ));
    };

    let text = fs::read_to_string(&first_part).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read preview part {}: {e}", first_part.display()),
        )
    })?;
    let title = first_part
        .strip_prefix(dir)
        .unwrap_or(&first_part)
        .display()
        .to_string();
    let mut lines = preview_summary_lines(dir);
    lines.extend(text.lines().map(|s| s.to_string()));
    Ok((title, lines))
}

fn preview_summary_lines(dir: &Path) -> Vec<String> {
    let manifest_path = dir.join("handoff.manifest.json");
    let Ok(text) = fs::read_to_string(&manifest_path) else {
        return vec![];
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return vec![];
    };
    let Some(summary) = value.get("summary") else {
        return vec![];
    };

    let file_count = summary
        .get("file_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let part_count = summary
        .get("part_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let commit_view_count = summary
        .get("commit_view_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut lines = vec![
        "# Structured context".to_string(),
        format!(
            "files={} parts={} commit-views={}",
            file_count, part_count, commit_view_count
        ),
    ];

    if let Some(categories) = value_count_map(summary.get("categories")) {
        lines.push(format!("categories: {}", render_u64_count_map(&categories)));
    }
    if let Some(segments) = value_count_map(summary.get("segments")) {
        lines.push(format!("segments: {}", render_u64_count_map(&segments)));
    }
    if let Some(statuses) = value_count_map(summary.get("statuses")) {
        lines.push(format!("statuses: {}", render_u64_count_map(&statuses)));
    }
    if let Some(reading_order) = value_string_list(value.get("reading_order"))
        && !reading_order.is_empty()
    {
        lines.push("reading order:".to_string());
        for item in reading_order {
            lines.push(format!("- {item}"));
        }
    }
    lines.push(String::new());
    lines
}

fn value_count_map(value: Option<&Value>) -> Option<Vec<(String, u64)>> {
    let object = value?.as_object()?;
    let mut items = object
        .iter()
        .filter_map(|(key, value)| value.as_u64().map(|count| (key.clone(), count)))
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    Some(items)
}

fn value_string_list(value: Option<&Value>) -> Option<Vec<String>> {
    let array = value?.as_array()?;
    Some(
        array
            .iter()
            .filter_map(|entry| entry.as_str().map(str::to_string))
            .collect(),
    )
}

fn render_u64_count_map(items: &[(String, u64)]) -> String {
    items
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn summarize_child_success(kind: &str, run: &ChildRunResult) -> String {
    if run.stdout.is_empty() {
        return format!("{kind} succeeded.");
    }
    let mut lines = run.stdout.lines();
    let first = lines.next().unwrap_or_default();
    let second = lines.next().unwrap_or_default();
    if second.is_empty() {
        format!("{kind} ok: {first}")
    } else {
        format!("{kind} ok:\n{first}\n{second}")
    }
}

fn summarize_child_failure(kind: &str, run: &ChildRunResult) -> String {
    let detail = if !run.stderr.is_empty() {
        run.stderr.lines().next().unwrap_or_default().to_string()
    } else if !run.stdout.is_empty() {
        run.stdout.lines().next().unwrap_or_default().to_string()
    } else {
        "(no output)".to_string()
    };
    format!("{kind} failed (exit={}): {detail}", run.code)
}

fn render_compare_report(run: &ChildRunResult) -> Result<(String, Vec<String>), String> {
    let value = serde_json::from_str::<Value>(&run.stdout)
        .map_err(|_| summarize_child_failure("compare", run))?;
    let equivalent = value
        .get("equivalent")
        .and_then(|entry| entry.as_bool())
        .unwrap_or(false);
    let mode = value
        .get("mode")
        .and_then(|entry| entry.as_str())
        .unwrap_or("unknown");
    let title = if equivalent {
        format!("compare ({mode}): equivalent")
    } else {
        format!("compare ({mode}): different")
    };

    let mut lines = vec![
        "# Compare".to_string(),
        format!("equivalent={}", yes_no(equivalent)),
        format!(
            "bundle_a={}",
            value
                .get("bundle_a")
                .and_then(|entry| entry.as_str())
                .unwrap_or("(unknown)")
        ),
        format!(
            "bundle_b={}",
            value
                .get("bundle_b")
                .and_then(|entry| entry.as_str())
                .unwrap_or("(unknown)")
        ),
    ];

    if let Some(areas) = value_count_map(value.get("areas")) {
        lines.push(format!("areas: {}", render_u64_count_map(&areas)));
    }
    if let Some(kinds) = value_count_map(value.get("kinds")) {
        lines.push(format!("kinds: {}", render_u64_count_map(&kinds)));
    }

    if let Some(structured) = value.get("structured_context") {
        let manifest_a = structured
            .get("manifest_a")
            .and_then(|entry| entry.as_bool())
            .unwrap_or(false);
        let manifest_b = structured
            .get("manifest_b")
            .and_then(|entry| entry.as_bool())
            .unwrap_or(false);
        lines.push(format!(
            "manifest-json: a={} b={}",
            yes_no(manifest_a),
            yes_no(manifest_b)
        ));

        if let Some(summary_diffs) = value_object_diffs(structured.get("summary_diffs"), false)
            && !summary_diffs.is_empty()
        {
            lines.push("manifest summary diffs:".to_string());
            lines.extend(summary_diffs.into_iter().map(|line| format!("- {line}")));
        }
        if let Some(reading_diffs) = value_object_diffs(structured.get("reading_order_diffs"), true)
            && !reading_diffs.is_empty()
        {
            lines.push("manifest reading-order diffs:".to_string());
            lines.extend(reading_diffs.into_iter().map(|line| format!("- {line}")));
        }
    }

    if let Some(diff_lines) = value_diff_lines(value.get("diffs"))
        && !diff_lines.is_empty()
    {
        lines.push("file diffs:".to_string());
        lines.extend(diff_lines.into_iter().map(|line| format!("- {line}")));
    }

    Ok((title, lines))
}

fn value_object_diffs(value: Option<&Value>, quote_text: bool) -> Option<Vec<String>> {
    let array = value?.as_array()?;
    Some(
        array
            .iter()
            .filter_map(|entry| {
                let key = entry.get("key")?.as_str()?;
                if quote_text {
                    let a = entry.get("a")?.as_str()?;
                    let b = entry.get("b")?.as_str()?;
                    Some(format!("{key}: {a:?} -> {b:?}"))
                } else {
                    let a = entry.get("a")?.as_u64()?;
                    let b = entry.get("b")?.as_u64()?;
                    Some(format!("{key}: {a} -> {b}"))
                }
            })
            .collect(),
    )
}

fn value_diff_lines(value: Option<&Value>) -> Option<Vec<String>> {
    let array = value?.as_array()?;
    Some(
        array
            .iter()
            .filter_map(|entry| {
                Some(format!(
                    "[{}/{}] {}",
                    entry.get("area")?.as_str()?,
                    entry.get("kind")?.as_str()?,
                    entry.get("path")?.as_str()?
                ))
            })
            .collect(),
    )
}

fn load_status(git_root: &Path, recent_runs_limit: usize) -> Result<StatusSnapshot, ExitError> {
    let lock_path = lock::default_lock_path(git_root);
    let held = lock::is_lock_held(&lock_path).unwrap_or(false);
    let info = lock::read_lock_info(&lock_path);

    let recent_runs = run::list_runs(git_root, recent_runs_limit)?;
    let sessions = session::list_sessions(git_root);
    let sandboxes = list_sandboxes(git_root);

    Ok(StatusSnapshot {
        git_root: git_root.display().to_string(),
        lock_path: lock_path.display().to_string(),
        lock_held: held,
        lock_info: info,
        sessions,
        sandboxes,
        recent_runs,
    })
}

fn list_sandboxes(git_root: &Path) -> Vec<SandboxRow> {
    let dir = worktree::sandboxes_dir(git_root);
    if !dir.exists() {
        return vec![];
    }

    let mut out = vec![];
    let Ok(rd) = fs::read_dir(&dir) else {
        return vec![];
    };
    for ent in rd.flatten() {
        let Ok(ft) = ent.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        let run_id = ent.file_name().to_string_lossy().to_string();
        let path = ent.path();
        out.push(SandboxRow {
            run_id,
            path: path.display().to_string(),
            exists: path.exists(),
        });
    }

    out.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    out
}

fn load_run_detail(git_root: &Path, run_id: &str) -> Result<Option<RunDetail>, ExitError> {
    let run_dir = run::run_dir(git_root, run_id);
    if !run_dir.exists() {
        return Ok(None);
    }

    let meta = read_json::<run::RunMeta>(&run_dir.join("run.json"))?;
    let sandbox = worktree::read_sandbox_meta(git_root, run_id);

    let apply = read_json_value_opt(&run_dir.join("apply.json"))?;
    let verify = read_json_value_opt(&run_dir.join("verify.json"))?;
    let promotion = read_json_value_opt(&run_dir.join("promotion.json"))?;
    let commands = command_log::read_records(&run_dir)?;

    Ok(Some(RunDetail {
        meta,
        run_dir: run_dir.clone(),
        sandbox,
        apply,
        verify,
        promotion,
        commands,
        user_tasks_path: tasks::user_tasks_path_in_run(&run_dir),
    }))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ExitError> {
    let bytes = fs::read(path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read {}: {e}", path.display()),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to parse {}: {e}", path.display()),
        )
    })
}

fn read_json_value_opt(path: &Path) -> Result<Option<Value>, ExitError> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read {}: {e}", path.display()),
        )
    })?;
    let v = serde_json::from_slice::<Value>(&bytes).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to parse {}: {e}", path.display()),
        )
    })?;
    Ok(Some(v))
}

fn summarize_step(v: Option<&Value>) -> String {
    let Some(v) = v else {
        return "(none)".to_string();
    };

    // Prefer `ok` boolean + `error` string when present.
    let ok = v.get("ok").and_then(|x| x.as_bool());
    let err = v.get("error").and_then(|x| x.as_str());
    match (ok, err) {
        (Some(true), _) => "ok".to_string(),
        (Some(false), Some(e)) => format!("failed: {}", e),
        (Some(false), None) => "failed".to_string(),
        (None, _) => {
            // e.g. verify.json uses ok + commands; still ok.
            if v.get("ok").and_then(|x| x.as_bool()) == Some(true) {
                "ok".to_string()
            } else {
                "(unknown)".to_string()
            }
        }
    }
}

fn edit_target_label(target: EditTarget) -> &'static str {
    match target {
        EditTarget::LoopBundle => "loop.bundle",
        EditTarget::CompareBundleA => "compare.bundle_a",
        EditTarget::CompareBundleB => "compare.bundle_b",
        EditTarget::HandoffFrom => "handoff.from",
        EditTarget::HandoffTo => "handoff.to",
        EditTarget::HandoffA => "handoff.a",
        EditTarget::HandoffB => "handoff.b",
        EditTarget::HandoffInclude => "handoff.include",
        EditTarget::HandoffExclude => "handoff.exclude",
        EditTarget::HandoffOut => "handoff.out",
        EditTarget::HandoffPlanPath => "handoff.plan_path",
        EditTarget::HandoffMaxParts => "handoff.max_parts",
        EditTarget::HandoffMaxBytes => "handoff.max_bytes_per_part",
    }
}

fn display_opt(s: Option<&str>) -> &str {
    s.filter(|v| !v.is_empty()).unwrap_or("(auto)")
}

fn empty_to_none(s: String) -> Option<String> {
    if s.trim().is_empty() { None } else { Some(s) }
}

fn parse_pattern_list(s: &str) -> Vec<String> {
    s.split([',', '\n'])
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_optional_usize(label: &str, s: &str) -> Result<Option<usize>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<usize>()
        .map(Some)
        .map_err(|e| format!("invalid {label}: {trimmed} ({e})"))
}

fn parse_optional_u64(label: &str, s: &str) -> Result<Option<u64>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<u64>()
        .map(Some)
        .map_err(|e| format!("invalid {label}: {trimmed} ({e})"))
}

fn opt_usize_to_string(value: Option<usize>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

fn opt_u64_to_string(value: Option<u64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

fn cycle_value(current: &str, values: &[&str]) -> String {
    let idx = values.iter().position(|v| *v == current).unwrap_or(0);
    values[(idx + 1) % values.len()].to_string()
}

fn cycle_named_value(current: Option<&str>, values: &[String]) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let current = current.unwrap_or(values[0].as_str());
    let idx = values.iter().position(|v| v == current).unwrap_or(0);
    Some(values[(idx + 1) % values.len()].clone())
}

fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

fn next_edit_target(target: EditTarget, reverse: bool) -> Option<EditTarget> {
    const COMPARE_ORDER: &[EditTarget] = &[EditTarget::CompareBundleA, EditTarget::CompareBundleB];
    const ORDER: &[EditTarget] = &[
        EditTarget::HandoffFrom,
        EditTarget::HandoffTo,
        EditTarget::HandoffA,
        EditTarget::HandoffB,
        EditTarget::HandoffInclude,
        EditTarget::HandoffExclude,
        EditTarget::HandoffOut,
        EditTarget::HandoffPlanPath,
        EditTarget::HandoffMaxParts,
        EditTarget::HandoffMaxBytes,
    ];

    let order = if COMPARE_ORDER.contains(&target) {
        COMPARE_ORDER
    } else {
        ORDER
    };
    let idx = order.iter().position(|entry| *entry == target)?;
    let next = if reverse {
        idx.checked_sub(1).unwrap_or(order.len() - 1)
    } else {
        (idx + 1) % order.len()
    };
    Some(order[next])
}

fn trim_last_word(s: &mut String) {
    let trimmed_len = s.trim_end_matches(char::is_whitespace).len();
    s.truncate(trimmed_len);
    let cut = s
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace() || *ch == ',' || *ch == '/')
        .map(|(idx, ch)| idx + ch.len_utf8())
        .unwrap_or(0);
    s.truncate(cut);
}

fn edit_status_lines(input_mode: &InputMode, edit_buffer: &str) -> Vec<String> {
    match input_mode {
        InputMode::Normal => vec![
            "7) Edit Buffer".to_string(),
            "  - idle (press a field key to edit, Tab/Shift+Tab works while editing handoff/compare fields)"
                .to_string(),
        ],
        InputMode::Editing(target) => vec![
            "7) Edit Buffer".to_string(),
            format!("  - field: {}", edit_target_label(*target)),
            format!(
                "  - value: {}",
                if edit_buffer.is_empty() {
                    "(empty)"
                } else {
                    edit_buffer
                }
            ),
            format!("  - help : {}", edit_help_line(*target)),
        ],
    }
}

fn edit_help_line(target: EditTarget) -> &'static str {
    match target {
        EditTarget::LoopBundle => "Enter=save+run  Esc=cancel  Ctrl+U=clear  Ctrl+W=delete word",
        EditTarget::CompareBundleA | EditTarget::CompareBundleB => {
            "Enter=save  Esc=cancel  Ctrl+U=clear  Ctrl+W=delete word  Tab/Shift+Tab=next field"
        }
        EditTarget::HandoffInclude | EditTarget::HandoffExclude => {
            "Enter=save  Esc=cancel  Ctrl+U=clear  Ctrl+W=delete token  comma/newline separated"
        }
        EditTarget::HandoffMaxParts | EditTarget::HandoffMaxBytes => {
            "Enter=save  Esc=cancel  empty=resets to selected profile  Tab/Shift+Tab=next field"
        }
        _ => "Enter=save  Esc=cancel  Ctrl+U=clear  Ctrl+W=delete word  Tab/Shift+Tab=next field",
    }
}

fn handoff_overview_lines(input_mode: &InputMode, handoff: &HandoffState) -> Vec<String> {
    let mode = match input_mode {
        InputMode::Normal => "normal".to_string(),
        InputMode::Editing(target) => format!("editing {}", edit_target_label(*target)),
    };
    vec![
        "Handoff".to_string(),
        "Keys: m=range-mode  f/t=from/to  a/b=merge-base refs  c/s/u/n=sources  l/e=include/exclude  p=split  h=profile  M=max-parts  B=max-bytes  o=out  O=plan-path  w=untracked  i=include-binary  y=binary-mode  z=zip  Z=zip-only  v=preview  g=build  P=export-plan  ↑/↓=scroll preview".to_string(),
        String::new(),
        format!("mode        : {mode}"),
        format!("build cmd   : {}", handoff.plan.to_shell_command()),
        format!(
            "plan file   : {}",
            current_plan_export_path(handoff)
        ),
        format!("plan path   : {}", handoff.plan_path),
        format!(
            "replay cmd  : {}",
            crate::plan::HandoffPlan::replay_shell_command_with_overrides(
                &current_plan_export_path(handoff),
                &handoff.plan,
            )
        ),
        String::new(),
        "1) Range".to_string(),
        format!("  - mode: {}", handoff.plan.range_mode),
        format!("  - from: {}", display_opt(handoff.plan.from.as_deref())),
        format!("  - to  : {}", display_opt(handoff.plan.to.as_deref())),
        format!("  - a   : {}", display_opt(handoff.plan.a.as_deref())),
        format!("  - b   : {}", display_opt(handoff.plan.b.as_deref())),
        String::new(),
        "2) Sources".to_string(),
        format!(
            "  - committed={} staged={} unstaged={} untracked={}",
            yes_no(handoff.plan.include_committed),
            yes_no(handoff.plan.include_staged),
            yes_no(handoff.plan.include_unstaged),
            yes_no(handoff.plan.include_untracked),
        ),
        String::new(),
        "3) Filters".to_string(),
        format!(
            "  - include: {}",
            if handoff.plan.include.is_empty() {
                "(none)".to_string()
            } else {
                handoff.plan.include.join(", ")
            }
        ),
        format!(
            "  - exclude: {}",
            if handoff.plan.exclude.is_empty() {
                "(none)".to_string()
            } else {
                handoff.plan.exclude.join(", ")
            }
        ),
        "  - `.diffshipignore` is always applied.".to_string(),
        String::new(),
        "4) Split / Profile".to_string(),
        format!("  - split-by: {}", handoff.plan.split_by),
        format!("  - profile: {}", display_opt(handoff.plan.profile.as_deref())),
        format!("  - max-parts: {}", display_opt_num(handoff.plan.max_parts)),
        format!(
            "  - max-bytes-per-part: {}",
            display_opt_num(handoff.plan.max_bytes_per_part)
        ),
        format!("  - untracked-mode: {}", handoff.plan.untracked_mode),
        format!(
            "  - binary: include={} mode={}",
            yes_no(handoff.plan.include_binary),
            handoff.plan.binary_mode
        ),
        format!(
            "  - zip/zip-only/out-dir/out: {} / {} / {} / {}",
            yes_no(handoff.plan.zip),
            yes_no(handoff.plan.zip_only),
            display_opt(handoff.plan.out_dir.as_deref()),
            display_opt(handoff.plan.out.as_deref())
        ),
        String::new(),
        format!("5) Preview: {}", handoff.preview_title),
    ]
}

fn compare_overview_lines(input_mode: &InputMode, compare: &CompareState) -> Vec<String> {
    let mode = match input_mode {
        InputMode::Normal => "normal".to_string(),
        InputMode::Editing(target) => format!("editing {}", edit_target_label(*target)),
    };
    let mut compare_cmd = format!(
        "diffship compare {} {}",
        if compare.bundle_a.trim().is_empty() {
            "<bundle-a>"
        } else {
            compare.bundle_a.trim()
        },
        if compare.bundle_b.trim().is_empty() {
            "<bundle-b>"
        } else {
            compare.bundle_b.trim()
        }
    );
    if compare.strict {
        compare_cmd.push_str(" --strict");
    }

    vec![
        "Compare".to_string(),
        "Keys: a=bundle-a  b=bundle-b  s=strict  Enter=compare  c=clear message  ↑/↓=scroll report"
            .to_string(),
        String::new(),
        format!("mode        : {mode}"),
        format!("compare cmd : {compare_cmd}"),
        format!(
            "bundle a    : {}",
            if compare.bundle_a.trim().is_empty() {
                "(empty)"
            } else {
                compare.bundle_a.trim()
            }
        ),
        format!(
            "bundle b    : {}",
            if compare.bundle_b.trim().is_empty() {
                "(empty)"
            } else {
                compare.bundle_b.trim()
            }
        ),
        format!("strict      : {}", yes_no(compare.strict)),
        String::new(),
        format!("3) Report: {}", compare.report_title),
    ]
}

fn current_plan_export_path(handoff: &HandoffState) -> String {
    if handoff.plan.zip_only {
        handoff.plan_path.clone()
    } else if let Some(out) = handoff.plan.out.as_deref() {
        format!("{}/plan.toml", out.trim_end_matches('/'))
    } else {
        handoff.plan_path.clone()
    }
}

fn preview_line_style(s: &str) -> PreviewLineStyle {
    if s.starts_with('+') && !s.starts_with("+++") {
        return PreviewLineStyle::Added;
    }
    if s.starts_with('-') && !s.starts_with("---") {
        return PreviewLineStyle::Removed;
    }
    if s.starts_with("@@") {
        return PreviewLineStyle::Hunk;
    }
    PreviewLineStyle::Plain
}

fn writeln_preview(out: &mut impl Write, s: &str, w: u16) -> Result<(), ExitError> {
    let clipped = clip_to_width(s, w);
    if preview_line_style(&clipped) == PreviewLineStyle::Added {
        return write!(out, "{}\r\n", clipped.green()).map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to write to terminal: {e}"))
        });
    }
    if preview_line_style(&clipped) == PreviewLineStyle::Removed {
        return write!(out, "{}\r\n", clipped.red()).map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to write to terminal: {e}"))
        });
    }
    if preview_line_style(&clipped) == PreviewLineStyle::Hunk {
        return write!(out, "{}\r\n", clipped.cyan()).map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to write to terminal: {e}"))
        });
    }
    writeln_trunc(out, &clipped, w)
}

fn writeln_trunc(out: &mut impl Write, s: &str, w: u16) -> Result<(), ExitError> {
    let clipped = clip_to_width(s, w);

    // In raw mode, `\n` is not guaranteed to imply carriage return on all terminals.
    // Use CRLF explicitly to keep the layout stable across emulators.
    write!(out, "{}\r\n", clipped)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write to terminal: {e}")))
}

fn clip_to_width(s: &str, w: u16) -> String {
    // Char-based clipping to avoid UTF-8 boundary panics.
    let max = w as usize;
    if max == 0 {
        return String::new();
    }

    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }

    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn line(ch: char, w: u16) -> String {
    std::iter::repeat_n(ch, w as usize).collect()
}

fn display_opt_num<T: ToString>(value: Option<T>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "(auto)".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ChildRunResult, CompareState, EditTarget, HandoffState, InputMode, PreviewLineStyle,
        compare_overview_lines, current_plan_export_path, cycle_named_value, cycle_value,
        edit_status_lines, handoff_overview_lines, next_edit_target, parse_optional_u64,
        parse_optional_usize, parse_pattern_list, preview_line_style, read_preview_lines,
        render_compare_report, should_start_tui_impl, summarize_child_failure,
        summarize_child_success,
    };

    #[test]
    fn should_start_tui_impl_requires_tty() {
        assert!(!should_start_tui_impl(false, ""));
        assert!(!should_start_tui_impl(false, "1"));
    }

    #[test]
    fn should_start_tui_impl_disables_on_common_truthy_values() {
        for v in ["1", "true", "yes", "on", " TRUE ", "Yes"] {
            assert!(
                !should_start_tui_impl(true, v),
                "expected disabled for: {v}"
            );
        }
    }

    #[test]
    fn should_start_tui_impl_allows_other_values() {
        for v in ["", "0", "false", "no", "off", "random"] {
            assert!(should_start_tui_impl(true, v), "expected enabled for: {v}");
        }
    }

    #[test]
    fn cycle_value_wraps() {
        assert_eq!(cycle_value("auto", &["auto", "file", "commit"]), "file");
        assert_eq!(cycle_value("commit", &["auto", "file", "commit"]), "auto");
    }

    #[test]
    fn cycle_named_value_wraps() {
        let values = vec!["20x512".to_string(), "10x100".to_string()];
        assert_eq!(
            cycle_named_value(Some("20x512"), &values),
            Some("10x100".to_string())
        );
        assert_eq!(
            cycle_named_value(Some("10x100"), &values),
            Some("20x512".to_string())
        );
    }

    #[test]
    fn next_edit_target_cycles_handoff_fields() {
        assert_eq!(
            next_edit_target(EditTarget::HandoffFrom, false),
            Some(EditTarget::HandoffTo)
        );
        assert_eq!(
            next_edit_target(EditTarget::HandoffMaxBytes, false),
            Some(EditTarget::HandoffFrom)
        );
        assert_eq!(
            next_edit_target(EditTarget::HandoffFrom, true),
            Some(EditTarget::HandoffMaxBytes)
        );
        assert_eq!(next_edit_target(EditTarget::LoopBundle, false), None);
    }

    #[test]
    fn next_edit_target_cycles_compare_fields() {
        assert_eq!(
            next_edit_target(EditTarget::CompareBundleA, false),
            Some(EditTarget::CompareBundleB)
        );
        assert_eq!(
            next_edit_target(EditTarget::CompareBundleB, false),
            Some(EditTarget::CompareBundleA)
        );
        assert_eq!(
            next_edit_target(EditTarget::CompareBundleA, true),
            Some(EditTarget::CompareBundleB)
        );
    }

    #[test]
    fn numeric_edit_parsers_accept_empty_and_reject_invalid_values() {
        assert_eq!(parse_optional_usize("max parts", "").unwrap(), None);
        assert_eq!(parse_optional_usize("max parts", "12").unwrap(), Some(12));
        assert!(parse_optional_usize("max parts", "abc").is_err());

        assert_eq!(parse_optional_u64("max bytes", "").unwrap(), None);
        assert_eq!(parse_optional_u64("max bytes", "1024").unwrap(), Some(1024));
        assert!(parse_optional_u64("max bytes", "oops").is_err());
    }

    #[test]
    fn edit_status_lines_show_field_buffer_and_help() {
        let lines = edit_status_lines(&InputMode::Editing(EditTarget::HandoffMaxParts), "12");
        let joined = lines.join("\n");
        assert!(joined.contains("field: handoff.max_parts"));
        assert!(joined.contains("value: 12"));
        assert!(joined.contains("empty=resets to selected profile"));
    }

    #[test]
    fn summarize_child_output_prefers_stderr_on_failure() {
        let run = ChildRunResult {
            code: 3,
            stdout: "ignored".to_string(),
            stderr: "bad thing happened\nmore".to_string(),
        };
        assert_eq!(
            summarize_child_failure("preview", &run),
            "preview failed (exit=3): bad thing happened"
        );
    }

    #[test]
    fn summarize_child_success_uses_first_lines() {
        let run = ChildRunResult {
            code: 0,
            stdout: "line1\nline2\nline3".to_string(),
            stderr: String::new(),
        };
        assert_eq!(
            summarize_child_success("build", &run),
            "build ok:\nline1\nline2"
        );
    }

    #[test]
    fn handoff_overview_lists_flow_sections_and_command() {
        let handoff = HandoffState {
            plan: crate::plan::HandoffPlan {
                profile: Some("10x100".to_string()),
                include_staged: true,
                include: vec!["src/*.rs".to_string()],
                exclude: vec!["src/generated.rs".to_string()],
                split_by: "commit".to_string(),
                ..crate::plan::HandoffPlan::default()
            },
            plan_path: "diffship_plan.toml".to_string(),
            message: String::new(),
            preview_title: "parts/part_01.patch".to_string(),
            preview_lines: vec![],
            preview_scroll: 0,
        };
        let lines = handoff_overview_lines(&InputMode::Editing(EditTarget::HandoffOut), &handoff);
        let joined = lines.join("\n");
        assert!(joined.contains("1) Range"));
        assert!(joined.contains("2) Sources"));
        assert!(joined.contains("3) Filters"));
        assert!(joined.contains("include: src/*.rs"));
        assert!(joined.contains("exclude: src/generated.rs"));
        assert!(joined.contains("4) Split / Profile"));
        assert!(joined.contains("profile: 10x100"));
        assert!(joined.contains("max-parts: (auto)"));
        assert!(joined.contains("plan path   : diffship_plan.toml"));
        assert!(joined.contains("5) Preview: parts/part_01.patch"));
        assert!(joined.contains(
            "diffship build --profile 10x100 --include-staged --include 'src/*.rs' --exclude src/generated.rs --split-by commit"
        ));
    }

    #[test]
    fn compare_overview_lists_command_and_inputs() {
        let compare = CompareState {
            bundle_a: "out/a.zip".to_string(),
            bundle_b: "out/b.zip".to_string(),
            strict: true,
            message: String::new(),
            report_title: "compare (strict): different".to_string(),
            report_lines: vec![],
            report_scroll: 0,
        };
        let lines =
            compare_overview_lines(&InputMode::Editing(EditTarget::CompareBundleA), &compare);
        let joined = lines.join("\n");
        assert!(joined.contains("Compare"));
        assert!(joined.contains("editing compare.bundle_a"));
        assert!(joined.contains("diffship compare out/a.zip out/b.zip --strict"));
        assert!(joined.contains("bundle a    : out/a.zip"));
        assert!(joined.contains("bundle b    : out/b.zip"));
        assert!(joined.contains("strict      : yes"));
        assert!(joined.contains("3) Report: compare (strict): different"));
    }

    #[test]
    fn preview_line_style_classifies_diff_lines() {
        assert_eq!(preview_line_style("+added"), PreviewLineStyle::Added);
        assert_eq!(preview_line_style("-removed"), PreviewLineStyle::Removed);
        assert_eq!(preview_line_style("@@ hunk"), PreviewLineStyle::Hunk);
        assert_eq!(preview_line_style("diff --git"), PreviewLineStyle::Plain);
        assert_eq!(preview_line_style("--- a/file"), PreviewLineStyle::Plain);
        assert_eq!(preview_line_style("+++ b/file"), PreviewLineStyle::Plain);
    }

    #[test]
    fn read_preview_lines_includes_manifest_summary_when_present() {
        let td = tempfile::tempdir().expect("tempdir");
        let root = td.path();
        std::fs::create_dir_all(root.join("parts")).expect("parts dir");
        std::fs::write(
            root.join("handoff.manifest.json"),
            r#"{
    "summary": {
    "file_count": 2,
    "part_count": 1,
    "commit_view_count": 0,
    "categories": {
      "docs": 1,
      "config": 0,
      "source": 1,
      "tests": 0,
      "other": 0
    },
    "segments": {
      "committed": 2
    },
      "statuses": {
      "M": 1,
      "A": 1
    }
  },
  "reading_order": [
    "Source changes: `part_01.patch` (2 files)",
    "Other changes"
  ]
}
"#,
        )
        .expect("manifest");
        std::fs::write(
            root.join("parts").join("part_01.patch"),
            "diff --git a/a.txt b/a.txt\n+line\n",
        )
        .expect("patch");

        let (title, lines) = read_preview_lines(root).expect("preview lines");
        assert_eq!(title, "parts/part_01.patch");
        assert_eq!(
            lines.first().map(String::as_str),
            Some("# Structured context")
        );
        assert_eq!(
            lines.get(1).map(String::as_str),
            Some("files=2 parts=1 commit-views=0")
        );
        let joined = lines.join("\n");
        assert!(joined.contains("categories: config=0, docs=1, other=0, source=1, tests=0"));
        assert!(joined.contains("segments: committed=2"));
        assert!(joined.contains("statuses: A=1, M=1"));
        assert!(joined.contains("reading order:"));
        assert!(joined.contains("- Source changes: `part_01.patch` (2 files)"));
        assert!(joined.contains("- Other changes"));
        assert!(joined.contains("diff --git a/a.txt b/a.txt"));
    }

    #[test]
    fn render_compare_report_surfaces_structured_context_deltas() {
        let run = ChildRunResult {
            code: 1,
            stdout: r#"{
  "bundle_a": "a.zip",
  "bundle_b": "b.zip",
  "mode": "normalized",
  "equivalent": false,
  "areas": { "handoff": 1, "patch": 1 },
  "kinds": { "content_differs": 2 },
  "structured_context": {
    "manifest_a": true,
    "manifest_b": true,
    "summary_diffs": [
      { "key": "file_count", "a": 1, "b": 2 }
    ],
    "reading_order_diffs": [
      { "key": "reading_order[0]", "a": "Docs", "b": "Source" }
    ]
  },
  "diffs": [
    { "area": "patch", "kind": "content_differs", "path": "parts/part_01.patch", "detail": "content differs: parts/part_01.patch" }
  ]
}"#
            .to_string(),
            stderr: "bundle comparison failed (see JSON diff output)".to_string(),
        };

        let (title, lines) = render_compare_report(&run).expect("compare report");
        let joined = lines.join("\n");
        assert_eq!(title, "compare (normalized): different");
        assert!(joined.contains("equivalent=no"));
        assert!(joined.contains("areas: handoff=1, patch=1"));
        assert!(joined.contains("manifest summary diffs:"));
        assert!(joined.contains("- file_count: 1 -> 2"));
        assert!(joined.contains("manifest reading-order diffs:"));
        assert!(joined.contains(r#"- reading_order[0]: "Docs" -> "Source""#));
        assert!(joined.contains("file diffs:"));
        assert!(joined.contains("- [patch/content_differs] parts/part_01.patch"));
    }

    #[test]
    fn parse_pattern_list_accepts_commas_and_newlines() {
        assert_eq!(
            parse_pattern_list("src/*.rs, docs/*.md\nnotes.txt"),
            vec!["src/*.rs", "docs/*.md", "notes.txt"]
        );
    }

    #[test]
    fn current_plan_export_path_prefers_bundle_plan_when_out_is_set() {
        let handoff = HandoffState {
            plan: crate::plan::HandoffPlan {
                out: Some("bundle_out".to_string()),
                ..crate::plan::HandoffPlan::default()
            },
            plan_path: "diffship_plan.toml".to_string(),
            message: String::new(),
            preview_title: String::new(),
            preview_lines: vec![],
            preview_scroll: 0,
        };
        assert_eq!(current_plan_export_path(&handoff), "bundle_out/plan.toml");
    }
}
