use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::process::Command;

use crate::app::App;
use crate::backend::ScoopBackend;
use crate::ui::{footer_text_for_width, layout_for, AppLayout};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseOutcome {
    Continue,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FooterAction {
    Filter,
    Search,
    ToggleInstalled,
    Install,
    Uninstall,
    Update,
    Help,
    Quit,
}

pub trait BrowserOpener {
    fn open(&self, url: &str);
}

pub struct SystemBrowser;

impl BrowserOpener for SystemBrowser {
    fn open(&self, url: &str) {
        let mut command = default_browser_command(url);
        let _ = command.spawn();
    }
}

struct NoopBrowser;

impl BrowserOpener for NoopBrowser {
    fn open(&self, _url: &str) {}
}

pub fn handle_mouse_event<B: ScoopBackend>(
    app: &mut App,
    backend: &B,
    area: Rect,
    event: MouseEvent,
) -> MouseOutcome {
    handle_mouse_event_with_browser(app, backend, &NoopBrowser, area, event)
}

pub fn handle_mouse_event_with_browser<B: ScoopBackend, O: BrowserOpener>(
    app: &mut App,
    backend: &B,
    browser: &O,
    area: Rect,
    event: MouseEvent,
) -> MouseOutcome {
    match event.kind {
        MouseEventKind::ScrollUp => {
            app.move_up();
            MouseOutcome::Continue
        }
        MouseEventKind::ScrollDown => {
            app.move_down();
            MouseOutcome::Continue
        }
        MouseEventKind::Down(MouseButton::Left) => {
            handle_left_click(app, backend, browser, area, event)
        }
        _ => MouseOutcome::Continue,
    }
}

fn handle_left_click<B: ScoopBackend, O: BrowserOpener>(
    app: &mut App,
    backend: &B,
    browser: &O,
    area: Rect,
    event: MouseEvent,
) -> MouseOutcome {
    let layout = layout_for(area);
    let column = event.column;
    let row = event.row;

    if app.show_help {
        if contains(layout.help_modal, column, row) {
            app.close_help();
        }
        return MouseOutcome::Continue;
    }

    if app.setup_prompt.is_some() {
        if let Some(choice) = modal_choice(layout.setup_modal, column, row) {
            match choice {
                ModalChoice::Confirm => {
                    app.confirm_setup_prompt(backend);
                }
                ModalChoice::Cancel => app.cancel_setup_prompt(),
            }
        }
        return MouseOutcome::Continue;
    }

    if app.pending_action.is_some() {
        if let Some(choice) = modal_choice(layout.confirmation_modal, column, row) {
            match choice {
                ModalChoice::Confirm => {
                    app.confirm_pending_action(backend);
                }
                ModalChoice::Cancel => app.cancel_pending_action(),
            }
        }
        return MouseOutcome::Continue;
    }

    if search_field_contains(app, layout.header, column, row) {
        app.start_search();
        return MouseOutcome::Continue;
    }

    if let Some(homepage) = homepage_at(app, layout.details, column, row) {
        browser.open(homepage);
        return MouseOutcome::Continue;
    }

    if let Some(index) = package_row_at(&layout, column, row) {
        app.select_filtered_index(index);
        return MouseOutcome::Continue;
    }

    if let Some(action) = footer_action_at(app, layout.footer, column, row) {
        return run_footer_action(app, backend, action);
    }

    MouseOutcome::Continue
}

fn default_browser_command(url: &str) -> Command {
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(url);
        command
    }

    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    }
}

fn run_footer_action<B: ScoopBackend>(
    app: &mut App,
    backend: &B,
    action: FooterAction,
) -> MouseOutcome {
    match action {
        FooterAction::Filter => app.start_search(),
        FooterAction::Search => app.load_search_results(backend),
        FooterAction::ToggleInstalled => app.toggle_installed_view(backend),
        FooterAction::Install => {
            app.request_install_selected();
        }
        FooterAction::Uninstall => {
            app.request_uninstall_selected();
        }
        FooterAction::Update => {
            app.request_update_selected();
        }
        FooterAction::Help => app.open_help(),
        FooterAction::Quit => return MouseOutcome::Quit,
    }

    MouseOutcome::Continue
}

fn package_row_at(layout: &AppLayout, column: u16, row: u16) -> Option<usize> {
    if !contains(layout.packages, column, row) {
        return None;
    }

    let first_row = layout.packages.y.saturating_add(2);
    if row < first_row
        || row
            >= layout
                .packages
                .y
                .saturating_add(layout.packages.height)
                .saturating_sub(1)
    {
        return None;
    }

    Some(usize::from(row - first_row))
}

fn search_field_contains(app: &App, area: Rect, column: u16, row: u16) -> bool {
    if row != area.y.saturating_add(1)
        || column <= area.x
        || column >= area.x.saturating_add(area.width).saturating_sub(1)
    {
        return false;
    }

    let prefix = format!(
        "Scoop TUI {}  |  View: {}  |  Search: /",
        env!("CARGO_PKG_VERSION"),
        app.package_view.label()
    );
    let start = area
        .x
        .saturating_add(1)
        .saturating_add(prefix.chars().count() as u16);
    let query_width = app
        .search_query()
        .chars()
        .count()
        .max("<empty>".chars().count()) as u16;
    let end = start.saturating_add(query_width).saturating_add(1);
    column >= start && column <= end
}

fn homepage_at(app: &App, area: Rect, column: u16, row: u16) -> Option<&str> {
    if !contains(area, column, row) {
        return None;
    }

    let homepage = app.selected_package()?.homepage.as_deref()?;
    let homepage_row = area.y.saturating_add(5);
    if row != homepage_row {
        return None;
    }

    let start = area
        .x
        .saturating_add(1)
        .saturating_add("Homepage: ".chars().count() as u16);
    let end = start.saturating_add(homepage.chars().count() as u16);
    (column >= start && column < end).then_some(homepage)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModalChoice {
    Confirm,
    Cancel,
}

fn modal_choice(area: Rect, column: u16, row: u16) -> Option<ModalChoice> {
    if !contains(area, column, row) {
        return None;
    }

    let first_action_row = area.y.saturating_add(area.height).saturating_sub(4);
    let last_action_row = area.y.saturating_add(area.height).saturating_sub(1);
    if row < first_action_row || row >= last_action_row {
        return None;
    }

    let midpoint = area.x.saturating_add(area.width / 2);
    if column < midpoint {
        Some(ModalChoice::Confirm)
    } else {
        Some(ModalChoice::Cancel)
    }
}

fn footer_action_at(app: &App, area: Rect, column: u16, row: u16) -> Option<FooterAction> {
    if row != area.y.saturating_add(1)
        || column <= area.x
        || column >= area.x.saturating_add(area.width).saturating_sub(1)
    {
        return None;
    }

    let relative = usize::from(column - area.x - 1);
    let text = footer_text_for_width(app.input_mode, area.width.saturating_sub(2));
    [
        ("[/]", FooterAction::Filter),
        ("[s]", FooterAction::Search),
        ("[l]", FooterAction::ToggleInstalled),
        ("[i]", FooterAction::Install),
        ("[r]", FooterAction::Uninstall),
        ("[u]", FooterAction::Update),
        ("[?]", FooterAction::Help),
        ("[q]", FooterAction::Quit),
    ]
    .into_iter()
    .find_map(|(label, action)| {
        action_range(&text, label)
            .contains(&relative)
            .then_some(action)
    })
}

fn action_range(text: &str, label: &str) -> std::ops::Range<usize> {
    let Some(start_byte) = text.find(label) else {
        return 0..0;
    };
    let rest = &text[start_byte + label.len()..];
    let end_byte = rest
        .find('[')
        .map(|offset| start_byte + label.len() + offset)
        .unwrap_or(text.len());
    let start = text[..start_byte].chars().count();
    let end = text[..end_byte].chars().count();
    start..end
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    use crate::app::{App, InputMode, SetupPrompt};
    use crate::backend::MockBackend;
    use crate::backend::ScoopBackend;

    #[derive(Default)]
    struct RecordingBrowser {
        opened_urls: RefCell<Vec<String>>,
    }

    impl super::BrowserOpener for RecordingBrowser {
        fn open(&self, url: &str) {
            self.opened_urls.borrow_mut().push(url.to_string());
        }
    }

    #[test]
    fn scroll_wheel_moves_selection() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        app.move_down();
        let area = Rect::new(0, 0, 120, 32);

        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::ScrollUp, 4, 8),
        );
        assert_eq!(app.selected_index(), 0);

        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::ScrollDown, 4, 8),
        );
        assert_eq!(app.selected_index(), 1);
    }

    #[test]
    fn clicking_package_row_selects_that_filtered_row() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        let area = Rect::new(0, 0, 120, 32);

        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 2, 7),
        );

        assert_eq!(app.selected_index(), 2);
    }

    #[test]
    fn clicking_header_search_region_enters_filter_mode() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        let area = Rect::new(0, 0, 120, 32);

        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 52, 1),
        );

        assert_eq!(app.input_mode, InputMode::Search);
    }

    #[test]
    fn clicking_selected_package_homepage_opens_it_in_browser() {
        let backend = MockBackend;
        let browser = RecordingBrowser::default();
        let mut app = App::new(backend.packages());
        let area = Rect::new(0, 0, 120, 32);

        super::handle_mouse_event_with_browser(
            &mut app,
            &backend,
            &browser,
            area,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 100, 8),
        );

        assert_eq!(
            browser.opened_urls.borrow().as_slice(),
            ["https://www.7-zip.org"]
        );
    }

    #[test]
    fn clicking_footer_actions_runs_existing_actions() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        let area = Rect::new(0, 0, 120, 32);

        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 48, 30),
        );
        assert_eq!(app.package_view, crate::cache::PackageView::Search);

        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 74, 30),
        );
        assert_eq!(
            app.pending_action
                .as_ref()
                .map(|pending| pending.command_preview()),
            Some("scoop update 7zip".to_string())
        );
    }

    #[test]
    fn clicking_modal_confirm_and_cancel_use_pending_action_buttons() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        let area = Rect::new(0, 0, 120, 32);

        app.request_install_selected();
        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 45, 17),
        );
        assert_eq!(app.pending_action, None);
        assert!(app.output.iter().any(|line| line == "> scoop install 7zip"));

        app.request_install_selected();
        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 63, 17),
        );
        assert_eq!(app.pending_action, None);
        assert_eq!(
            app.output
                .iter()
                .filter(|line| line.as_str() == "> scoop install 7zip")
                .count(),
            1
        );
    }

    #[test]
    fn clicking_setup_modal_confirm_and_cancel_use_setup_buttons() {
        let backend = MockBackend;
        let mut app = App::new(backend.packages());
        let area = Rect::new(0, 0, 120, 32);

        app.setup_prompt = Some(SetupPrompt::InstallScoop);
        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 45, 18),
        );
        assert_eq!(app.setup_prompt, None);
        assert!(app.output.iter().any(|line| line.contains("get.scoop.sh")));

        app.setup_prompt = Some(SetupPrompt::InstallScoop);
        super::handle_mouse_event(
            &mut app,
            &backend,
            area,
            mouse_event(MouseEventKind::Down(MouseButton::Left), 75, 18),
        );
        assert_eq!(app.setup_prompt, None);
        assert_eq!(
            app.output
                .iter()
                .filter(|line| line.contains("get.scoop.sh"))
                .count(),
            1
        );
    }

    fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }
}
