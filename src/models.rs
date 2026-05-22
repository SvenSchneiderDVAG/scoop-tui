use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallState {
    Installed,
    UpdateAvailable,
    Available,
    Held,
}

impl InstallState {
    pub fn marker(&self) -> &'static str {
        match self {
            Self::Installed => "[I]",
            Self::UpdateAvailable => "[U]",
            Self::Available => "[ ]",
            Self::Held => "[H]",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Installed => "Installed",
            Self::UpdateAvailable => "Update available",
            Self::Available => "Available",
            Self::Held => "Held",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub source: String,
    pub name: String,
    pub version: String,
    pub latest_version: Option<String>,
    pub description: String,
    pub installed: bool,
    pub homepage: Option<String>,
}

impl Package {
    pub fn state(&self) -> InstallState {
        if self.update_available() {
            InstallState::UpdateAvailable
        } else if self.installed {
            InstallState::Installed
        } else {
            InstallState::Available
        }
    }

    pub fn update_available(&self) -> bool {
        self.latest_version
            .as_ref()
            .is_some_and(|latest_version| latest_version != &self.version)
    }

    pub fn latest_display(&self) -> &str {
        self.latest_version.as_deref().unwrap_or("")
    }
}
