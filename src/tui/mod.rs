use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::{lock, run, session, tasks, worktree};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    EditingBundle,
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
    user_tasks_path: PathBuf,
}

#[derive(Debug)]
struct App {
    screen: Screen,
    input_mode: InputMode,

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

        Ok(Self {
            screen: Screen::Runs,
            input_mode: InputMode::Normal,

            runs,
            selected_run: 0,
            show_detail: false,
            run_detail: None,

            status,

            bundle_input: String::new(),
            loop_message: "Bundle path is empty. Press i to type, Enter to run.".to_string(),
            loop_scroll: 0,

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
            "diffship TUI  |  [1]Runs [2]Status [3]Loop  |  r=refresh  q/Esc=quit",
            w,
        )?;
        writeln_trunc(&mut out, &line('-', w), w)?;

        match self.screen {
            Screen::Runs => self.draw_runs(&mut out, w, h)?,
            Screen::Status => self.draw_status(&mut out, w, h)?,
            Screen::Loop => self.draw_loop(&mut out, w, h)?,
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
                "  - tasks           : {}{}",
                d.user_tasks_path.display(),
                if tasks_present { "" } else { " (missing)" }
            ),
            w,
        )?;

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
            "Keys: i=edit path, Enter=run, c=clear message, ↑/↓ scroll",
            w,
        )?;

        writeln_trunc(out, "", w)?;
        let mode = match self.input_mode {
            InputMode::Normal => "normal",
            InputMode::EditingBundle => "editing",
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
        if self.input_mode == InputMode::EditingBundle {
            return self.handle_bundle_input(git_root, guard, key);
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
            KeyCode::Char('r') => {
                self.refresh_all(git_root)?;
                self.loop_message = "refreshed".to_string();
            }
            _ => {}
        }

        match self.screen {
            Screen::Runs => self.handle_runs_keys(git_root, key),
            Screen::Status => Ok(()),
            Screen::Loop => self.handle_loop_keys(git_root, guard, key),
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
                self.input_mode = InputMode::EditingBundle;
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

    fn handle_bundle_input(
        &mut self,
        git_root: &Path,
        guard: &mut TerminalGuard,
        key: KeyEvent,
    ) -> Result<(), ExitError> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
                if !self.bundle_input.trim().is_empty() {
                    self.run_loop_now(git_root, guard)?;
                }
            }
            KeyCode::Backspace => {
                self.bundle_input.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.bundle_input.clear();
            }
            KeyCode::Char(c) => {
                // Only accept printable characters.
                if !c.is_control() {
                    self.bundle_input.push(c);
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

    Ok(Some(RunDetail {
        meta,
        run_dir: run_dir.clone(),
        sandbox,
        apply,
        verify,
        promotion,
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

#[cfg(test)]
mod tests {
    use super::should_start_tui_impl;

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
}
