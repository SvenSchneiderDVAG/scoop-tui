use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::models::Package;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PackageView {
    Installed,
    Search,
}

impl PackageView {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Installed => "Installed",
            Self::Search => "Search",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageCacheKey {
    view: PackageView,
    query: String,
}

impl PackageCacheKey {
    pub fn new(view: PackageView, query: &str) -> Self {
        Self {
            view,
            query: query.trim().to_lowercase(),
        }
    }
}

#[derive(Clone, Debug)]
struct PackageCacheEntry {
    packages: Vec<Package>,
    inserted_at: Instant,
}

#[derive(Debug)]
pub struct PackageCache {
    ttl: Duration,
    entries: HashMap<PackageCacheKey, PackageCacheEntry>,
}

impl PackageCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: HashMap::new(),
        }
    }

    pub fn get(&self, key: &PackageCacheKey, now: Instant) -> Option<Vec<Package>> {
        let entry = self.entries.get(key)?;
        (now.duration_since(entry.inserted_at) < self.ttl).then(|| entry.packages.clone())
    }

    pub fn insert(&mut self, key: PackageCacheKey, packages: Vec<Package>, now: Instant) {
        self.entries.insert(
            key,
            PackageCacheEntry {
                packages,
                inserted_at: now,
            },
        );
    }
}
