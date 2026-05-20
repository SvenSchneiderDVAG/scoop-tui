use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoopEnvironment {
    pub user_root: PathBuf,
    pub global_root: PathBuf,
}

impl ScoopEnvironment {
    pub fn discover() -> Self {
        Self::from_env_map(std::env::vars().collect())
    }

    pub fn from_env_map(env: HashMap<String, String>) -> Self {
        let user_root = env
            .get("SCOOP")
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                env.get("USERPROFILE")
                    .filter(|value| !value.trim().is_empty())
                    .map(|profile| PathBuf::from(profile).join("scoop"))
            })
            .unwrap_or_else(|| PathBuf::from(r"C:\Users\Default\scoop"));

        let global_root = env
            .get("SCOOP_GLOBAL")
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData\scoop"));

        Self {
            user_root,
            global_root,
        }
    }

    pub fn user_buckets_dir(&self) -> PathBuf {
        self.user_root.join("buckets")
    }

    pub fn global_buckets_dir(&self) -> PathBuf {
        self.global_root.join("buckets")
    }

    pub fn user_apps_dir(&self) -> PathBuf {
        self.user_root.join("apps")
    }

    pub fn global_apps_dir(&self) -> PathBuf {
        self.global_root.join("apps")
    }

    pub fn user_cache_dir(&self) -> PathBuf {
        self.user_root.join("cache")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_scoop_and_scoop_global_when_set() {
        let env = HashMap::from([
            ("SCOOP".to_string(), r"D:\ScoopUser".to_string()),
            ("SCOOP_GLOBAL".to_string(), r"E:\ScoopGlobal".to_string()),
            ("USERPROFILE".to_string(), r"C:\Users\Ada".to_string()),
        ]);

        let discovered = ScoopEnvironment::from_env_map(env);

        assert_eq!(discovered.user_root, PathBuf::from(r"D:\ScoopUser"));
        assert_eq!(discovered.global_root, PathBuf::from(r"E:\ScoopGlobal"));
    }

    #[test]
    fn falls_back_to_userprofile_for_user_root() {
        let env = HashMap::from([("USERPROFILE".to_string(), r"C:\Users\Ada".to_string())]);

        let discovered = ScoopEnvironment::from_env_map(env);

        assert_eq!(
            discovered.user_root,
            PathBuf::from(r"C:\Users\Ada").join("scoop")
        );
        assert_eq!(
            discovered.global_root,
            PathBuf::from(r"C:\ProgramData\scoop")
        );
    }

    #[test]
    fn exposes_common_scoop_paths() {
        let env = HashMap::from([
            ("SCOOP".to_string(), r"D:\ScoopUser".to_string()),
            ("SCOOP_GLOBAL".to_string(), r"E:\ScoopGlobal".to_string()),
        ]);

        let discovered = ScoopEnvironment::from_env_map(env);

        assert_eq!(
            discovered.user_buckets_dir(),
            PathBuf::from(r"D:\ScoopUser").join("buckets")
        );
        assert_eq!(
            discovered.global_buckets_dir(),
            PathBuf::from(r"E:\ScoopGlobal").join("buckets")
        );
        assert_eq!(
            discovered.user_apps_dir(),
            PathBuf::from(r"D:\ScoopUser").join("apps")
        );
        assert_eq!(
            discovered.global_apps_dir(),
            PathBuf::from(r"E:\ScoopGlobal").join("apps")
        );
        assert_eq!(
            discovered.user_cache_dir(),
            PathBuf::from(r"D:\ScoopUser").join("cache")
        );
    }
}
