use crate::model::event::Event;
use crate::util::ascii::contains_ignore_ascii_case;

const WINDOW_DISPLAY_APIS: [&str; 5] = [
    "CreateWindowExW",
    "SetWindowPos",
    "MoveWindow",
    "ChangeDisplaySettingsExW",
    "AdjustWindowRectEx",
];

const OTHER_GRAPHICS_APIS: [&str; 8] = [
    "Direct3DCreate9",
    "Direct3DCreate9Ex",
    "CreateDXGIFactory",
    "CreateDXGIFactory1",
    "D3D11CreateDevice",
    "D3D11CreateDeviceAndSwapChain",
    // These are useful when troubleshooting hook setup, but they are not DirectDraw API calls.
    "DirectDrawHookStatus",
    "DirectDrawHookInstall",
];

const DIRECTDRAW_NONCALL_EVENTS: [&str; 2] = ["DirectDrawHookStatus", "DirectDrawHookInstall"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiScope {
    All,
    WindowDisplayAndGraphics,
    DirectDrawCallsOnly,
}

impl Default for ApiScope {
    fn default() -> Self {
        Self::All
    }
}

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

#[derive(Debug)]
pub struct EventFilters {
    pub text_query: String,
    pub api_scope: ApiScope,
    pub sort: EventSort,
}

impl Default for EventFilters {
    fn default() -> Self {
        Self {
            text_query: String::new(),
            api_scope: ApiScope::DirectDrawCallsOnly,
            sort: EventSort::default(),
        }
    }
}

impl EventFilters {
    pub fn matches(&self, event: &Event) -> bool {
        match self.api_scope {
            ApiScope::All => {}
            ApiScope::WindowDisplayAndGraphics => {
                if !is_window_display_or_graphics_api(&event.api) {
                    return false;
                }
            }
            ApiScope::DirectDrawCallsOnly => {
                if !is_directdraw_call_api(&event.api) {
                    return false;
                }
            }
        }

        let query = self.text_query.trim();
        if query.is_empty() {
            return true;
        }

        contains_ignore_ascii_case(&event.api, query)
            || contains_ignore_ascii_case(&event.summary, query)
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
    WINDOW_DISPLAY_APIS
        .iter()
        .any(|candidate| *candidate == api)
}

fn is_other_graphics_api(api: &str) -> bool {
    OTHER_GRAPHICS_APIS
        .iter()
        .any(|candidate| *candidate == api)
}

fn is_window_display_or_graphics_api(api: &str) -> bool {
    is_window_display_api(api) || is_other_graphics_api(api) || is_directdraw_api(api)
}

fn is_directdraw_api(api: &str) -> bool {
    // Prefer prefix-based matching so new DirectDraw/IDirectDraw events automatically fall under the filter.
    (api.starts_with("CoCreateInstance") && api.contains("DirectDraw"))
        || api.starts_with("DirectDraw")
        || api.starts_with("IDirectDraw")
}

fn is_directdraw_call_api(api: &str) -> bool {
    if DIRECTDRAW_NONCALL_EVENTS
        .iter()
        .any(|candidate| *candidate == api)
    {
        return false;
    }

    is_directdraw_api(api)
}
