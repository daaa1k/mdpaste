use chrono::Local;

/// Generate a timestamp-based WebP filename, e.g. `20260312_120233.webp`.
pub fn generate_filename() -> String {
    format!("{}.webp", Local::now().format("%Y%m%d_%H%M%S"))
}

/// Generate a timestamp-based WebP filename with a sequence number suffix,
/// e.g. `20260312_120233_2.webp`.  Used when uploading multiple files to
/// avoid name collisions within the same second.
pub fn generate_filename_n(n: usize) -> String {
    format!("{}_{n}.webp", Local::now().format("%Y%m%d_%H%M%S"))
}
