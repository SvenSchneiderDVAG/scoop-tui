use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const LOG_FILE_NAME: &str = "scoop-tui.log";

/// Command log that is truncated on every application start.
#[derive(Debug)]
pub struct AppLog {
    path: PathBuf,
    file: Mutex<File>,
}

impl AppLog {
    pub fn create(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::create(&path)?;
        Ok(Self {
            path,
            file: Mutex::new(file),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write_command(&self, command: &str, status: &str, output: &str) {
        let Ok(mut file) = self.file.lock() else {
            return;
        };
        let _ = writeln!(file, "> {command}\n[{status}]\n{}\n", output.trim_end());
        let _ = file.flush();
    }
}

pub fn default_log_path() -> PathBuf {
    log_path_in(std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
}

fn log_path_in(local_app_data: Option<PathBuf>) -> PathBuf {
    local_app_data
        .unwrap_or_else(std::env::temp_dir)
        .join("scoop-tui")
        .join(LOG_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("scoop-tui-log-test-{name}-{}", std::process::id()))
            .join(LOG_FILE_NAME)
    }

    #[test]
    fn create_truncates_previous_log() {
        let path = temp_log_path("truncate");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "old run").unwrap();

        let log = AppLog::create(&path).unwrap();

        assert_eq!(log.path(), path.as_path());
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn writes_full_command_output_with_status() {
        let path = temp_log_path("write");
        let log = AppLog::create(&path).unwrap();

        log.write_command("scoop update *", "exit code: 1", "line one\nERROR denied\n");
        log.write_command("scoop list", "exit code: 0", "7zip 26.03 main");

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("> scoop update *\n[exit code: 1]\nline one\nERROR denied\n"));
        assert!(content.contains("> scoop list\n[exit code: 0]\n7zip 26.03 main\n"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn log_path_uses_local_app_data_with_temp_fallback() {
        assert_eq!(
            log_path_in(Some(PathBuf::from(r"C:\Users\Test\AppData\Local"))),
            PathBuf::from(r"C:\Users\Test\AppData\Local")
                .join("scoop-tui")
                .join("scoop-tui.log")
        );
        assert_eq!(
            log_path_in(None),
            std::env::temp_dir().join("scoop-tui").join("scoop-tui.log")
        );
    }
}
