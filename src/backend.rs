use crate::discovery::ScoopEnvironment;
use crate::models::Package;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const SCOOP_INSTALL_COMMAND: &str =
    "Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression";
const SCOOP_NOT_FOUND_MESSAGE: &str =
    "Scoop executable was not found. Install Scoop or make sure its shims directory is on PATH.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub command: String,
    pub output: String,
    pub packages: Vec<Package>,
}

pub trait ScoopBackend {
    fn is_scoop_available(&self) -> bool;
    fn environment(&self) -> ScoopEnvironment;
    fn packages(&self) -> Vec<Package>;
    fn installed(&self) -> CommandResult;
    fn search(&self, query: &str) -> CommandResult;
    fn install_scoop(&self) -> CommandResult;
    fn install(&self, name: &str) -> CommandResult;
    fn uninstall(&self, name: &str) -> CommandResult;
    fn update(&self, name: &str) -> CommandResult;
}

#[derive(Debug, Clone)]
pub struct ProductionBackend {
    environment: ScoopEnvironment,
}

impl Default for ProductionBackend {
    fn default() -> Self {
        Self {
            environment: ScoopEnvironment::discover(),
        }
    }
}

impl ProductionBackend {
    pub fn new(environment: ScoopEnvironment) -> Self {
        Self { environment }
    }

    fn scoop_executable(&self) -> Option<PathBuf> {
        find_scoop_executable(&self.environment)
    }

    fn run_scoop_command(&self, args: &[&str]) -> CommandResult {
        let command = format!("scoop {}", args.join(" "));
        let Some(executable) = self.scoop_executable() else {
            return CommandResult {
                command,
                output: SCOOP_NOT_FOUND_MESSAGE.to_string(),
                packages: Vec::new(),
            };
        };
        let mut process = scoop_command(&executable, args);

        match process.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = [stdout.trim_end(), stderr.trim_end()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");

                CommandResult {
                    command,
                    output: if combined.is_empty() {
                        format!("Command exited with status {}", output.status)
                    } else {
                        combined
                    },
                    packages: Vec::new(),
                }
            }
            Err(error) => CommandResult {
                command,
                output: format!("Failed to execute scoop: {error}"),
                packages: Vec::new(),
            },
        }
    }
}

impl ScoopBackend for ProductionBackend {
    fn is_scoop_available(&self) -> bool {
        self.scoop_executable().is_some()
    }

    fn environment(&self) -> ScoopEnvironment {
        self.environment.clone()
    }

    fn packages(&self) -> Vec<Package> {
        self.installed().packages
    }

    fn installed(&self) -> CommandResult {
        let list = self.run_scoop_command(&["list"]);
        let status = self.run_scoop_command(&["status"]);
        let latest_versions = parse_scoop_status_latest_versions(&status.output);
        let packages =
            merge_latest_versions(parse_scoop_list_packages(&list.output), latest_versions);
        let output = [list.output.trim_end(), status.output.trim_end()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        CommandResult {
            command: "scoop list && scoop status".to_string(),
            output,
            packages,
        }
    }

    fn search(&self, query: &str) -> CommandResult {
        let mut result = if query.trim().is_empty() {
            self.run_scoop_command(&["search"])
        } else {
            self.run_scoop_command(&["search", query.trim()])
        };
        result.packages = parse_scoop_search_packages(&result.output);
        result
    }

    fn install_scoop(&self) -> CommandResult {
        let command = SCOOP_INSTALL_COMMAND.to_string();
        let script = powershell_script(&command);
        let mut process = Command::new("powershell");
        process.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ]);
        hide_child_window(&mut process);
        let output = process.output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = [stdout.trim_end(), stderr.trim_end()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");

                CommandResult {
                    command,
                    output: if combined.is_empty() {
                        format!("Command exited with status {}", output.status)
                    } else {
                        combined
                    },
                    packages: Vec::new(),
                }
            }
            Err(error) => CommandResult {
                command,
                output: format!("Failed to execute PowerShell: {error}"),
                packages: Vec::new(),
            },
        }
    }

    fn install(&self, name: &str) -> CommandResult {
        self.run_scoop_command(&["install", name])
    }

    fn uninstall(&self, name: &str) -> CommandResult {
        self.run_scoop_command(&["uninstall", name])
    }

    fn update(&self, name: &str) -> CommandResult {
        self.run_scoop_command(&["update", name])
    }
}

fn find_scoop_executable(environment: &ScoopEnvironment) -> Option<PathBuf> {
    scoop_executable_candidates(environment)
        .into_iter()
        .find(|candidate| candidate.is_file())
}

fn scoop_command(executable: &Path, args: &[&str]) -> Command {
    match executable
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some(extension) if extension.eq_ignore_ascii_case("cmd") => {
            cmd_command(&cmd_script(executable, args))
        }
        Some(extension) if extension.eq_ignore_ascii_case("ps1") => {
            powershell_command(&executable.to_string_lossy(), args)
        }
        _ => {
            let mut command = Command::new(executable);
            command.args(args);
            hide_child_window(&mut command);
            command
        }
    }
}

fn powershell_script(script: &str) -> String {
    format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; {script}"
    )
}

fn cmd_command(script: &str) -> Command {
    let mut command = Command::new("cmd.exe");
    command.args(["/D", "/C"]);
    push_cmd_script_arg(&mut command, script);
    hide_child_window(&mut command);
    command
}

#[cfg(target_os = "windows")]
fn push_cmd_script_arg(command: &mut Command, script: &str) {
    use std::os::windows::process::CommandExt;

    command.raw_arg(script);
}

#[cfg(not(target_os = "windows"))]
fn push_cmd_script_arg(command: &mut Command, script: &str) {
    command.arg(script);
}

fn cmd_script(executable: &Path, args: &[&str]) -> String {
    let quoted_args = args
        .iter()
        .map(|argument| quote_cmd_argument(argument))
        .collect::<Vec<_>>()
        .join(" ");
    let executable = executable.to_string_lossy();
    let executable_command = if executable.contains('\\')
        || executable.contains('/')
        || executable.to_ascii_lowercase().ends_with(".cmd")
    {
        format!("call {}", quote_cmd_argument(&executable))
    } else {
        executable.to_string()
    };

    if quoted_args.is_empty() {
        format!("chcp 65001 >NUL && {executable_command}")
    } else {
        format!("chcp 65001 >NUL && {executable_command} {quoted_args}")
    }
}

fn quote_cmd_argument(argument: &str) -> String {
    format!("\"{}\"", argument.replace('"', "\"\""))
}

fn powershell_command(executable: &str, args: &[&str]) -> Command {
    let mut command = Command::new("powershell");
    let quoted_args = args
        .iter()
        .map(|argument| quote_powershell_argument(argument))
        .collect::<Vec<_>>()
        .join(" ");
    let script = if quoted_args.is_empty() {
        format!("& {}", quote_powershell_argument(executable))
    } else {
        format!("& {} {quoted_args}", quote_powershell_argument(executable))
    };
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(powershell_script(&script));
    hide_child_window(&mut command);
    command
}

#[cfg(target_os = "windows")]
fn hide_child_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_child_window(_command: &mut Command) {}

fn quote_powershell_argument(argument: &str) -> String {
    format!("'{}'", argument.replace('\'', "''"))
}

fn scoop_executable_candidates(environment: &ScoopEnvironment) -> Vec<PathBuf> {
    let mut candidates = vec![
        environment.user_root.join("shims").join("scoop.ps1"),
        environment.global_root.join("shims").join("scoop.ps1"),
        environment.user_root.join("shims").join("scoop.cmd"),
        environment.global_root.join("shims").join("scoop.cmd"),
    ];

    if let Some(paths) = env::var_os("PATH") {
        for path in env::split_paths(&paths) {
            candidates.push(path.join("scoop.ps1"));
            candidates.push(path.join("scoop.cmd"));
            candidates.push(path.join("scoop.exe"));
        }
    }
    candidates
}

fn parse_scoop_list_packages(output: &str) -> Vec<Package> {
    parse_scoop_package_rows(output, true, "Installed package")
}

fn parse_scoop_search_packages(output: &str) -> Vec<Package> {
    parse_scoop_package_rows(output, false, "")
}

fn parse_scoop_package_rows(output: &str, installed: bool, description: &str) -> Vec<Package> {
    output
        .lines()
        .filter_map(|line| parse_scoop_package_row(line, installed, description))
        .collect()
}

fn parse_scoop_package_row(line: &str, installed: bool, description: &str) -> Option<Package> {
    let line = line.trim();
    if line.is_empty()
        || line.starts_with("Installed apps")
        || line.starts_with("Results from")
        || line.starts_with("Name ")
        || line.starts_with("----")
        || line.starts_with("Der Befehl")
        || line.contains("konnte nicht gefunden werden")
    {
        return None;
    }

    let columns = line.split_whitespace().collect::<Vec<_>>();
    if columns.len() < 3 {
        return None;
    }

    let name = columns.first()?.to_string();
    let version = columns.get(1)?.to_string();
    if !version.chars().any(|character| character.is_ascii_digit()) {
        return None;
    }
    let source = columns.get(2)?.to_string();

    Some(Package {
        source,
        name,
        version,
        latest_version: None,
        description: description.to_string(),
        installed,
        homepage: None,
    })
}

fn parse_scoop_status_latest_versions(output: &str) -> HashMap<String, String> {
    output.lines().filter_map(parse_scoop_status_line).collect()
}

fn parse_scoop_status_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty()
        || line.starts_with("Scoop ")
        || line.starts_with("Name ")
        || line.starts_with("----")
    {
        return None;
    }

    let columns = line.split_whitespace().collect::<Vec<_>>();
    if columns.len() < 3 {
        return None;
    }

    let name = columns.first()?.to_string();
    let installed_version = columns.get(1)?;
    let latest_version = columns.get(2)?;
    if !installed_version
        .chars()
        .any(|character| character.is_ascii_digit())
        || !latest_version
            .chars()
            .any(|character| character.is_ascii_digit())
    {
        return None;
    }

    Some((name, latest_version.to_string()))
}

fn merge_latest_versions(
    packages: Vec<Package>,
    latest_versions: HashMap<String, String>,
) -> Vec<Package> {
    packages
        .into_iter()
        .map(|mut package| {
            package.latest_version = latest_versions.get(&package.name).cloned();
            package
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
#[cfg(any(test, not(target_os = "windows")))]
pub struct MockBackend;

#[cfg(any(test, not(target_os = "windows")))]
impl ScoopBackend for MockBackend {
    fn is_scoop_available(&self) -> bool {
        true
    }

    fn environment(&self) -> ScoopEnvironment {
        ScoopEnvironment::discover()
    }

    fn packages(&self) -> Vec<Package> {
        vec![
            Package {
                source: "main".to_string(),
                name: "7zip".to_string(),
                version: "24.09".to_string(),
                latest_version: None,
                description: "File archiver with high compression ratio".to_string(),
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
                description: "Blazing fast terminal UI for git".to_string(),
                installed: false,
                homepage: Some("https://github.com/extrawurst/gitui".to_string()),
            },
            Package {
                source: "main".to_string(),
                name: "ripgrep".to_string(),
                version: "14.1.0".to_string(),
                latest_version: None,
                description: "Line-oriented search tool".to_string(),
                installed: true,
                homepage: Some("https://github.com/BurntSushi/ripgrep".to_string()),
            },
            Package {
                source: "extras".to_string(),
                name: "wezterm".to_string(),
                version: "20240203-110809".to_string(),
                latest_version: None,
                description: "GPU-accelerated terminal emulator".to_string(),
                installed: false,
                homepage: Some("https://wezfurlong.org/wezterm/".to_string()),
            },
            Package {
                source: "nerd-fonts".to_string(),
                name: "JetBrainsMono-NF".to_string(),
                version: "3.2.1".to_string(),
                latest_version: None,
                description: "JetBrains Mono Nerd Font".to_string(),
                installed: false,
                homepage: Some("https://www.nerdfonts.com".to_string()),
            },
            Package {
                source: "main".to_string(),
                name: "fd".to_string(),
                version: "10.1.0".to_string(),
                latest_version: None,
                description: "Simple, fast and user-friendly alternative to find".to_string(),
                installed: true,
                homepage: Some("https://github.com/sharkdp/fd".to_string()),
            },
            Package {
                source: "versions".to_string(),
                name: "python311".to_string(),
                version: "3.11.9".to_string(),
                latest_version: None,
                description: "Python programming language, version 3.11".to_string(),
                installed: false,
                homepage: Some("https://www.python.org".to_string()),
            },
        ]
    }

    fn installed(&self) -> CommandResult {
        CommandResult {
            command: "scoop list".to_string(),
            output: "Mock backend: installed packages served from sample package data.".to_string(),
            packages: self
                .packages()
                .into_iter()
                .filter(|package| package.installed)
                .collect(),
        }
    }

    fn search(&self, query: &str) -> CommandResult {
        let query = query.trim().to_lowercase();
        let packages = self
            .packages()
            .into_iter()
            .filter(|package| {
                query.is_empty()
                    || package.name.to_lowercase().contains(&query)
                    || package.description.to_lowercase().contains(&query)
            })
            .collect();

        CommandResult {
            command: if query.is_empty() {
                "scoop search".to_string()
            } else {
                format!("scoop search {query}")
            },
            output: "Mock backend: search result served from sample package data.".to_string(),
            packages,
        }
    }

    fn install_scoop(&self) -> CommandResult {
        CommandResult {
            command: SCOOP_INSTALL_COMMAND.to_string(),
            output: "Mock backend: would install Scoop for the current Windows user.".to_string(),
            packages: Vec::new(),
        }
    }

    fn install(&self, name: &str) -> CommandResult {
        CommandResult {
            command: format!("scoop install {name}"),
            output: format!(
                "Resolving manifest for {name}\nDownloading package archive\nRunning installer\nDone"
            ),
            packages: Vec::new(),
        }
    }

    fn uninstall(&self, name: &str) -> CommandResult {
        CommandResult {
            command: format!("scoop uninstall {name}"),
            output: format!("Removing shim links for {name}\nDeleting app directory\nDone"),
            packages: Vec::new(),
        }
    }

    fn update(&self, name: &str) -> CommandResult {
        CommandResult {
            command: format!("scoop update {name}"),
            output: format!("Mock backend: would update {name} on Windows."),
            packages: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_scoop_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("scoop-tui-{name}-{nanos}"))
    }

    #[test]
    fn finds_scoop_cmd_in_user_shims() {
        let user_root = temp_scoop_root("user");
        let shim_dir = user_root.join("shims");
        fs::create_dir_all(&shim_dir).expect("create shim dir");
        let shim = shim_dir.join("scoop.cmd");
        fs::write(&shim, "@echo off").expect("write scoop shim");

        let environment = ScoopEnvironment {
            user_root: user_root.clone(),
            global_root: temp_scoop_root("global"),
        };

        assert_eq!(find_scoop_executable(&environment), Some(shim));

        fs::remove_dir_all(user_root).expect("remove temp user root");
    }

    #[test]
    fn prefers_scoop_ps1_to_avoid_cmd_shim_windows() {
        let user_root = temp_scoop_root("user");
        let shim_dir = user_root.join("shims");
        fs::create_dir_all(&shim_dir).expect("create shim dir");
        let cmd_shim = shim_dir.join("scoop.cmd");
        let ps1_shim = shim_dir.join("scoop.ps1");
        fs::write(&cmd_shim, "@echo off").expect("write scoop cmd shim");
        fs::write(&ps1_shim, "Write-Output scoop").expect("write scoop ps1 shim");

        let environment = ScoopEnvironment {
            user_root: user_root.clone(),
            global_root: temp_scoop_root("global"),
        };

        assert_eq!(find_scoop_executable(&environment), Some(ps1_shim));

        fs::remove_dir_all(user_root).expect("remove temp user root");
    }

    #[test]
    fn wraps_cmd_shims_with_cmd_exe() {
        let shim = PathBuf::from(r"C:\Users\diz\scoop\shims\scoop.cmd");

        let command = scoop_command(&shim, &["search"]);

        assert_eq!(command.get_program(), "cmd.exe");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            vec![
                "/D",
                "/C",
                r#"chcp 65001 >NUL && call "C:\Users\diz\scoop\shims\scoop.cmd" "search""#
            ]
        );
    }

    #[test]
    fn does_not_add_relative_scoop_fallback_candidate() {
        let environment = ScoopEnvironment {
            user_root: temp_scoop_root("user"),
            global_root: temp_scoop_root("global"),
        };

        let candidates = scoop_executable_candidates(&environment);

        assert!(!candidates.contains(&PathBuf::from("scoop")));
    }

    #[test]
    fn missing_scoop_does_not_shell_resolve_bare_command() {
        let environment = ScoopEnvironment {
            user_root: temp_scoop_root("missing-user"),
            global_root: temp_scoop_root("missing-global"),
        };
        let backend = ProductionBackend::new(environment);

        let result = backend.search("7zip");

        assert_eq!(result.command, "scoop search 7zip");
        assert_eq!(
            result.output,
            "Scoop executable was not found. Install Scoop or make sure its shims directory is on PATH."
        );
        assert!(result.packages.is_empty());
    }

    #[test]
    fn parses_scoop_list_output_into_installed_packages() {
        let output = r#"
Installed apps:

Name      Version  Source Updated             Info
----      -------  ------ -------             ----
zettlr    4.5.0    extras 2026-05-20 18:31:00
zim       0.76.3   extras 2026-05-20 18:31:00
7zip      24.09    main   2026-05-20 18:31:00
"#;

        let packages = parse_scoop_list_packages(output);

        assert_eq!(
            packages,
            vec![
                Package {
                    source: "extras".to_string(),
                    name: "zettlr".to_string(),
                    version: "4.5.0".to_string(),
                    latest_version: None,
                    description: "Installed package".to_string(),
                    installed: true,
                    homepage: None,
                },
                Package {
                    source: "extras".to_string(),
                    name: "zim".to_string(),
                    version: "0.76.3".to_string(),
                    latest_version: None,
                    description: "Installed package".to_string(),
                    installed: true,
                    homepage: None,
                },
                Package {
                    source: "main".to_string(),
                    name: "7zip".to_string(),
                    version: "24.09".to_string(),
                    latest_version: None,
                    description: "Installed package".to_string(),
                    installed: true,
                    homepage: None,
                },
            ]
        );
    }

    #[test]
    fn parses_scoop_search_output_into_available_packages() {
        let output = r#"
Results from local buckets...

Name      Version  Source Binaries
----      -------  ------ --------
zettlr    4.5.0    extras
zim       0.76.3   extras
"#;

        let packages = parse_scoop_search_packages(output);

        assert_eq!(
            packages,
            vec![
                Package {
                    source: "extras".to_string(),
                    name: "zettlr".to_string(),
                    version: "4.5.0".to_string(),
                    latest_version: None,
                    description: String::new(),
                    installed: false,
                    homepage: None,
                },
                Package {
                    source: "extras".to_string(),
                    name: "zim".to_string(),
                    version: "0.76.3".to_string(),
                    latest_version: None,
                    description: String::new(),
                    installed: false,
                    homepage: None,
                },
            ]
        );
    }

    #[test]
    fn parses_scoop_status_output_into_latest_versions() {
        let output = r#"
Scoop is up to date.

Name   Installed Version Latest Version Missing Dependencies Info
----   ----------------- -------------- -------------------- ----
7zip   24.09             26.01
ffmpeg 7.1.1             8.1
gh     2.72.0            2.92.0
"#;

        let latest_versions = parse_scoop_status_latest_versions(output);

        assert_eq!(
            latest_versions.get("7zip").map(String::as_str),
            Some("26.01")
        );
        assert_eq!(
            latest_versions.get("ffmpeg").map(String::as_str),
            Some("8.1")
        );
        assert_eq!(
            latest_versions.get("gh").map(String::as_str),
            Some("2.92.0")
        );
    }

    #[test]
    fn merges_latest_versions_into_installed_packages() {
        let packages = vec![
            Package {
                source: "main".to_string(),
                name: "7zip".to_string(),
                version: "24.09".to_string(),
                latest_version: None,
                description: "Installed package".to_string(),
                installed: true,
                homepage: None,
            },
            Package {
                source: "main".to_string(),
                name: "dark".to_string(),
                version: "3.14.1".to_string(),
                latest_version: None,
                description: "Installed package".to_string(),
                installed: true,
                homepage: None,
            },
        ];
        let latest_versions = HashMap::from([("7zip".to_string(), "26.01".to_string())]);

        let packages = merge_latest_versions(packages, latest_versions);

        assert_eq!(packages[0].latest_version.as_deref(), Some("26.01"));
        assert_eq!(packages[0].state().marker(), "[U]");
        assert_eq!(packages[1].latest_version, None);
        assert_eq!(packages[1].state().marker(), "[I]");
    }

    #[test]
    fn ignores_localized_command_errors_when_parsing_scoop_list() {
        let output = r#"Der Befehl "\"C:\Users\diz\scoop\shims\scoop.cmd\"" ist entweder
falsch geschrieben oder
konnte nicht gefunden werden."#;

        assert!(parse_scoop_list_packages(output).is_empty());
    }

    #[test]
    fn install_command_does_not_persist_execution_policy() {
        assert_eq!(
            SCOOP_INSTALL_COMMAND,
            "Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression"
        );
        assert!(!SCOOP_INSTALL_COMMAND.contains("Set-ExecutionPolicy"));
    }

    #[test]
    fn powershell_scripts_set_utf8_output_encoding() {
        let script = powershell_script(SCOOP_INSTALL_COMMAND);

        assert!(script.contains("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8"));
        assert!(script.contains("$OutputEncoding = [System.Text.Encoding]::UTF8"));
        assert!(script.ends_with(SCOOP_INSTALL_COMMAND));
    }
}
