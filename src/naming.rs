use chrono::Local;

/// Generate a timestamp-based WebP filename, e.g. `20260312_120233.webp`.
pub fn generate_filename() -> String {
    format!("{}.webp", Local::now().format("%Y%m%d_%H%M%S"))
}
