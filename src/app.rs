use std::time::{Duration, Instant};

use crate::backend::{CommandResult, ScoopBackend, SCOOP_INSTALL_COMMAND};
use crate::cache::{PackageCache, PackageCacheKey, PackageView};
use crate::models::Package;

const DEFAULT_PACKAGE_CACHE_TTL: Duration = Duration::from_secs(90);
const MAX_OUTPUT_LINES: usize = 500;
const SPINNER_FRAMES: [&str; 8] = ["|", "/", "-", "\\", "|", "/", "-", "\\"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Browse,
    Search,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackageAction {
    Install,
    Uninstall,
    Update,
}

impl PackageAction {
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
            Self::Update => "update",
        }
    }

    pub fn gerund(&self) -> &'static str {
        match self {
            Self::Install => "Installing",
            Self::Uninstall => "Uninstalling",
            Self::Update => "Updating",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingAction {
    pub action: PackageAction,
    pub package_name: String,
}

impl PendingAction {
    pub fn command_preview(&self) -> String {
        format!("scoop {} {}", self.action.verb(), self.package_name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupPrompt {
    InstallScoop,
}

impl SetupPrompt {
    pub fn command_preview(&self) -> &'static str {
        match self {
            Self::InstallScoop => SCOOP_INSTALL_COMMAND,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activity {
    Idle,
    Searching,
    LoadingInstalled,
    InstallingScoop,
    RunningCommand {
        action: PackageAction,
        package_name: String,
    },
}

impl Activity {
    pub fn label(&self) -> String {
        match self {
            Self::Idle => "Idle".to_string(),
            Self::Searching => "Searching".to_string(),
            Self::LoadingInstalled => "Loading installed".to_string(),
            Self::InstallingScoop => "Installing Scoop".to_string(),
            Self::RunningCommand {
                action,
                package_name,
            } => {
                format!("{} {}", action.gerund(), package_name)
            }
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Idle)
    }
}

#[derive(Debug)]
pub struct App {
    pub packages: Vec<Package>,
    search_query: String,
    selected: usize,
    pub output: Vec<String>,
    pub input_mode: InputMode,
    pub package_view: PackageView,
    pub pending_action: Option<PendingAction>,
    pub setup_prompt: Option<SetupPrompt>,
    pub show_help: bool,
    pub help_scroll: usize,
    pub activity: Activity,
    pub output_scroll: usize,
    spinner_index: usize,
    package_cache: PackageCache,
    installed_packages: Vec<Package>,
}

impl App {
    pub fn new(packages: Vec<Package>) -> Self {
        Self {
            packages,
            search_query: String::new(),
            selected: 0,
            output: Vec::new(),
            input_mode: InputMode::Browse,
            package_view: PackageView::Installed,
            pending_action: None,
            setup_prompt: None,
            show_help: false,
            help_scroll: 0,
            activity: Activity::Idle,
            output_scroll: 0,
            spinner_index: 0,
            package_cache: PackageCache::new(DEFAULT_PACKAGE_CACHE_TTL),
            installed_packages: Vec::new(),
        }
    }

    #[cfg(test)]
    fn with_cache_ttl(packages: Vec<Package>, ttl: Duration) -> Self {
        let mut app = Self::new(packages);
        app.package_cache = PackageCache::new(ttl);
        app
    }

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    pub fn selected_package(&self) -> Option<&Package> {
        self.filtered_indices()
            .get(self.selected)
            .and_then(|index| self.packages.get(*index))
    }

    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        self.clamp_selection();
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn select_filtered_index(&mut self, index: usize) -> bool {
        if index >= self.filtered_indices().len() {
            return false;
        }

        self.selected = index;
        true
    }

    pub fn move_down(&mut self) {
        let len = self.filtered_indices().len();
        if len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_page_down(&mut self, rows: usize) {
        let len = self.filtered_indices().len();
        if len > 0 {
            self.selected = self.selected.saturating_add(rows.max(1)).min(len - 1);
        }
    }

    pub fn move_page_up(&mut self, rows: usize) {
        self.selected = self.selected.saturating_sub(rows.max(1));
    }

    pub fn move_first(&mut self) {
        self.selected = 0;
    }

    pub fn move_last(&mut self) {
        let len = self.filtered_indices().len();
        if len > 0 {
            self.selected = len - 1;
        }
    }

    pub fn filtered_packages(&self) -> Vec<&Package> {
        self.filtered_indices()
            .into_iter()
            .filter_map(|index| self.packages.get(index))
            .collect()
    }

    pub fn request_install_selected(&mut self) -> Option<&PendingAction> {
        self.request_selected_action(PackageAction::Install)
    }

    pub fn request_uninstall_selected(&mut self) -> Option<&PendingAction> {
        self.request_selected_action(PackageAction::Uninstall)
    }

    pub fn request_update_selected(&mut self) -> Option<&PendingAction> {
        self.request_selected_action(PackageAction::Update)
    }

    pub fn request_update_all_available(&mut self) -> Option<&PendingAction> {
        self.packages
            .iter()
            .any(|package| package.update_available())
            .then_some(())?;
        self.pending_action = Some(PendingAction {
            action: PackageAction::Update,
            package_name: "*".to_string(),
        });
        self.pending_action.as_ref()
    }

    pub fn check_scoop_available<B: ScoopBackend>(&mut self, backend: &B) {
        if !backend.is_scoop_available() {
            self.setup_prompt = Some(SetupPrompt::InstallScoop);
        }
    }

    pub fn cancel_setup_prompt(&mut self) {
        self.setup_prompt = None;
    }

    pub fn confirm_setup_prompt<B: ScoopBackend>(&mut self, backend: &B) -> Option<CommandResult> {
        self.begin_setup_prompt()?;
        let result = backend.install_scoop();
        self.finish_setup_prompt(result.clone());
        Some(result)
    }

    pub fn begin_setup_prompt(&mut self) -> Option<()> {
        self.setup_prompt.take()?;
        self.activity = Activity::InstallingScoop;
        Some(())
    }

    pub fn finish_setup_prompt(&mut self, result: CommandResult) {
        self.record_command(&result);
        self.activity = Activity::Idle;
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_action = None;
    }

    pub fn open_help(&mut self) {
        self.show_help = true;
        self.help_scroll = 0;
    }

    pub fn close_help(&mut self) {
        self.show_help = false;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        if self.show_help {
            self.help_scroll = 0;
        }
    }

    pub fn confirm_pending_action<B: ScoopBackend>(
        &mut self,
        backend: &B,
    ) -> Option<CommandResult> {
        let pending = self.begin_pending_action()?;
        let result = match pending.action {
            PackageAction::Install => backend.install(&pending.package_name),
            PackageAction::Uninstall => backend.uninstall(&pending.package_name),
            PackageAction::Update => backend.update(&pending.package_name),
        };
        let installed_result = backend.installed();
        self.finish_pending_action(result.clone(), installed_result);
        Some(result)
    }

    pub fn begin_pending_action(&mut self) -> Option<PendingAction> {
        let pending = self.pending_action.take()?;
        self.activity = Activity::RunningCommand {
            action: pending.action.clone(),
            package_name: pending.package_name.clone(),
        };
        Some(pending)
    }

    pub fn finish_pending_action(
        &mut self,
        result: CommandResult,
        installed_result: CommandResult,
    ) {
        let selected_name = self.selected_package().map(|package| package.name.clone());
        self.record_command(&result);
        self.installed_packages = installed_result.packages;
        match self.package_view {
            PackageView::Installed => {
                self.packages = self.installed_packages.clone();
                self.restore_selection_by_name(selected_name.as_deref());
            }
            PackageView::Search => {
                self.packages = self.with_installed_state(self.packages.clone());
            }
        }
        self.activity = Activity::Idle;
    }

    pub fn load_search_results<B: ScoopBackend>(&mut self, backend: &B) {
        self.load_search_results_at(backend, Instant::now());
    }

    pub fn load_search_results_at<B: ScoopBackend>(&mut self, backend: &B, now: Instant) {
        let Some(query) = self.begin_search_results_at(now) else {
            return;
        };
        let result = backend.search(&query);
        self.finish_search_results_at(result, now);
    }

    pub fn begin_search_results(&mut self) -> Option<String> {
        self.begin_search_results_at(Instant::now())
    }

    pub fn begin_search_results_at(&mut self, now: Instant) -> Option<String> {
        let key = PackageCacheKey::new(PackageView::Search, &self.search_query);
        self.package_view = PackageView::Search;

        if let Some(packages) = self.package_cache.get(&key, now) {
            self.packages = self.with_installed_state(packages);
            self.selected = 0;
            return None;
        }

        self.activity = Activity::Searching;
        Some(self.search_query.clone())
    }

    pub fn finish_search_results(&mut self, result: CommandResult) {
        self.finish_search_results_at(result, Instant::now());
    }

    pub fn finish_search_results_at(&mut self, result: CommandResult, now: Instant) {
        let key = PackageCacheKey::new(PackageView::Search, &self.search_query);
        let packages = self.with_installed_state(result.packages);
        self.packages = packages.clone();
        self.package_cache.insert(key, packages, now);
        self.selected = 0;
        self.activity = Activity::Idle;
    }

    pub fn start_search(&mut self) {
        self.input_mode = InputMode::Search;
    }

    pub fn stop_search(&mut self) {
        self.input_mode = InputMode::Browse;
    }

    pub fn push_search_char(&mut self, value: char) {
        self.search_query.push(value);
        self.clamp_selection();
    }

    pub fn pop_search_char(&mut self) {
        self.search_query.pop();
        self.clamp_selection();
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.clamp_selection();
    }

    pub fn show_installed_view<B: ScoopBackend>(&mut self, backend: &B) {
        self.begin_installed_view();
        let result = backend.installed();
        self.finish_installed_view(result);
    }

    pub fn begin_installed_view(&mut self) {
        self.package_view = PackageView::Installed;
        self.activity = Activity::LoadingInstalled;
    }

    pub fn finish_installed_view(&mut self, result: CommandResult) {
        self.installed_packages = result.packages.clone();
        self.packages = self.installed_packages.clone();
        self.selected = 0;
        self.activity = Activity::Idle;
    }

    pub fn toggle_installed_view<B: ScoopBackend>(&mut self, backend: &B) {
        self.toggle_installed_view_at(backend, Instant::now());
    }

    pub fn toggle_installed_view_at<B: ScoopBackend>(&mut self, backend: &B, now: Instant) {
        match self.package_view {
            PackageView::Installed => self.load_search_results_at(backend, now),
            PackageView::Search => self.show_installed_view(backend),
        }
    }

    pub fn total_count(&self) -> usize {
        self.packages.len()
    }

    pub fn filtered_count(&self) -> usize {
        self.filtered_indices().len()
    }

    pub fn tick(&mut self) {
        self.spinner_index = (self.spinner_index + 1) % SPINNER_FRAMES.len();
    }

    pub fn spinner_frame(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_index]
    }

    pub fn activity_label(&self) -> String {
        if self.activity.is_active() {
            format!("{} {}", self.activity.label(), self.spinner_frame())
        } else {
            self.activity.label()
        }
    }

    pub fn scroll_output_up(&mut self) {
        self.output_scroll = self
            .output_scroll
            .saturating_add(1)
            .min(self.output.len().saturating_sub(1));
    }

    pub fn scroll_output_down(&mut self) {
        self.output_scroll = self.output_scroll.saturating_sub(1);
    }

    pub fn scroll_output_page_up(&mut self, rows: usize) {
        self.output_scroll = self
            .output_scroll
            .saturating_add(rows.max(1))
            .min(self.output.len().saturating_sub(1));
    }

    pub fn scroll_output_page_down(&mut self, rows: usize) {
        self.output_scroll = self.output_scroll.saturating_sub(rows.max(1));
    }

    pub fn scroll_output_to_top(&mut self) {
        self.output_scroll = self.output.len().saturating_sub(1);
    }

    pub fn scroll_output_to_bottom(&mut self) {
        self.output_scroll = 0;
    }

    pub fn scroll_help_up(&mut self) {
        self.help_scroll = self.help_scroll.saturating_sub(1);
    }

    pub fn scroll_help_down(&mut self) {
        self.help_scroll = self.help_scroll.saturating_add(1);
    }

    pub fn scroll_help_page_up(&mut self, rows: usize) {
        self.help_scroll = self.help_scroll.saturating_sub(rows.max(1));
    }

    pub fn scroll_help_page_down(&mut self, rows: usize) {
        self.help_scroll = self.help_scroll.saturating_add(rows.max(1));
    }

    pub fn scroll_help_to_top(&mut self) {
        self.help_scroll = 0;
    }

    fn filtered_indices(&self) -> Vec<usize> {
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            return (0..self.packages.len()).collect();
        }

        self.packages
            .iter()
            .enumerate()
            .filter_map(|(index, package)| {
                let haystack = format!(
                    "{} {} {} {}",
                    package.source, package.name, package.version, package.description
                )
                .to_lowercase();
                haystack.contains(&query).then_some(index)
            })
            .collect()
    }

    fn clamp_selection(&mut self) {
        let len = self.filtered_indices().len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    fn record_command(&mut self, result: &CommandResult) {
        self.output.push(format!("> {}", result.command));
        self.output.extend(display_output_lines(result));
        if self.output.len() > MAX_OUTPUT_LINES {
            let overflow = self.output.len() - MAX_OUTPUT_LINES;
            self.output.drain(0..overflow);
        }
        self.scroll_output_to_bottom();
    }

    fn restore_selection_by_name(&mut self, name: Option<&str>) {
        self.selected = name
            .and_then(|name| {
                self.filtered_packages()
                    .into_iter()
                    .position(|package| package.name == name)
            })
            .unwrap_or(0);
        self.clamp_selection();
    }

    fn with_installed_state(&self, packages: Vec<Package>) -> Vec<Package> {
        packages
            .into_iter()
            .map(|mut package| {
                if self
                    .installed_packages
                    .iter()
                    .any(|installed_package| installed_package.name == package.name)
                {
                    package.installed = true;
                }
                package
            })
            .collect()
    }

    fn request_selected_action(&mut self, action: PackageAction) -> Option<&PendingAction> {
        let package_name = self.selected_package()?.name.clone();
        self.pending_action = Some(PendingAction {
            action,
            package_name,
        });
        self.pending_action.as_ref()
    }
}

fn display_output_lines(result: &CommandResult) -> Vec<String> {
    let Some((action, package_name)) = package_command(&result.command) else {
        return result.output.lines().map(str::to_string).collect();
    };
    let package_label = package_output_label(package_name);

    let failed = output_looks_failed(&result.output);
    let past_tense = match action {
        "install" => "Installed",
        "uninstall" => "Uninstalled",
        "update" => {
            if output_looks_current(&result.output) {
                "Already current"
            } else {
                "Updated"
            }
        }
        _ => return result.output.lines().map(str::to_string).collect(),
    };
    let in_progress = match action {
        "install" => "Installing",
        "uninstall" => "Uninstalling",
        "update" => "Updating",
        _ => unreachable!("package_command only returns supported actions"),
    };

    if failed {
        let failure = match action {
            "install" => "Install failed",
            "uninstall" => "Uninstall failed",
            "update" => "Update failed",
            _ => unreachable!("package_command only returns supported actions"),
        };
        let mut lines = vec![format!("{failure}: {package_label}")];
        if let Some(message) = first_meaningful_output_line(&result.output) {
            lines.push(message.to_string());
        }
        return lines;
    }

    vec![
        format!("{in_progress}: {package_label}"),
        format!("{past_tense}: {package_label}"),
    ]
}

fn package_output_label(package_name: &str) -> &str {
    if package_name == "*" {
        "all packages"
    } else {
        package_name
    }
}

fn package_command(command: &str) -> Option<(&str, &str)> {
    let mut parts = command.split_whitespace();
    if parts.next()? != "scoop" {
        return None;
    }

    let action = parts.next()?;
    if !matches!(action, "install" | "uninstall" | "update") {
        return None;
    }

    let package_name = parts.next()?;
    Some((action, package_name))
}

fn output_looks_current(output: &str) -> bool {
    let output = output.to_lowercase();
    output.contains("already up to date") || output.contains("is up to date")
}

fn output_looks_failed(output: &str) -> bool {
    let output = output.to_lowercase();
    [
        "failed",
        "error",
        "not found",
        "could not",
        "couldn't",
        "konnte nicht",
        "denied",
    ]
    .iter()
    .any(|needle| output.contains(needle))
}

fn first_meaningful_output_line(output: &str) -> Option<&str> {
    output.lines().map(str::trim).find(|line| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::CommandResult;
    use std::cell::Cell;

    struct RecordingBackend {
        search_calls: Cell<usize>,
        installed_calls: Cell<usize>,
        scoop_available: bool,
    }

    impl RecordingBackend {
        fn new() -> Self {
            Self {
                search_calls: Cell::new(0),
                installed_calls: Cell::new(0),
                scoop_available: true,
            }
        }

        fn without_scoop() -> Self {
            Self {
                search_calls: Cell::new(0),
                installed_calls: Cell::new(0),
                scoop_available: false,
            }
        }
    }

    impl ScoopBackend for RecordingBackend {
        fn is_scoop_available(&self) -> bool {
            self.scoop_available
        }

        fn environment(&self) -> crate::discovery::ScoopEnvironment {
            crate::discovery::ScoopEnvironment::from_env_map(std::collections::HashMap::from([
                ("SCOOP".to_string(), r"C:\Users\Test\scoop".to_string()),
                (
                    "SCOOP_GLOBAL".to_string(),
                    r"C:\ProgramData\scoop".to_string(),
                ),
            ]))
        }

        fn packages(&self) -> Vec<Package> {
            packages()
        }

        fn installed(&self) -> CommandResult {
            self.installed_calls.set(self.installed_calls.get() + 1);
            CommandResult {
                command: "scoop list".to_string(),
                output: "Listed installed packages".to_string(),
                packages: packages()
                    .into_iter()
                    .filter(|package| package.installed)
                    .collect(),
            }
        }

        fn search(&self, name: &str) -> CommandResult {
            self.search_calls.set(self.search_calls.get() + 1);
            let query = name.to_lowercase();
            CommandResult {
                command: format!("scoop search {query}"),
                output: format!("Found packages for {query}"),
                packages: packages()
                    .into_iter()
                    .filter(|package| package.name.to_lowercase().contains(&query))
                    .collect(),
            }
        }

        fn install_scoop(&self) -> CommandResult {
            CommandResult {
                command: SCOOP_INSTALL_COMMAND.to_string(),
                output: "Installed Scoop".to_string(),
                packages: Vec::new(),
            }
        }

        fn install(&self, name: &str) -> CommandResult {
            CommandResult {
                command: format!("scoop install {name}"),
                output: format!("Installed {name}"),
                packages: Vec::new(),
            }
        }

        fn uninstall(&self, name: &str) -> CommandResult {
            CommandResult {
                command: format!("scoop uninstall {name}"),
                output: format!("Uninstalled {name}"),
                packages: Vec::new(),
            }
        }

        fn update(&self, name: &str) -> CommandResult {
            CommandResult {
                command: format!("scoop update {name}"),
                output: format!("Updated {name}"),
                packages: Vec::new(),
            }
        }
    }

    #[test]
    fn filters_packages_by_name_and_description_case_insensitively() {
        let mut app = App::new(packages());

        app.set_search_query("git");

        let names: Vec<_> = app
            .filtered_packages()
            .into_iter()
            .map(|package| package.name.as_str())
            .collect();
        assert_eq!(names, vec!["git", "gitui"]);
        assert_eq!(app.search_query(), "git");
    }

    #[test]
    fn clamps_selection_to_filtered_result_count() {
        let mut app = App::new(packages());

        app.move_down();
        app.move_down();
        app.set_search_query("gitui");

        assert_eq!(app.selected_index(), 0);
        assert_eq!(
            app.selected_package().map(|package| package.name.as_str()),
            Some("gitui")
        );
    }

    #[test]
    fn selects_package_by_filtered_row_index() {
        let mut app = App::new(packages());
        app.set_search_query("git");

        assert!(app.select_filtered_index(1));

        assert_eq!(app.selected_index(), 1);
        assert_eq!(
            app.selected_package().map(|package| package.name.as_str()),
            Some("gitui")
        );
    }

    #[test]
    fn opens_confirmation_for_selected_install_without_running_command() {
        let mut app = App::new(packages());
        app.set_search_query("gitui");

        let pending = app
            .request_install_selected()
            .expect("pending install action");

        assert_eq!(pending.command_preview(), "scoop install gitui");
        assert!(app.output.is_empty());
    }

    #[test]
    fn confirms_pending_update_and_records_integrated_terminal_output() {
        let mut app = App::new(packages());
        app.move_down();
        app.request_update_selected();

        let result = app
            .confirm_pending_action(&RecordingBackend::new())
            .expect("confirmed action");

        assert_eq!(result.command, "scoop update git");
        assert_eq!(result.output, "Updated git");
        assert_eq!(app.pending_action, None);
        assert!(app.output.iter().any(|line| line == "> scoop update git"));
        assert!(app.output.iter().any(|line| line == "Updated: git"));
    }

    #[test]
    fn requests_update_all_available_packages_as_scoop_star_update() {
        let mut packages = packages();
        packages[0].latest_version = Some("26.01".to_string());
        packages[1].latest_version = Some("2.50.0".to_string());
        let mut app = App::new(packages);

        let pending = app
            .request_update_all_available()
            .expect("pending bulk update action");

        assert_eq!(pending.command_preview(), "scoop update *");
    }

    #[test]
    fn skips_update_all_when_no_packages_have_updates() {
        let mut app = App::new(packages());

        assert_eq!(app.request_update_all_available(), None);
        assert_eq!(app.pending_action, None);
    }

    #[test]
    fn confirms_update_all_and_records_simple_terminal_output() {
        let mut packages = packages();
        packages[0].latest_version = Some("26.01".to_string());
        let mut app = App::new(packages);
        app.request_update_all_available();

        let result = app
            .confirm_pending_action(&RecordingBackend::new())
            .expect("confirmed bulk action");

        assert_eq!(result.command, "scoop update *");
        assert_eq!(
            app.output,
            vec![
                "> scoop update *".to_string(),
                "Updating: all packages".to_string(),
                "Updated: all packages".to_string(),
            ]
        );
    }

    #[test]
    fn confirmed_package_action_records_simple_terminal_lines_without_bucket_noise() {
        let mut app = App::new(packages());
        app.move_down();
        app.request_update_selected();

        app.finish_pending_action(
            CommandResult {
                command: "scoop update git".to_string(),
                output: [
                    "* 429584fc930be0ad qq-nt: Update to version 9.9.31.260528",
                    "extras 36m6 days ago",
                    "* 33m2dcbc841b37b stirlingpdf@2.11.0: Update `suggest`",
                    "Updated git",
                ]
                .join("\n"),
                packages: Vec::new(),
            },
            CommandResult {
                command: "scoop list".to_string(),
                output: "Listed installed packages".to_string(),
                packages: packages()
                    .into_iter()
                    .filter(|package| package.installed)
                    .collect(),
            },
        );

        assert_eq!(
            app.output,
            vec![
                "> scoop update git".to_string(),
                "Updating: git".to_string(),
                "Updated: git".to_string(),
            ]
        );
    }

    #[test]
    fn refreshes_installed_view_after_confirmed_package_action() {
        let backend = RecordingBackend::new();
        let mut app = App::new(packages());
        app.show_installed_view(&backend);
        app.move_down();
        app.request_update_selected();

        app.confirm_pending_action(&backend);

        assert_eq!(backend.installed_calls.get(), 2);
        assert_eq!(app.package_view, PackageView::Installed);
        assert_eq!(
            app.selected_package().map(|package| package.name.as_str()),
            Some("git")
        );
        assert!(app.output.iter().all(|line| line != "> scoop list"));
        assert!(app.output.iter().any(|line| line == "> scoop update git"));
    }

    #[test]
    fn confirmed_package_action_refreshes_installed_state_in_search_view() {
        let backend = RecordingBackend::new();
        let mut app = App::new(packages());
        app.show_installed_view(&backend);
        app.set_search_query("gitui");
        app.load_search_results_at(&backend, Instant::now());
        app.request_install_selected();

        app.confirm_pending_action(&backend);

        assert_eq!(backend.installed_calls.get(), 2);
        assert_eq!(app.package_view, PackageView::Search);
        assert!(app
            .output
            .iter()
            .any(|line| line == "> scoop install gitui"));
    }

    #[test]
    fn cancels_pending_uninstall_without_recording_output() {
        let mut app = App::new(packages());
        app.request_uninstall_selected();

        app.cancel_pending_action();

        assert_eq!(app.pending_action, None);
        assert!(app.output.is_empty());
    }

    #[test]
    fn opens_setup_prompt_when_scoop_is_missing() {
        let backend = RecordingBackend::without_scoop();
        let mut app = App::new(packages());

        app.check_scoop_available(&backend);

        assert_eq!(app.setup_prompt, Some(SetupPrompt::InstallScoop));
        assert_eq!(
            app.setup_prompt
                .as_ref()
                .map(|prompt| prompt.command_preview()),
            Some(SCOOP_INSTALL_COMMAND)
        );
    }

    #[test]
    fn confirming_setup_prompt_records_scoop_install_command() {
        let backend = RecordingBackend::without_scoop();
        let mut app = App::new(packages());
        app.check_scoop_available(&backend);

        let result = app
            .confirm_setup_prompt(&backend)
            .expect("setup command result");

        assert_eq!(result.command, SCOOP_INSTALL_COMMAND);
        assert_eq!(app.setup_prompt, None);
        assert!(app.output.iter().any(
            |line| line == "> Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression"
        ));
        assert!(app.output.iter().any(|line| line == "Installed Scoop"));
    }

    #[test]
    fn beginning_setup_prompt_keeps_activity_active_until_finished() {
        let mut app = App::new(packages());
        app.setup_prompt = Some(SetupPrompt::InstallScoop);

        app.begin_setup_prompt();

        assert_eq!(app.setup_prompt, None);
        assert_eq!(app.activity, Activity::InstallingScoop);

        app.finish_setup_prompt(CommandResult {
            command: SCOOP_INSTALL_COMMAND.to_string(),
            output: "Installed Scoop".to_string(),
            packages: Vec::new(),
        });

        assert_eq!(app.activity, Activity::Idle);
        assert!(app.output.iter().any(|line| line == "Installed Scoop"));
    }

    #[test]
    fn toggles_help_modal_visibility() {
        let mut app = App::new(packages());

        app.toggle_help();
        assert!(app.show_help);

        app.toggle_help();
        assert!(!app.show_help);
    }

    #[test]
    fn closes_help_modal_without_changing_pending_action() {
        let mut app = App::new(packages());
        app.request_install_selected();
        app.open_help();

        app.close_help();

        assert!(!app.show_help);
        assert_eq!(
            app.pending_action
                .as_ref()
                .map(|action| action.command_preview()),
            Some("scoop install 7zip".to_string())
        );
    }

    #[test]
    fn reuses_recent_search_results_without_recording_duplicate_output() {
        let now = Instant::now();
        let backend = RecordingBackend::new();
        let mut app = App::with_cache_ttl(packages(), Duration::from_secs(30));
        app.set_search_query("git");

        app.load_search_results_at(&backend, now);
        let output_after_first_search = app.output.clone();
        app.show_installed_view(&backend);
        app.load_search_results_at(&backend, now + Duration::from_secs(5));

        assert_eq!(backend.search_calls.get(), 1);
        assert_eq!(app.package_view, PackageView::Search);
        assert!(app.output.iter().all(|line| line != "> scoop search git"));
        assert!(app.output.starts_with(&output_after_first_search));
        assert_eq!(
            app.filtered_packages()
                .into_iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            vec!["git", "gitui"]
        );
    }

    #[test]
    fn refreshes_search_results_after_cache_ttl_expires() {
        let now = Instant::now();
        let backend = RecordingBackend::new();
        let mut app = App::with_cache_ttl(packages(), Duration::from_secs(10));
        app.set_search_query("git");

        app.load_search_results_at(&backend, now);
        app.show_installed_view(&backend);
        app.load_search_results_at(&backend, now + Duration::from_secs(11));

        assert_eq!(backend.search_calls.get(), 2);
        assert!(app.output.iter().all(|line| line != "> scoop search git"));
    }

    #[test]
    fn search_results_keep_installed_state_without_recording_result_rows() {
        let now = Instant::now();
        let backend = RecordingBackend::new();
        let mut app = App::new(packages());
        app.show_installed_view(&backend);
        let output_after_installed_view = app.output.clone();
        app.set_search_query("7zip");

        app.load_search_results_at(&backend, now);

        assert_eq!(app.package_view, PackageView::Search);
        assert_eq!(app.output, output_after_installed_view);
        assert_eq!(
            app.selected_package()
                .map(|package| package.state().marker()),
            Some("[I]")
        );
    }

    #[test]
    fn installed_view_updates_packages_without_recording_raw_list_output() {
        let backend = RecordingBackend::new();
        let mut app = App::new(Vec::new());

        app.show_installed_view(&backend);

        assert_eq!(app.package_view, PackageView::Installed);
        assert_eq!(app.filtered_count(), 2);
        assert!(app.output.is_empty());
    }

    #[test]
    fn beginning_uncached_search_keeps_activity_active_until_finished() {
        let now = Instant::now();
        let mut app = App::new(packages());
        app.set_search_query("git");

        let query = app
            .begin_search_results_at(now)
            .expect("uncached search query");

        assert_eq!(query, "git");
        assert_eq!(app.activity, Activity::Searching);

        app.finish_search_results_at(
            CommandResult {
                command: "scoop search git".to_string(),
                output: "Found packages for git".to_string(),
                packages: packages()
                    .into_iter()
                    .filter(|package| package.name.contains("git"))
                    .collect(),
            },
            now,
        );

        assert_eq!(app.activity, Activity::Idle);
        assert_eq!(app.filtered_count(), 2);
    }

    #[test]
    fn beginning_installed_view_keeps_activity_active_until_finished() {
        let mut app = App::new(Vec::new());

        app.begin_installed_view();

        assert_eq!(app.package_view, PackageView::Installed);
        assert_eq!(app.activity, Activity::LoadingInstalled);

        app.finish_installed_view(CommandResult {
            command: "scoop list".to_string(),
            output: "Listed installed packages".to_string(),
            packages: packages()
                .into_iter()
                .filter(|package| package.installed)
                .collect(),
        });

        assert_eq!(app.activity, Activity::Idle);
        assert_eq!(app.filtered_count(), 2);
    }

    #[test]
    fn l_toggle_switches_from_search_view_to_installed_view() {
        let now = Instant::now();
        let backend = RecordingBackend::new();
        let mut app = App::new(packages());
        app.set_search_query("git");
        app.load_search_results_at(&backend, now);

        app.toggle_installed_view_at(&backend, now + Duration::from_secs(1));

        assert_eq!(app.package_view, PackageView::Installed);
        assert!(app.output.is_empty());
        assert_eq!(
            app.filtered_packages()
                .into_iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            vec!["git"]
        );
    }

    #[test]
    fn l_toggle_switches_from_installed_view_back_to_cached_search_view() {
        let now = Instant::now();
        let backend = RecordingBackend::new();
        let mut app = App::with_cache_ttl(packages(), Duration::from_secs(30));
        app.set_search_query("git");
        app.load_search_results_at(&backend, now);
        app.toggle_installed_view_at(&backend, now + Duration::from_secs(1));

        app.toggle_installed_view_at(&backend, now + Duration::from_secs(2));

        assert_eq!(backend.search_calls.get(), 1);
        assert_eq!(app.package_view, PackageView::Search);
        assert_eq!(
            app.filtered_packages()
                .into_iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            vec!["git", "gitui"]
        );
    }

    #[test]
    fn spinner_advances_on_tick() {
        let mut app = App::new(packages());

        let first = app.spinner_frame();
        app.tick();

        assert_ne!(app.spinner_frame(), first);
    }

    #[test]
    fn page_and_edge_navigation_move_selected_package() {
        let mut app = App::new(
            (0..20)
                .map(|index| Package {
                    source: "main".to_string(),
                    name: format!("pkg-{index:02}"),
                    version: "1.0.0".to_string(),
                    latest_version: None,
                    description: String::new(),
                    installed: false,
                    homepage: None,
                })
                .collect(),
        );

        app.move_page_down(8);
        assert_eq!(app.selected_index(), 8);

        app.move_last();
        assert_eq!(app.selected_index(), 19);

        app.move_page_up(8);
        assert_eq!(app.selected_index(), 11);

        app.move_first();
        assert_eq!(app.selected_index(), 0);
    }

    #[test]
    fn page_and_edge_navigation_scroll_output_history() {
        let mut app = App::new(packages());
        app.output = (0..20).map(|index| format!("line-{index:02}")).collect();

        app.scroll_output_page_up(8);
        assert_eq!(app.output_scroll, 8);

        app.scroll_output_to_top();
        assert_eq!(app.output_scroll, 19);

        app.scroll_output_page_down(8);
        assert_eq!(app.output_scroll, 11);

        app.scroll_output_to_bottom();
        assert_eq!(app.output_scroll, 0);
    }

    #[test]
    fn help_scroll_tracks_help_modal_offset() {
        let mut app = App::new(packages());
        app.open_help();

        app.scroll_help_page_down(8);
        assert_eq!(app.help_scroll, 8);

        app.scroll_help_up();
        assert_eq!(app.help_scroll, 7);

        app.scroll_help_page_up(8);
        assert_eq!(app.help_scroll, 0);
    }

    #[test]
    fn activity_label_includes_spinner_when_active() {
        let mut app = App::new(packages());
        app.activity = Activity::RunningCommand {
            action: PackageAction::Install,
            package_name: "gitui".to_string(),
        };

        assert_eq!(app.activity_label(), "Installing gitui |");
        app.tick();
        assert_eq!(app.activity_label(), "Installing gitui /");
    }

    #[test]
    fn beginning_pending_action_keeps_activity_active_until_finished() {
        let mut app = App::new(packages());
        app.move_down();
        app.request_update_selected();

        let pending = app.begin_pending_action().expect("pending action");

        assert_eq!(pending.command_preview(), "scoop update git");
        assert_eq!(
            app.activity,
            Activity::RunningCommand {
                action: PackageAction::Update,
                package_name: "git".to_string()
            }
        );

        app.finish_pending_action(
            CommandResult {
                command: "scoop update git".to_string(),
                output: "Updated git".to_string(),
                packages: Vec::new(),
            },
            CommandResult {
                command: "scoop list".to_string(),
                output: "Listed installed packages".to_string(),
                packages: packages()
                    .into_iter()
                    .filter(|package| package.installed)
                    .collect(),
            },
        );

        assert_eq!(app.activity, Activity::Idle);
        assert!(app.output.iter().any(|line| line == "Updated: git"));
    }

    fn packages() -> Vec<Package> {
        vec![
            Package {
                source: "main".to_string(),
                name: "7zip".to_string(),
                version: "24.09".to_string(),
                latest_version: None,
                description: "File archiver".to_string(),
                installed: true,
                homepage: Some("https://www.7-zip.org".to_string()),
            },
            Package {
                source: "main".to_string(),
                name: "git".to_string(),
                version: "2.45.2".to_string(),
                latest_version: None,
                description: "Distributed version control system".to_string(),
                installed: true,
                homepage: Some("https://git-scm.com".to_string()),
            },
            Package {
                source: "extras".to_string(),
                name: "gitui".to_string(),
                version: "0.26.3".to_string(),
                latest_version: None,
                description: "Terminal UI for git".to_string(),
                installed: false,
                homepage: Some("https://github.com/extrawurst/gitui".to_string()),
            },
        ]
    }
}
