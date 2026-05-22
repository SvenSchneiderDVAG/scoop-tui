use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::cursor::{DisableBlinking, EnableBlinking};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use scoop_tui::app::{App, InputMode, PackageAction, PendingAction};
#[cfg(not(target_os = "windows"))]
use scoop_tui::backend::MockBackend;
#[cfg(target_os = "windows")]
use scoop_tui::backend::ProductionBackend;
use scoop_tui::backend::{CommandResult, ScoopBackend};
use scoop_tui::input::SystemBrowser;
use scoop_tui::single_instance::SingleInstanceGuard;

fn main() -> Result<()> {
    let Some(_single_instance) = SingleInstanceGuard::acquire()? else {
        return Ok(());
    };

    #[cfg(target_os = "windows")]
    let backend = ProductionBackend::default();
    #[cfg(not(target_os = "windows"))]
    let backend = MockBackend;

    let mut app = App::new(Vec::new());
    let browser = SystemBrowser;
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBlinking
    )?;
    let terminal_backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(terminal_backend)?;

    app.check_scoop_available(&backend);

    let result = run(&mut terminal, &mut app, backend, &browser);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableBlinking,
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result
}

enum BackgroundCompletion {
    Setup(CommandResult),
    Search {
        result: CommandResult,
        requested_at: Instant,
    },
    Installed(CommandResult),
    PackageAction {
        result: CommandResult,
        installed_result: CommandResult,
    },
}

fn run<B: ScoopBackend + Clone + Send + 'static>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    backend: B,
    browser: &SystemBrowser,
) -> Result<()> {
    let (completion_tx, completion_rx) = mpsc::channel();
    if app.setup_prompt.is_none() {
        start_installed_command(app, &backend, &completion_tx);
    }

    loop {
        apply_background_completions(app, &completion_rx);
        app.tick();
        terminal.draw(|frame| scoop_tui::ui::draw(frame, app))?;

        if !event::poll(Duration::from_millis(120))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                let size = terminal.size()?;
                let area = Rect::new(0, 0, size.width, size.height);
                let layout = scoop_tui::ui::layout_for(area);
                let page_rows = usize::from(layout.packages.height.saturating_sub(3)).max(1);
                let log_rows = usize::from(layout.terminal.height.saturating_sub(2)).max(1);
                let help_rows = usize::from(layout.help_modal.height.saturating_sub(4)).max(1);

                if app.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => app.close_help(),
                        KeyCode::Down | KeyCode::Char('j') => app.scroll_help_down(),
                        KeyCode::Up | KeyCode::Char('k') => app.scroll_help_up(),
                        KeyCode::PageDown => app.scroll_help_page_down(help_rows),
                        KeyCode::PageUp => app.scroll_help_page_up(help_rows),
                        KeyCode::Home | KeyCode::Char('g') => app.scroll_help_to_top(),
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::PageUp if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.scroll_output_page_up(log_rows);
                        continue;
                    }
                    KeyCode::PageDown if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.scroll_output_page_down(log_rows);
                        continue;
                    }
                    KeyCode::Home if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.scroll_output_to_top();
                        continue;
                    }
                    KeyCode::End if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.scroll_output_to_bottom();
                        continue;
                    }
                    _ => {}
                }

                if app.activity.is_active() {
                    continue;
                }

                if app.setup_prompt.is_some() {
                    match key.code {
                        KeyCode::Char('y') => {
                            start_setup_command(app, &backend, &completion_tx);
                        }
                        KeyCode::Char('n') | KeyCode::Esc => app.cancel_setup_prompt(),
                        _ => {}
                    }
                    continue;
                }

                match app.input_mode {
                    InputMode::Browse => match key.code {
                        KeyCode::Char('y') if app.pending_action.is_some() => {
                            start_pending_action(app, &backend, &completion_tx);
                        }
                        KeyCode::Char('n') | KeyCode::Esc if app.pending_action.is_some() => {
                            app.cancel_pending_action();
                        }
                        KeyCode::Char('q') => break,
                        KeyCode::Char('?') => app.open_help(),
                        KeyCode::Char('/') => app.start_search(),
                        KeyCode::Char('c') => app.clear_search(),
                        KeyCode::Char('g') => app.move_first(),
                        KeyCode::Char('G') => app.move_last(),
                        KeyCode::Char('s') => start_search_command(app, &backend, &completion_tx),
                        KeyCode::Char('l') => {
                            start_toggle_installed_command(app, &backend, &completion_tx)
                        }
                        KeyCode::Char('i') => {
                            app.request_install_selected();
                        }
                        KeyCode::Char('r') => {
                            app.request_uninstall_selected();
                        }
                        KeyCode::Char('u') => {
                            app.request_update_selected();
                        }
                        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                        KeyCode::PageDown => app.move_page_down(page_rows),
                        KeyCode::PageUp => app.move_page_up(page_rows),
                        KeyCode::Home => app.move_first(),
                        KeyCode::End => app.move_last(),
                        _ => {}
                    },
                    InputMode::Search => match key.code {
                        KeyCode::Esc | KeyCode::Enter => app.stop_search(),
                        KeyCode::Char('?') => app.open_help(),
                        KeyCode::Backspace => app.pop_search_char(),
                        KeyCode::Char(value) => app.push_search_char(value),
                        _ => {}
                    },
                }
            }
            Event::Mouse(mouse) => {
                if app.activity.is_active() {
                    continue;
                }

                let size = terminal.size()?;
                let area = Rect::new(0, 0, size.width, size.height);
                if scoop_tui::input::handle_mouse_event_with_browser(
                    app, &backend, browser, area, mouse,
                ) == scoop_tui::input::MouseOutcome::Quit
                {
                    break;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn apply_background_completions(app: &mut App, completion_rx: &Receiver<BackgroundCompletion>) {
    while let Ok(completion) = completion_rx.try_recv() {
        match completion {
            BackgroundCompletion::Setup(result) => app.finish_setup_prompt(result),
            BackgroundCompletion::Search {
                result,
                requested_at,
            } => app.finish_search_results_at(result, requested_at),
            BackgroundCompletion::Installed(result) => app.finish_installed_view(result),
            BackgroundCompletion::PackageAction {
                result,
                installed_result,
            } => app.finish_pending_action(result, installed_result),
        }
    }
}

fn start_search_command<B: ScoopBackend + Clone + Send + 'static>(
    app: &mut App,
    backend: &B,
    completion_tx: &Sender<BackgroundCompletion>,
) {
    let requested_at = Instant::now();
    let Some(query) = app.begin_search_results_at(requested_at) else {
        return;
    };

    let backend = backend.clone();
    let completion_tx = completion_tx.clone();
    thread::spawn(move || {
        let result = backend.search(&query);
        let _ = completion_tx.send(BackgroundCompletion::Search {
            result,
            requested_at,
        });
    });
}

fn start_toggle_installed_command<B: ScoopBackend + Clone + Send + 'static>(
    app: &mut App,
    backend: &B,
    completion_tx: &Sender<BackgroundCompletion>,
) {
    if app.package_view == scoop_tui::cache::PackageView::Installed {
        start_search_command(app, backend, completion_tx);
        return;
    }

    start_installed_command(app, backend, completion_tx);
}

fn start_installed_command<B: ScoopBackend + Clone + Send + 'static>(
    app: &mut App,
    backend: &B,
    completion_tx: &Sender<BackgroundCompletion>,
) {
    app.begin_installed_view();
    let backend = backend.clone();
    let completion_tx = completion_tx.clone();
    thread::spawn(move || {
        let result = backend.installed();
        let _ = completion_tx.send(BackgroundCompletion::Installed(result));
    });
}

fn start_setup_command<B: ScoopBackend + Clone + Send + 'static>(
    app: &mut App,
    backend: &B,
    completion_tx: &Sender<BackgroundCompletion>,
) {
    if app.begin_setup_prompt().is_none() {
        return;
    }

    let backend = backend.clone();
    let completion_tx = completion_tx.clone();
    thread::spawn(move || {
        let result = backend.install_scoop();
        let _ = completion_tx.send(BackgroundCompletion::Setup(result));
    });
}

fn start_pending_action<B: ScoopBackend + Clone + Send + 'static>(
    app: &mut App,
    backend: &B,
    completion_tx: &Sender<BackgroundCompletion>,
) {
    let Some(pending) = app.begin_pending_action() else {
        return;
    };

    let backend = backend.clone();
    let completion_tx = completion_tx.clone();
    thread::spawn(move || {
        let result = run_pending_action(&backend, &pending);
        let installed_result = backend.installed();
        let _ = completion_tx.send(BackgroundCompletion::PackageAction {
            result,
            installed_result,
        });
    });
}

fn run_pending_action<B: ScoopBackend>(backend: &B, pending: &PendingAction) -> CommandResult {
    match pending.action {
        PackageAction::Install => backend.install(&pending.package_name),
        PackageAction::Uninstall => backend.uninstall(&pending.package_name),
        PackageAction::Update => backend.update(&pending.package_name),
    }
}
