use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallState {
    Installed,
    Available,
    Held,
}

impl InstallState {
    pub fn marker(&self) -> &'static str {
        match self {
            Self::Installed => "[I]",
            Self::Available => "[ ]",
            Self::Held => "[H]",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Installed => "Installed",
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
    pub description: String,
    pub installed: bool,
    pub homepage: Option<String>,
}

impl Package {
    pub fn state(&self) -> InstallState {
        if self.installed {
            InstallState::Installed
        } else {
            InstallState::Available
        }
    }
}
