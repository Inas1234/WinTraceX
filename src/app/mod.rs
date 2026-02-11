use crate::hook::injector::inject_agent_dll;
use crate::hook::udp_listener::start_udp_event_listener;
use crate::hook::{trigger_smoke_test_call, HookManager};
use crate::model::event::Event;
use crate::model::filters::{EventFilters, EventSortColumn};
use crate::model::process::{enumerate_processes, ProcessEntry};
use crate::util::process_launch::launch_target_exe;
use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

pub mod widgets {
    pub mod details_panel;
    pub mod event_table;
    pub mod left_panel;
}

pub struct WinApiTraceApp {
    pid_input: String,
    launch_exe_path: String,
    attach_status: String,
    events: Vec<Event>,
    filters: EventFilters,
    selected_event: Option<usize>,
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
                    self.attach_status = format!("Attached. Local hooks active in PID {current_pid}.");
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
        match launch_target_exe(&self.launch_exe_path) {
            Ok(pid) => {
                self.pid_input = pid.to_string();
                self.attach_status = format!("Started process PID {pid}, attempting attach...");
                self.refresh_process_list();
                self.handle_attach_request(pid);
            }
            Err(error) => {
                self.attach_status = format!("Launch failed: {error}");
            }
        }
    }

    fn drain_live_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            self.events.push(event);
        }
    }

    fn visible_event_indices(&self) -> Vec<usize> {
        let mut visible_indices: Vec<usize> = self
            .events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| self.filters.matches(event).then_some(index))
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
                widgets::left_panel::LeftPanelAction::Attach(pid) => self.handle_attach_request(pid),
                widgets::left_panel::LeftPanelAction::RefreshProcesses => self.refresh_process_list(),
                widgets::left_panel::LeftPanelAction::LaunchAndAttach => {
                    self.handle_launch_and_attach_request()
                }
            }
        }

        self.drain_live_events();
        widgets::details_panel::show(ctx, self.selected_event.and_then(|idx| self.events.get(idx)));

        let visible_indices = self.visible_event_indices();
        widgets::event_table::show(
            ctx,
            &self.events,
            &visible_indices,
            &mut self.selected_event,
            &mut self.filters,
        );
    }
}
