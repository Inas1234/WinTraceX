use crate::model::event::Event;
use crate::model::filters::{EventFilters, EventSortColumn};
use crate::util::time_format::format_timestamp_ms;
use eframe::egui;

pub fn show(
    ctx: &egui::Context,
    events: &[Event],
    visible_indices: &[usize],
    selected_event: &mut Option<usize>,
    filters: &mut EventFilters,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Events");

        ui.horizontal(|ui| {
            ui.label("Text filter:");
            ui.add(
                egui::TextEdit::singleline(&mut filters.text_query)
                    .hint_text("Filter by API name or summary"),
            );
        });

        ui.checkbox(
            &mut filters.only_window_display_apis,
            "Only window/display APIs",
        );

        ui.separator();

        if visible_indices.is_empty() {
            ui.label("No events match current filters.");
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("events_grid")
                    .num_columns(4)
                    .striped(true)
                    .min_col_width(130.0)
                    .show(ui, |ui| {
                        if sort_header_button(
                            ui,
                            "Time",
                            filters.sort.column == EventSortColumn::Time,
                            filters.sort.descending,
                        )
                        .clicked()
                        {
                            filters.toggle_sort(EventSortColumn::Time);
                        }

                        if sort_header_button(
                            ui,
                            "API",
                            filters.sort.column == EventSortColumn::Api,
                            filters.sort.descending,
                        )
                        .clicked()
                        {
                            filters.toggle_sort(EventSortColumn::Api);
                        }

                        ui.strong("Summary");
                        if sort_header_button(
                            ui,
                            "Caller",
                            filters.sort.column == EventSortColumn::Caller,
                            filters.sort.descending,
                        )
                        .clicked()
                        {
                            filters.toggle_sort(EventSortColumn::Caller);
                        }
                        ui.end_row();

                        for &index in visible_indices {
                            let event = &events[index];
                            let is_selected = selected_event.is_some_and(|selected| selected == index);
                            let mut clicked = false;

                            clicked |= ui
                                .selectable_label(is_selected, format_timestamp_ms(event.timestamp_ms))
                                .clicked();
                            clicked |= ui.selectable_label(is_selected, &event.api).clicked();
                            clicked |= ui.selectable_label(is_selected, &event.summary).clicked();
                            clicked |= ui.selectable_label(is_selected, &event.caller).clicked();
                            ui.end_row();

                            if clicked {
                                *selected_event = Some(index);
                            }
                        }
                    });
            });
    });
}

fn sort_header_button(
    ui: &mut egui::Ui,
    label: &str,
    is_active: bool,
    descending: bool,
) -> egui::Response {
    let indicator = if is_active {
        if descending {
            " v"
        } else {
            " ^"
        }
    } else {
        ""
    };

    ui.button(format!("{label}{indicator}"))
}
