use crate::model::event::Event;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LoadedDll {
    pub name: String,
    pub path: String,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub count: u32,
    pub last_summary: String,
}

#[derive(Debug, Default)]
pub struct LoadedDlls {
    by_key: HashMap<String, LoadedDll>,
}

impl LoadedDlls {
    pub fn observe_event(&mut self, event: &Event) {
        if event.api != "DllLoad" {
            return;
        }

        let path = event.result.trim().to_owned();
        let key = if path.is_empty() || path == "(failed)" {
            event.summary.clone()
        } else {
            path.clone()
        };

        let name = if path.is_empty() || path == "(failed)" {
            "<unknown>".to_owned()
        } else {
            basename(&path).to_owned()
        };

        let entry = self.by_key.entry(key.clone()).or_insert_with(|| LoadedDll {
            name,
            path: path.clone(),
            first_seen_ms: event.timestamp_ms,
            last_seen_ms: event.timestamp_ms,
            count: 0,
            last_summary: String::new(),
        });

        entry.last_seen_ms = event.timestamp_ms;
        entry.count = entry.count.saturating_add(1);
        entry.last_summary = event.summary.clone();
        if !path.is_empty() {
            entry.path = path;
        }
    }

    pub fn values(&self) -> impl Iterator<Item = &LoadedDll> {
        self.by_key.values()
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }
}

fn basename(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}
