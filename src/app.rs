use std::time::{Duration, Instant};

use crate::backend::{CommandResult, ScoopBackend, SCOOP_INSTALL_COMMAND};
use crate::cache::{PackageCache, PackageCacheKey, PackageView};
use crate::models::Package;

const DEFAULT_PACKAGE_CACHE_TTL: Duration = Duration::from_secs(90);
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
    pub activity: Activity,
    spinner_index: usize,
    package_cache: PackageCache,
}

impl App {
    pub fn new(packages: Vec<Package>) -> Self {
        Self {
            packages,
            search_query: String::new(),
            selected: 0,
            output: vec![
                "Mock Scoop backend active. Real commands are Windows-only for now.".to_string(),
            ],
            input_mode: InputMode::Browse,
            package_view: PackageView::Installed,
            pending_action: None,
            setup_prompt: None,
            show_help: false,
            activity: Activity::Idle,
            spinner_index: 0,
            package_cache: PackageCache::new(DEFAULT_PACKAGE_CACHE_TTL),
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

    pub fn move_down(&mut self) {
        let len = self.filtered_indices().len();
        if len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
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

    pub fn check_scoop_available<B: ScoopBackend>(&mut self, backend: &B) {
        if !backend.is_scoop_available() {
            self.setup_prompt = Some(SetupPrompt::InstallScoop);
        }
    }

    pub fn cancel_setup_prompt(&mut self) {
        self.setup_prompt = None;
    }

    pub fn confirm_setup_prompt<B: ScoopBackend>(&mut self, backend: &B) -> Option<CommandResult> {
        self.setup_prompt.take()?;
        self.activity = Activity::InstallingScoop;
        let result = backend.install_scoop();
        self.record_command(&result);
        self.activity = Activity::Idle;
        Some(result)
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_action = None;
    }

    pub fn open_help(&mut self) {
        self.show_help = true;
    }

    pub fn close_help(&mut self) {
        self.show_help = false;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn confirm_pending_action<B: ScoopBackend>(
        &mut self,
        backend: &B,
    ) -> Option<CommandResult> {
        let pending = self.pending_action.take()?;
        self.activity = Activity::RunningCommand {
            action: pending.action.clone(),
            package_name: pending.package_name.clone(),
        };
        let result = match pending.action {
            PackageAction::Install => backend.install(&pending.package_name),
            PackageAction::Uninstall => backend.uninstall(&pending.package_name),
            PackageAction::Update => backend.update(&pending.package_name),
        };
        self.record_command(&result);
        self.activity = Activity::Idle;
        Some(result)
    }

    pub fn load_search_results<B: ScoopBackend>(&mut self, backend: &B) {
        self.load_search_results_at(backend, Instant::now());
    }

    pub fn load_search_results_at<B: ScoopBackend>(&mut self, backend: &B, now: Instant) {
        let key = PackageCacheKey::new(PackageView::Search, &self.search_query);
        self.package_view = PackageView::Search;

        if let Some(packages) = self.package_cache.get(&key, now) {
            self.packages = packages;
            self.selected = 0;
            return;
        }

        self.activity = Activity::Searching;
        let result = backend.search(&self.search_query);
        self.packages = result.packages.clone();
        self.package_cache.insert(key, result.packages.clone(), now);
        self.selected = 0;
        self.record_command(&result);
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
        self.package_view = PackageView::Installed;
        self.packages = backend
            .packages()
            .into_iter()
            .filter(|package| package.installed)
            .collect();
        self.selected = 0;
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
        self.output
            .extend(result.output.lines().map(str::to_string));
        if self.output.len() > 12 {
            let overflow = self.output.len() - 12;
            self.output.drain(0..overflow);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::CommandResult;
    use std::cell::Cell;

    struct RecordingBackend {
        search_calls: Cell<usize>,
        scoop_available: bool,
    }

    impl RecordingBackend {
        fn new() -> Self {
            Self {
                search_calls: Cell::new(0),
                scoop_available: true,
            }
        }

        fn without_scoop() -> Self {
            Self {
                search_calls: Cell::new(0),
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
    fn opens_confirmation_for_selected_install_without_running_command() {
        let mut app = App::new(packages());
        app.set_search_query("gitui");

        let pending = app
            .request_install_selected()
            .expect("pending install action");

        assert_eq!(pending.command_preview(), "scoop install gitui");
        assert_eq!(app.output.len(), 1);
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
        assert!(app.output.iter().any(|line| line == "Updated git"));
    }

    #[test]
    fn cancels_pending_uninstall_without_recording_output() {
        let mut app = App::new(packages());
        app.request_uninstall_selected();

        app.cancel_pending_action();

        assert_eq!(app.pending_action, None);
        assert_eq!(app.output.len(), 1);
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
        assert!(app.output.iter().any(|line| line == "> Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser; Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression"));
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
        assert_eq!(app.output, output_after_first_search);
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
        assert_eq!(
            app.output
                .iter()
                .filter(|line| line.as_str() == "> scoop search git")
                .count(),
            2
        );
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

    fn packages() -> Vec<Package> {
        vec![
            Package {
                source: "main".to_string(),
                name: "7zip".to_string(),
                version: "24.09".to_string(),
                description: "File archiver".to_string(),
                installed: true,
                homepage: Some("https://www.7-zip.org".to_string()),
            },
            Package {
                source: "main".to_string(),
                name: "git".to_string(),
                version: "2.45.2".to_string(),
                description: "Distributed version control system".to_string(),
                installed: true,
                homepage: Some("https://git-scm.com".to_string()),
            },
            Package {
                source: "extras".to_string(),
                name: "gitui".to_string(),
                version: "0.26.3".to_string(),
                description: "Terminal UI for git".to_string(),
                installed: false,
                homepage: Some("https://github.com/extrawurst/gitui".to_string()),
            },
        ]
    }
}
