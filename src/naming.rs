use chrono::Local;

/// Generate a timestamp-based PNG filename, e.g. `20260312_120233.png`.
pub fn generate_filename() -> String {
    format!("{}.png", Local::now().format("%Y%m%d_%H%M%S"))
}
