pub fn format_timestamp_ms(timestamp_ms: u64) -> String {
    let millis = timestamp_ms % 1_000;
    let total_seconds = timestamp_ms / 1_000;
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    let minutes = total_minutes % 60;
    let hours = (total_minutes / 60) % 24;

    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}
