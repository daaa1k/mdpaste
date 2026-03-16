pub mod local;
pub mod nodebb;
pub mod r2;

/// Map a lowercase file extension to its MIME type.
pub(super) fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        _ => "image/webp",
    }
}
