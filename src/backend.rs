use crate::discovery::ScoopEnvironment;
use crate::models::Package;
use std::process::Command;

pub const SCOOP_INSTALL_COMMAND: &str =
    "Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser; Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression";

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

    fn run_scoop_command(&self, args: &[&str]) -> CommandResult {
        let command = format!("scoop {}", args.join(" "));
        match Command::new("scoop").args(args).output() {
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
        Command::new("scoop")
            .arg("--version")
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
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &command,
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
