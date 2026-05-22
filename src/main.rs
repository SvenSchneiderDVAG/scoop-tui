use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::cursor::{DisableBlinking, EnableBlinking};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use scoop_tui::app::{App, InputMode};
#[cfg(not(target_os = "windows"))]
use scoop_tui::backend::MockBackend;
#[cfg(target_os = "windows")]
use scoop_tui::backend::ProductionBackend;
use scoop_tui::backend::ScoopBackend;
use scoop_tui::input::SystemBrowser;

fn main() -> Result<()> {
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
    if app.setup_prompt.is_none() {
        app.show_installed_view(&backend);
    }

    let result = run(&mut terminal, &mut app, &backend, &browser);

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

fn run<B: ScoopBackend>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    backend: &B,
    browser: &SystemBrowser,
) -> Result<()> {
    loop {
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

                if app.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => app.close_help(),
                        _ => {}
                    }
                    continue;
                }

                if app.setup_prompt.is_some() {
                    match key.code {
                        KeyCode::Char('y') => {
                            app.confirm_setup_prompt(backend);
                        }
                        KeyCode::Char('n') | KeyCode::Esc => app.cancel_setup_prompt(),
                        _ => {}
                    }
                    continue;
                }

                match app.input_mode {
                    InputMode::Browse => match key.code {
                        KeyCode::Char('y') if app.pending_action.is_some() => {
                            app.confirm_pending_action(backend);
                        }
                        KeyCode::Char('n') | KeyCode::Esc if app.pending_action.is_some() => {
                            app.cancel_pending_action();
                        }
                        KeyCode::Char('q') => break,
                        KeyCode::Char('?') => app.open_help(),
                        KeyCode::Char('/') => app.start_search(),
                        KeyCode::Char('c') => app.clear_search(),
                        KeyCode::Char('s') => app.load_search_results(backend),
                        KeyCode::Char('l') => app.toggle_installed_view(backend),
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
                let size = terminal.size()?;
                let area = Rect::new(0, 0, size.width, size.height);
                if scoop_tui::input::handle_mouse_event_with_browser(
                    app, backend, browser, area, mouse,
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
