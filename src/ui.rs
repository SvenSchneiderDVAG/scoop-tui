use ratatui::prelude::*;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Padding, Paragraph, Row, Table, Wrap};

use crate::app::{App, InputMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppLayout {
    pub header: Rect,
    pub body: Rect,
    pub packages: Rect,
    pub details: Rect,
    pub footer: Rect,
    pub confirmation_modal: Rect,
    pub setup_modal: Rect,
    pub help_modal: Rect,
}

pub fn layout_for(area: Rect) -> AppLayout {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(74), Constraint::Percentage(26)])
        .split(root[1]);
    let side_panel = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(body[1]);

    AppLayout {
        header: root[0],
        body: root[1],
        packages: body[0],
        details: side_panel[0],
        footer: root[2],
        confirmation_modal: centered_rect(48, 24, area),
        setup_modal: centered_rect(74, 34, area),
        help_modal: centered_rect(58, 62, area),
    }
}

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let layout = layout_for(frame.area());

    draw_header(frame, layout.header, app);
    draw_body(frame, layout.body, app);
    draw_footer(frame, layout.footer, app);
    draw_confirmation_modal(frame, app);
    draw_setup_modal(frame, app);
    draw_help_modal(frame, app);

    if app.input_mode == InputMode::Search {
        frame.set_cursor_position(search_cursor_position(app, layout.header));
    }
}

fn draw_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let query = if app.search_query().is_empty() {
        "<empty>"
    } else {
        app.search_query()
    };
    let status = Line::from(vec![
        Span::styled(
            format!("Scoop TUI {}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  |  "),
        Span::styled("View: ", Style::default().fg(Color::Cyan)),
        Span::styled(app.package_view.label(), Style::default().fg(Color::White)),
        Span::raw("  |  "),
        Span::styled("Search: /", Style::default().fg(Color::Cyan)),
        Span::styled(
            query,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  "),
        Span::styled(
            format!(
                "Total: {}  Filtered: {}  |  Activity: ",
                app.total_count(),
                app.filtered_count()
            ),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(
            app.activity_label(),
            if app.activity.is_active() {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
    ]);
    let header =
        Paragraph::new(status).block(Block::default().borders(Borders::ALL).title("scoop"));
    frame.render_widget(header, area);
}

pub(crate) fn search_cursor_position(app: &App, area: Rect) -> Position {
    let prefix = format!(
        "Scoop TUI {}  |  View: {}  |  Search: /",
        env!("CARGO_PKG_VERSION"),
        app.package_view.label()
    );
    let query_width = app.search_query().chars().count() as u16;
    Position::new(
        area.x
            .saturating_add(1)
            .saturating_add(prefix.chars().count() as u16)
            .saturating_add(query_width),
        area.y.saturating_add(1),
    )
}

fn draw_body(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(74), Constraint::Percentage(26)])
        .split(area);

    draw_packages(frame, columns[0], app);
    draw_side_panel(frame, columns[1], app);
}

fn draw_packages(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let header = Row::new(vec![
        Cell::from("State"),
        Cell::from("Bucket"),
        Cell::from("Name"),
        Cell::from("Version"),
        Cell::from("Latest"),
    ])
    .style(Style::default().fg(Color::Blue));

    let rows = app
        .filtered_packages()
        .into_iter()
        .enumerate()
        .map(|(index, package)| {
            let style = if index == app.selected_index() {
                Style::default().fg(Color::Black).bg(Color::Gray)
            } else if package.update_available() {
                Style::default().fg(Color::Yellow)
            } else if package.installed {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Gray)
            };
            Row::new(vec![
                Cell::from(package.state().marker()),
                Cell::from(truncate_for_table(&package.source, 14)),
                Cell::from(truncate_for_table(&package.name, 24)),
                Cell::from(truncate_for_table(&package.version, 18)),
                Cell::from(truncate_for_table(package.latest_display(), 18)),
            ])
            .style(style)
        });

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Length(14),
            Constraint::Length(24),
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Min(1),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Packages"));
    frame.render_widget(table, area);
}

fn draw_side_panel(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(area);

    let details = app
        .selected_package()
        .map(|package| {
            Text::from(vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(Color::Yellow)),
                    Span::raw(package.name.clone()),
                ]),
                Line::from(vec![
                    Span::styled("Bucket: ", Style::default().fg(Color::Yellow)),
                    Span::raw(package.source.clone()),
                ]),
                Line::from(vec![
                    Span::styled("Version: ", Style::default().fg(Color::Yellow)),
                    Span::raw(package.version.clone()),
                ]),
                Line::from(vec![
                    Span::styled("Latest: ", Style::default().fg(Color::Yellow)),
                    Span::raw(package.latest_display().to_string()),
                ]),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::Yellow)),
                    Span::raw(package.state().label()),
                ]),
            ])
        })
        .unwrap_or_else(|| Text::from("No package selected"));

    let details = Paragraph::new(details)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL).title("Details"))
        .wrap(Wrap { trim: true });
    frame.render_widget(details, sections[0]);

    let output = if app.output.is_empty() {
        "No output yet".to_string()
    } else {
        app.output.join("\n")
    };
    let terminal_title = if app.activity.is_active() {
        format!("Terminal - {}", app.activity_label())
    } else {
        "Terminal".to_string()
    };
    let output = Paragraph::new(output)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL).title(terminal_title))
        .wrap(Wrap { trim: false });
    frame.render_widget(output, sections[1]);
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let footer = Paragraph::new(footer_text_for_width(
        app.input_mode,
        area.width.saturating_sub(2),
    ))
    .style(Style::default().fg(Color::Yellow))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, area);
}

pub(crate) fn footer_text_for_width(input_mode: crate::app::InputMode, width: u16) -> String {
    let mode = match input_mode {
        crate::app::InputMode::Browse => "Browse",
        crate::app::InputMode::Search => "Search",
    };
    let full = format!(
        "[{}]  [↑/↓] Navigate  [/] Filter  [s] Search Apps  [l] Toggle Installed  [i] Install  [r] Uninstall  [u] Update  [?] Help  [q] Quit",
        mode
    );
    if full.chars().count() <= usize::from(width) {
        return full;
    }

    let compact = format!(
        "[{}] [↑/↓] Nav [/] Filter [s] Search [l] Toggle [i] Inst [r] Uninst [u] Upd [?] Help [q] Quit",
        mode
    );
    if compact.chars().count() <= usize::from(width) {
        return compact;
    }

    format!("[{}] [↑/↓] [/] [s] [l] [i] [r] [u] [?] [q]", mode)
}

fn draw_confirmation_modal(frame: &mut Frame<'_>, app: &App) {
    let Some(pending) = &app.pending_action else {
        return;
    };

    let area = centered_rect(48, 24, frame.area());
    let message = format!(
        "Run this command?\n\n{}\n\n[y] Confirm    [n/Esc] Cancel",
        pending.command_preview()
    );
    let modal = Paragraph::new(message)
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Confirm Scoop Command")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, area);
    frame.render_widget(modal, area);
}

fn draw_setup_modal(frame: &mut Frame<'_>, app: &App) {
    let Some(prompt) = &app.setup_prompt else {
        return;
    };

    let area = centered_rect(74, 34, frame.area());
    let message = format!(
        "Scoop was not detected on this system.\n\nInstall Scoop for the current Windows user?\n\n{}\n\n[y] Install\n[n/Esc] Skip",
        prompt.command_preview()
    );
    let modal = Paragraph::new(message)
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Scoop Setup")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, area);
    frame.render_widget(modal, area);
}

fn draw_help_modal(frame: &mut Frame<'_>, app: &App) {
    if !app.show_help {
        return;
    }

    let area = centered_rect(58, 62, frame.area());
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Help")
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .padding(Padding::new(1, 1, 0, 0));
    let inner = block.inner(area);

    let rows = help_rows().into_iter().map(|row| match row {
        HelpRow::Section(title) => Row::new(vec![
            Cell::from(title).style(Style::default().fg(Color::Yellow)),
            Cell::from(""),
        ]),
        HelpRow::Binding { keys, description } => Row::new(vec![
            Cell::from(keys).style(Style::default().fg(Color::Cyan)),
            Cell::from(description),
        ]),
        HelpRow::Spacer => Row::new(vec![Cell::from(""), Cell::from("")]),
    });

    let table = Table::new(rows, [Constraint::Length(18), Constraint::Min(24)])
        .column_spacing(2)
        .style(Style::default().fg(Color::White).bg(Color::Black));
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(table, inner);
}

enum HelpRow {
    Section(&'static str),
    Binding {
        keys: &'static str,
        description: &'static str,
    },
    Spacer,
}

fn help_rows() -> Vec<HelpRow> {
    vec![
        HelpRow::Section("Navigation"),
        HelpRow::Binding {
            keys: "[↑/↓] or [k/j]",
            description: "Move package selection",
        },
        HelpRow::Spacer,
        HelpRow::Section("Views"),
        HelpRow::Binding {
            keys: "[s]",
            description: "Search available apps",
        },
        HelpRow::Binding {
            keys: "[l]",
            description: "Toggle installed/search view",
        },
        HelpRow::Spacer,
        HelpRow::Section("Search and Filter"),
        HelpRow::Binding {
            keys: "[/]",
            description: "Edit filter text",
        },
        HelpRow::Binding {
            keys: "[Enter] or [Esc]",
            description: "Leave filter mode",
        },
        HelpRow::Binding {
            keys: "[c]",
            description: "Clear filter",
        },
        HelpRow::Spacer,
        HelpRow::Section("Package Actions"),
        HelpRow::Binding {
            keys: "[i]",
            description: "Install selected package",
        },
        HelpRow::Binding {
            keys: "[u]",
            description: "Update selected package",
        },
        HelpRow::Binding {
            keys: "[r]",
            description: "Uninstall selected package",
        },
        HelpRow::Spacer,
        HelpRow::Section("Confirmation"),
        HelpRow::Binding {
            keys: "[y]",
            description: "Confirm pending command",
        },
        HelpRow::Binding {
            keys: "[n] or [Esc]",
            description: "Cancel pending command",
        },
        HelpRow::Spacer,
        HelpRow::Section("Global"),
        HelpRow::Binding {
            keys: "[?]",
            description: "Toggle this help",
        },
        HelpRow::Binding {
            keys: "[q]",
            description: "Quit",
        },
    ]
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn truncate_for_table(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    if max_chars <= 1 {
        return "…".to_string();
    }

    let keep = max_chars - 1;
    let mut truncated = value.chars().take(keep).collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use ratatui::backend::{Backend, TestBackend};
    use ratatui::layout::Position;
    use ratatui::Terminal;

    use super::*;
    use crate::backend::{MockBackend, ScoopBackend};

    #[test]
    fn renders_core_regions_and_mock_package_data() {
        let backend = MockBackend;
        let app = App::new(backend.packages());
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).expect("test terminal");

        terminal
            .draw(|frame| draw(frame, &app))
            .expect("render frame");

        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Scoop TUI"));
        assert!(rendered.contains("Activity:"));
        assert!(rendered.contains("Packages"));
        assert!(rendered.contains("Details"));
        assert!(rendered.contains("Terminal"));
        assert!(rendered.contains("7zip"));
    }

    #[test]
    fn positions_terminal_cursor_at_search_end_in_search_mode() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        app.start_search();
        app.set_search_query("git");
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).expect("test terminal");

        terminal
            .draw(|frame| draw(frame, &app))
            .expect("render frame");

        let cursor_position = terminal
            .backend_mut()
            .get_cursor_position()
            .expect("cursor position");
        assert_eq!(cursor_position, Position::new(54, 1));
    }

    #[test]
    fn renders_confirmation_modal_for_pending_action() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        app.request_install_selected();
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).expect("test terminal");

        terminal
            .draw(|frame| draw(frame, &app))
            .expect("render frame");

        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Confirm Scoop Command"));
        assert!(rendered.contains("scoop install 7zip"));
        assert!(rendered.contains("[y] Confirm"));
    }

    #[test]
    fn renders_setup_modal_for_missing_scoop_prompt() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        app.setup_prompt = Some(crate::app::SetupPrompt::InstallScoop);
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).expect("test terminal");

        terminal
            .draw(|frame| draw(frame, &app))
            .expect("render frame");

        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Scoop Setup"));
        assert!(rendered.contains("Scoop was not detected"));
        assert!(rendered.contains("get.scoop.sh"));
        assert!(rendered.contains("[y] Install"));
    }

    #[test]
    fn renders_help_modal_with_grouped_keymap() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        app.open_help();
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).expect("test terminal");

        terminal
            .draw(|frame| draw(frame, &app))
            .expect("render frame");

        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Help"));
        assert!(rendered.contains("Navigation"));
        assert!(rendered.contains("Package Actions"));
        assert!(rendered.contains("[i]"));
        assert!(rendered.contains("Install selected package"));
        assert!(rendered.contains("[r]"));
        assert!(rendered.contains("Uninstall selected package"));
    }

    #[test]
    fn renders_active_activity_in_terminal_title() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        app.activity = crate::app::Activity::RunningCommand {
            action: crate::app::PackageAction::Install,
            package_name: "gitui".to_string(),
        };
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).expect("test terminal");

        terminal
            .draw(|frame| draw(frame, &app))
            .expect("render frame");

        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Activity:"));
        assert!(rendered.contains("Installing gitui"));
        assert!(rendered.contains("Terminal - Installing gitui"));
    }

    #[test]
    fn table_truncates_long_names_but_details_keep_full_name() {
        let long_name = "this-is-an-extremely-long-package-name-that-should-remain-readable";
        let app = App::new(vec![crate::models::Package {
            source: "very-long-bucket-name-that-will-truncate".to_string(),
            name: long_name.to_string(),
            version: "2026.05.20-build-with-long-suffix".to_string(),
            latest_version: None,
            description: "A package with intentionally long fields".to_string(),
            installed: false,
            homepage: Some("https://example.com/very/long/path/that/should/wrap".to_string()),
        }]);
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).expect("test terminal");

        terminal
            .draw(|frame| draw(frame, &app))
            .expect("render frame");

        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert_eq!(
            truncate_for_table(long_name, 24),
            "this-is-an-extremely-lo…"
        );
        assert!(rendered.contains("…"));
        assert!(rendered.contains("Name:"));
        assert_eq!(
            app.selected_package().map(|package| package.name.as_str()),
            Some(long_name)
        );
    }

    #[test]
    fn footer_uses_compact_labels_when_width_is_limited() {
        let text = footer_text_for_width(crate::app::InputMode::Browse, 100);

        assert!(text.contains("[s] Search"));
        assert!(text.contains("[l] Toggle"));
        assert!(!text.contains("Search Apps"));
        assert!(text.chars().count() <= 100);
    }

    #[test]
    fn footer_falls_back_to_key_only_labels_at_very_small_widths() {
        let text = footer_text_for_width(crate::app::InputMode::Browse, 48);

        assert!(text.contains("[s]"));
        assert!(text.contains("[q]"));
        assert!(!text.contains("Search"));
        assert!(text.chars().count() <= 48);
    }
}
