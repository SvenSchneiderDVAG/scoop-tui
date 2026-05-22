use crate::discovery::ScoopEnvironment;
use crate::models::Package;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const SCOOP_INSTALL_COMMAND: &str =
    "Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression";

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
        let mut process = scoop_process(self.scoop_executable().as_deref(), args);

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
        if self.scoop_executable().is_some() {
            return true;
        }

        let mut process = shell_scoop_lookup_command();

        process
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn environment(&self) -> ScoopEnvironment {
        self.environment.clone()
    }

    fn packages(&self) -> Vec<Package> {
        Vec::new()
    }

    fn search(&self, query: &str) -> CommandResult {
        if query.trim().is_empty() {
            self.run_scoop_command(&["search"])
        } else {
            self.run_scoop_command(&["search", query.trim()])
        }
    }

    fn install_scoop(&self) -> CommandResult {
        let command = SCOOP_INSTALL_COMMAND.to_string();
        let script = powershell_script(&command);
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .output();

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

fn scoop_process(executable: Option<&Path>, args: &[&str]) -> Command {
    match executable {
        Some(executable) => scoop_command(executable, args),
        None => shell_scoop_command(args),
    }
}

fn shell_scoop_command(args: &[&str]) -> Command {
    powershell_command("scoop", args)
}

fn shell_scoop_lookup_command() -> Command {
    let mut command = Command::new("cmd.exe");
    command.args(["/D", "/C", "where scoop >NUL 2>NUL"]);
    command
}

fn scoop_command(executable: &Path, args: &[&str]) -> Command {
    match executable
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some(extension) if extension.eq_ignore_ascii_case("cmd") => {
            powershell_command(&executable.to_string_lossy(), args)
        }
        Some(extension) if extension.eq_ignore_ascii_case("ps1") => {
            powershell_command(&executable.to_string_lossy(), args)
        }
        _ => {
            let mut command = Command::new(executable);
            command.args(args);
            command
        }
    }
}

fn powershell_script(script: &str) -> String {
    format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; {script}"
    )
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
    command
}

fn quote_powershell_argument(argument: &str) -> String {
    format!("'{}'", argument.replace('\'', "''"))
}

fn scoop_executable_candidates(environment: &ScoopEnvironment) -> Vec<PathBuf> {
    let mut candidates = vec![
        environment.user_root.join("shims").join("scoop.cmd"),
        environment.global_root.join("shims").join("scoop.cmd"),
        environment.user_root.join("shims").join("scoop.ps1"),
        environment.global_root.join("shims").join("scoop.ps1"),
    ];

    if let Some(paths) = env::var_os("PATH") {
        for path in env::split_paths(&paths) {
            candidates.push(path.join("scoop.cmd"));
            candidates.push(path.join("scoop.ps1"));
            candidates.push(path.join("scoop.exe"));
        }
    }

    candidates.push(PathBuf::from("scoop"));
    candidates
}

#[derive(Debug, Default)]
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
                description: "File archiver with high compression ratio".to_string(),
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
                description: "Blazing fast terminal UI for git".to_string(),
                installed: false,
                homepage: Some("https://github.com/extrawurst/gitui".to_string()),
            },
            Package {
                source: "main".to_string(),
                name: "ripgrep".to_string(),
                version: "14.1.0".to_string(),
                description: "Line-oriented search tool".to_string(),
                installed: true,
                homepage: Some("https://github.com/BurntSushi/ripgrep".to_string()),
            },
            Package {
                source: "extras".to_string(),
                name: "wezterm".to_string(),
                version: "20240203-110809".to_string(),
                description: "GPU-accelerated terminal emulator".to_string(),
                installed: false,
                homepage: Some("https://wezfurlong.org/wezterm/".to_string()),
            },
            Package {
                source: "nerd-fonts".to_string(),
                name: "JetBrainsMono-NF".to_string(),
                version: "3.2.1".to_string(),
                description: "JetBrains Mono Nerd Font".to_string(),
                installed: false,
                homepage: Some("https://www.nerdfonts.com".to_string()),
            },
            Package {
                source: "main".to_string(),
                name: "fd".to_string(),
                version: "10.1.0".to_string(),
                description: "Simple, fast and user-friendly alternative to find".to_string(),
                installed: true,
                homepage: Some("https://github.com/sharkdp/fd".to_string()),
            },
            Package {
                source: "versions".to_string(),
                name: "python311".to_string(),
                version: "3.11.9".to_string(),
                description: "Python programming language, version 3.11".to_string(),
                installed: false,
                homepage: Some("https://www.python.org".to_string()),
            },
        ]
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
    fn wraps_cmd_shims_with_cmd_exe() {
        let shim = PathBuf::from(r"C:\Users\diz\scoop\shims\scoop.cmd");

        let command = scoop_command(&shim, &["search"]);

        assert_eq!(command.get_program(), "powershell");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            vec![
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; & 'C:\\Users\\diz\\scoop\\shims\\scoop.cmd' 'search'"
            ]
        );
    }

    #[test]
    fn falls_back_to_cmd_shell_resolution_when_no_shim_path_was_found() {
        let command = scoop_process(None, &["--version"]);

        assert_eq!(command.get_program(), "powershell");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            vec![
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; & 'scoop' '--version'"
            ]
        );
    }

    #[test]
    fn availability_uses_shell_lookup_instead_of_version_command() {
        let command = shell_scoop_lookup_command();

        assert_eq!(command.get_program(), "cmd.exe");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            vec!["/D", "/C", "where scoop >NUL 2>NUL"]
        );
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
