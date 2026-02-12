use crate::model::dll::{LoadedDll, LoadedDlls};
use crate::util::ascii::contains_ignore_ascii_case;
use crate::util::time_format::format_timestamp_ms;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, dlls: &LoadedDlls, query: &mut String) {
    ui.heading("DLLs");

    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.add(egui::TextEdit::singleline(query).hint_text("Filter by name/path"));
    });

    ui.label(format!("Unique DLLs: {}", dlls.len()));
    ui.separator();

    let q = query.trim();

    let mut rows: Vec<&LoadedDll> = dlls
        .values()
        .filter(|dll| {
            if q.is_empty() {
                return true;
            }
            contains_ignore_ascii_case(&dll.name, q)
                || contains_ignore_ascii_case(&dll.path, q)
                || contains_ignore_ascii_case(&dll.last_summary, q)
        })
        .collect();

    rows.sort_by(|a, b| b.last_seen_ms.cmp(&a.last_seen_ms));

    if rows.is_empty() {
        ui.label("No DLLs match current filter.");
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("dlls_grid")
                .num_columns(5)
                .striped(true)
                .min_col_width(130.0)
                .show(ui, |ui| {
                    ui.strong("First Seen");
                    ui.strong("Last Seen");
                    ui.strong("Name");
                    ui.strong("Path");
                    ui.strong("Count");
                    ui.end_row();

                    for dll in rows {
                        ui.monospace(format_timestamp_ms(dll.first_seen_ms));
                        ui.monospace(format_timestamp_ms(dll.last_seen_ms));
                        ui.monospace(&dll.name);
                        ui.monospace(&dll.path);
                        ui.monospace(dll.count.to_string());
                        ui.end_row();

                        // Optional extra context line (kept compact).
                        ui.monospace("");
                        ui.monospace("");
                        ui.monospace("last:");
                        ui.monospace(&dll.last_summary);
                        ui.monospace("");
                        ui.end_row();
                    }
                });
        });
}
