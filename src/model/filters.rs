use crate::model::event::Event;

const WINDOW_DISPLAY_APIS: [&str; 5] = [
    "CreateWindowExW",
    "SetWindowPos",
    "MoveWindow",
    "ChangeDisplaySettingsExW",
    "AdjustWindowRectEx",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSortColumn {
    Time,
    Api,
    Caller,
}

#[derive(Debug, Clone, Copy)]
pub struct EventSort {
    pub column: EventSortColumn,
    pub descending: bool,
}

impl Default for EventSort {
    fn default() -> Self {
        Self {
            column: EventSortColumn::Time,
            descending: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct EventFilters {
    pub text_query: String,
    pub only_window_display_apis: bool,
    pub sort: EventSort,
}

impl EventFilters {
    pub fn matches(&self, event: &Event) -> bool {
        if self.only_window_display_apis && !is_window_display_api(&event.api) {
            return false;
        }

        let query = self.text_query.trim();
        if query.is_empty() {
            return true;
        }

        let query_lower = query.to_ascii_lowercase();
        let api_lower = event.api.to_ascii_lowercase();
        let summary_lower = event.summary.to_ascii_lowercase();

        api_lower.contains(&query_lower) || summary_lower.contains(&query_lower)
    }

    pub fn toggle_sort(&mut self, column: EventSortColumn) {
        if self.sort.column == column {
            self.sort.descending = !self.sort.descending;
        } else {
            self.sort.column = column;
            self.sort.descending = false;
        }
    }
}

fn is_window_display_api(api: &str) -> bool {
    WINDOW_DISPLAY_APIS.iter().any(|candidate| *candidate == api)
}
