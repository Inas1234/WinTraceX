use crate::hook::injector::inject_agent_dll;
use crate::hook::udp_listener::start_udp_event_listener;
use crate::hook::{HookManager, trigger_smoke_test_call};
use crate::model::dll::LoadedDlls;
use crate::model::event::Event;
use crate::model::filters::{EventFilters, EventSortColumn};
use crate::model::process::{ProcessEntry, enumerate_processes};
use crate::util::process_launch::launch_target_exe_suspended;
use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

pub mod widgets {
    pub mod details_panel;
    pub mod dll_table;
    pub mod event_table;
    pub mod left_panel;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainTab {
    Events,
    Dlls,
}

pub struct WinApiTraceApp {
    pid_input: String,
    launch_exe_path: String,
    attach_status: String,
    events: Vec<Event>,
    filters: EventFilters,
    selected_event: Option<usize>,
    main_tab: MainTab,
    dlls: LoadedDlls,
    dll_query: String,
    hook_manager: HookManager,
    event_tx: Sender<Event>,
    event_rx: Receiver<Event>,
    processes: Vec<ProcessEntry>,
    selected_process: Option<usize>,
}

impl Default for WinApiTraceApp {
    fn default() -> Self {
        let (event_tx, event_rx) = mpsc::channel();

        let mut app = Self {
            pid_input: String::new(),
            launch_exe_path: String::new(),
            attach_status: "Not attached".to_owned(),
            events: Vec::new(),
            filters: EventFilters::default(),
            selected_event: None,
            main_tab: MainTab::Events,
            dlls: LoadedDlls::default(),
            dll_query: String::new(),
            hook_manager: HookManager::default(),
            event_tx,
            event_rx,
            processes: Vec::new(),
            selected_process: None,
        };

        app.refresh_process_list();
        if let Err(error) = start_udp_event_listener(app.event_tx.clone()) {
            app.attach_status = format!("Not attached (listener error: {error})");
        }

        app
    }
}

impl WinApiTraceApp {
    fn should_retry_attach_after_resume(error: &str) -> bool {
        error.contains("GetLastError=299")
            || error.contains("last error=299")
            || error.contains("ERROR_PARTIAL_COPY")
            || error.contains("ERROR_BAD_LENGTH")
    }

    fn retry_attach_after_resume(
        &self,
        pid: u32,
        attempts: usize,
    ) -> Result<(usize, String), String> {
        let mut last_error = String::new();
        for attempt in 1..=attempts {
            if attempt > 1 {
                std::thread::sleep(Duration::from_millis(2));
            }
            match inject_agent_dll(pid) {
                Ok(dll_path) => return Ok((attempt, dll_path)),
                Err(error) => {
                    last_error = error;
                }
            }
        }

        Err(last_error)
    }

    fn refresh_process_list(&mut self) {
        match enumerate_processes() {
            Ok(processes) => {
                self.processes = processes;
                if self
                    .selected_process
                    .is_some_and(|index| index >= self.processes.len())
                {
                    self.selected_process = None;
                }
            }
            Err(error) => {
                self.attach_status = format!("Process list refresh failed: {error}");
            }
        }
    }

    fn handle_attach_request(&mut self, pid: u32) {
        let current_pid = unsafe { GetCurrentProcessId() };
        if pid == current_pid {
            match self.hook_manager.install(self.event_tx.clone()) {
                Ok(()) => {
                    self.attach_status =
                        format!("Attached. Local hooks active in PID {current_pid}.");
                    trigger_smoke_test_call();
                }
                Err(error) => {
                    self.attach_status = format!("Hook install failed: {error}");
                }
            }
            return;
        }

        match inject_agent_dll(pid) {
            Ok(dll_path) => {
                self.attach_status = format!("Injected agent into PID {pid} using {dll_path}");
                self.refresh_process_list();
            }
            Err(error) => {
                self.attach_status = format!("Attach failed for PID {pid}: {error}");
            }
        }
    }

    fn handle_launch_and_attach_request(&mut self) {
        match launch_target_exe_suspended(&self.launch_exe_path) {
            Ok(process) => {
                let pid = process.pid;
                self.pid_input = pid.to_string();
                self.attach_status = format!(
                    "Started suspended process PID {pid}, injecting agent before first frame..."
                );

                let attach_result = inject_agent_dll(pid);
                match &attach_result {
                    Ok(dll_path) => {
                        self.attach_status =
                            format!("Injected agent into suspended PID {pid} using {dll_path}");
                    }
                    Err(error) => {
                        self.attach_status = format!(
                            "Attach failed for suspended PID {pid}: {error}. Resuming process anyway."
                        );
                    }
                }

                if let Err(error) = process.resume() {
                    self.attach_status = format!("Launch failed to resume PID {pid}: {error}");
                    return;
                }

                if let Err(error) = attach_result {
                    if Self::should_retry_attach_after_resume(&error) {
                        match self.retry_attach_after_resume(pid, 16) {
                            Ok((attempt, dll_path)) => {
                                self.attach_status = format!(
                                    "Initial suspended attach failed, but retry succeeded after resume (attempt {attempt}) using {dll_path}"
                                );
                            }
                            Err(retry_error) => {
                                self.attach_status = format!(
                                    "Process PID {pid} resumed, initial attach failed ({error}), and retry also failed: {retry_error}"
                                );
                            }
                        }
                    } else {
                        self.attach_status =
                            format!("Process PID {pid} resumed, but attach failed: {error}");
                    }
                } else {
                    self.attach_status = format!(
                        "Attached to PID {pid} before resume. Early graphics init calls should now be visible."
                    );
                }
                std::thread::sleep(Duration::from_millis(120));
                self.refresh_process_list();
            }
            Err(error) => {
                self.attach_status = format!("Launch failed: {error}");
            }
        }
    }

    fn drain_live_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            self.dlls.observe_event(&event);
            self.events.push(event);
        }
    }

    fn visible_event_indices(&self) -> Vec<usize> {
        let mut visible_indices: Vec<usize> = self
            .events
            .iter()
            .enumerate()
            // DLL load events have their own dedicated tab; keep the Events view focused.
            .filter_map(|(index, event)| {
                (event.api != "DllLoad" && self.filters.matches(event)).then_some(index)
            })
            .collect();

        visible_indices.sort_by(|left, right| {
            let left_event = &self.events[*left];
            let right_event = &self.events[*right];

            let ordering = match self.filters.sort.column {
                EventSortColumn::Time => left_event.timestamp_ms.cmp(&right_event.timestamp_ms),
                EventSortColumn::Api => left_event
                    .api
                    .cmp(&right_event.api)
                    .then_with(|| left_event.timestamp_ms.cmp(&right_event.timestamp_ms)),
                EventSortColumn::Caller => left_event
                    .caller
                    .cmp(&right_event.caller)
                    .then_with(|| left_event.timestamp_ms.cmp(&right_event.timestamp_ms)),
            };

            if self.filters.sort.descending {
                ordering.reverse()
            } else {
                ordering
            }
        });

        visible_indices
    }
}

impl eframe::App for WinApiTraceApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(100));

        if let Some(action) = widgets::left_panel::show(
            ctx,
            &mut self.pid_input,
            &mut self.launch_exe_path,
            &self.attach_status,
            &self.processes,
            &mut self.selected_process,
        ) {
            match action {
                widgets::left_panel::LeftPanelAction::Attach(pid) => {
                    self.handle_attach_request(pid)
                }
                widgets::left_panel::LeftPanelAction::RefreshProcesses => {
                    self.refresh_process_list()
                }
                widgets::left_panel::LeftPanelAction::LaunchAndAttach => {
                    self.handle_launch_and_attach_request()
                }
            }
        }

        self.drain_live_events();
        widgets::details_panel::show(
            ctx,
            self.selected_event.and_then(|idx| self.events.get(idx)),
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.main_tab, MainTab::Events, "Events");
                ui.selectable_value(&mut self.main_tab, MainTab::Dlls, "DLLs");
            });
            ui.separator();

            match self.main_tab {
                MainTab::Events => {
                    let visible_indices = self.visible_event_indices();
                    if let Some(selected) = self.selected_event {
                        // If the currently selected event is hidden in the Events view, clear it
                        // so the details panel doesn't keep showing stale/irrelevant data.
                        if !visible_indices.contains(&selected) {
                            self.selected_event = None;
                        }
                    }
                    widgets::event_table::show(
                        ui,
                        &self.events,
                        &visible_indices,
                        &mut self.selected_event,
                        &mut self.filters,
                    );
                }
                MainTab::Dlls => {
                    widgets::dll_table::show(ui, &self.dlls, &mut self.dll_query);
                }
            }
        });
    }
}
