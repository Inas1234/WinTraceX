mod app;
mod hook;
mod model {
    pub mod event;
    pub mod filters;
    pub mod ipc;
    pub mod process;
}
mod util {
    pub mod process_launch;
    pub mod time_format;
}

use app::WinApiTraceApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Window API Trace UI (Step 1 Skeleton)",
        options,
        Box::new(|_cc| Ok(Box::new(WinApiTraceApp::default()))),
    )
}
