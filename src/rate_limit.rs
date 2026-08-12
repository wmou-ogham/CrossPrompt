use std::{sync::Arc, time::{Duration, Instant}};

use dashmap::DashMap;

#[derive(Clone, Default)]
pub struct RateLimits {
    entries: Arc<DashMap<String, Window>>,
}

#[derive(Clone)]
struct Window {
    started: Instant,
    count: u32,
}

impl RateLimits {
    pub fn check(&self, key: impl Into<String>, limit: u32, duration: Duration) -> bool {
        let key = key.into();
        let now = Instant::now();
        let mut entry = self.entries.entry(key).or_insert(Window { started: now, count: 0 });
        if now.duration_since(entry.started) >= duration {
            entry.started = now;
            entry.count = 0;
        }
        if entry.count >= limit {
            return false;
        }
        entry.count += 1;
        true
    }

    pub fn clear(&self, key: &str) {
        self.entries.remove(key);
    }

    pub fn prune(&self) {
        let now = Instant::now();
        self.entries.retain(|_, window| now.duration_since(window.started) < Duration::from_secs(172_800));
    }
}

