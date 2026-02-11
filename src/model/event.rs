use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp_ms: u64,
    pub api: String,
    pub summary: String,
    pub caller: String,
    pub thread_id: u32,
    pub result: String,
}

impl Event {
    #[allow(dead_code)]
    fn new(
        timestamp_ms: u64,
        api: &str,
        summary: &str,
        caller: &str,
        thread_id: u32,
        result: &str,
    ) -> Self {
        Self {
            timestamp_ms,
            api: api.to_owned(),
            summary: summary.to_owned(),
            caller: caller.to_owned(),
            thread_id,
            result: result.to_owned(),
        }
    }

    #[allow(dead_code)]
    pub fn sample_events() -> Vec<Self> {
        vec![
            Self::new(12_100, "CreateWindowExW", "Main window created (1280x720)", "game.exe+0x1A20", 1884, "HWND=0x00000000001204F0"),
            Self::new(12_420, "SetWindowPos", "Resize to 1600x900", "game.exe+0x21D0", 1884, "TRUE"),
            Self::new(12_720, "SendMessageW", "WM_SIZE dispatched", "user32.dll+0xA2B0", 1884, "LRESULT=0"),
            Self::new(13_050, "MoveWindow", "Move to x=100 y=80", "game.exe+0x22AF", 1884, "TRUE"),
            Self::new(13_410, "ChangeDisplaySettingsExW", "Switch display mode to 1920x1080@60", "render.dll+0x45E1", 4120, "DISP_CHANGE_SUCCESSFUL"),
            Self::new(13_900, "GetClientRect", "Client rect requested", "game.exe+0x1902", 1884, "TRUE"),
            Self::new(14_210, "AdjustWindowRectEx", "Frame recalculated for WS_OVERLAPPEDWINDOW", "game.exe+0x2B31", 1884, "TRUE"),
            Self::new(14_520, "ShowWindow", "Window shown with SW_SHOW", "game.exe+0x2050", 1884, "TRUE"),
            Self::new(14_880, "DefWindowProcW", "Default handling for WM_DISPLAYCHANGE", "user32.dll+0x8430", 1884, "LRESULT=0"),
            Self::new(15_130, "SetWindowPos", "Topmost flag toggled off", "game.exe+0x2348", 1884, "TRUE"),
        ]
    }
}
