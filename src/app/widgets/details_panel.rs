use crate::model::event::Event;
use crate::util::time_format::format_timestamp_ms;
use eframe::egui;

pub fn show(ctx: &egui::Context, selected_event: Option<&Event>) {
    egui::SidePanel::right("details_panel")
        .resizable(true)
        .default_width(340.0)
        .show(ctx, |ui| {
            ui.heading("Selected Event Details");
            ui.separator();

            match selected_event {
                Some(event) => {
                    ui.group(|ui| {
                        ui.monospace(format!("Time: {}", format_timestamp_ms(event.timestamp_ms)));
                        ui.monospace(format!("Timestamp (ms): {}", event.timestamp_ms));
                        ui.monospace(format!("API: {}", event.api));
                        ui.monospace(format!("Summary: {}", event.summary));
                        ui.monospace(format!("Caller: {}", event.caller));
                        ui.monospace(format!("Thread ID: {}", event.thread_id));
                        ui.monospace(format!("Result: {}", event.result));
                    });
                }
                None => {
                    ui.label("Select an event to inspect its full fields.");
                }
            }
        });
}
