use eframe::egui;
use crate::model::process::ProcessEntry;
use rfd::FileDialog;

pub enum LeftPanelAction {
    Attach(u32),
    RefreshProcesses,
    LaunchAndAttach,
}

pub fn show(
    ctx: &egui::Context,
    pid_input: &mut String,
    launch_exe_path: &mut String,
    attach_status: &str,
    processes: &[ProcessEntry],
    selected_process: &mut Option<usize>,
) -> Option<LeftPanelAction> {
    let mut action = None;

    egui::SidePanel::left("left_panel")
        .resizable(true)
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("Target Process");
            ui.label("PID");
            ui.text_edit_singleline(pid_input);

            let pid_validation = validate_pid(pid_input);
            let attach_clicked = ui
                .add_enabled(
                    pid_validation.is_ok(),
                    egui::Button::new("Attach"),
                )
                .clicked();

            if attach_clicked {
                action = pid_validation.ok().map(LeftPanelAction::Attach);
            }

            ui.separator();
            ui.heading("Launch Target");
            ui.label("EXE path");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(launch_exe_path);
                if ui.button("Browse...").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Executable", &["exe"])
                        .pick_file()
                    {
                        *launch_exe_path = path.to_string_lossy().to_string();
                    }
                }
            });
            if ui.button("Run EXE and Attach").clicked() {
                action = Some(LeftPanelAction::LaunchAndAttach);
            }

            ui.separator();
            if let Err(message) = pid_validation {
                ui.colored_label(egui::Color32::LIGHT_RED, message);
            }
            ui.label(format!("Status: {attach_status}"));

            ui.separator();
            if ui.button("Refresh process list").clicked() {
                action = Some(LeftPanelAction::RefreshProcesses);
            }

            ui.label(format!("Processes: {}", processes.len()));
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for (index, process) in processes.iter().enumerate() {
                        let is_selected = selected_process.is_some_and(|current| current == index);
                        let label = format!("{} ({})", process.name, process.pid);
                        if ui.selectable_label(is_selected, label).clicked() {
                            *selected_process = Some(index);
                            *pid_input = process.pid.to_string();
                        }
                    }
                });
        });

    action
}

fn validate_pid(pid_input: &str) -> Result<u32, &'static str> {
    let trimmed = pid_input.trim();
    if trimmed.is_empty() {
        return Err("Enter a PID to enable attach.");
    }

    match trimmed.parse::<u32>() {
        Ok(0) => Err("PID must be greater than 0."),
        Ok(pid) => Ok(pid),
        Err(_) => Err("PID must be a valid number."),
    }
}
