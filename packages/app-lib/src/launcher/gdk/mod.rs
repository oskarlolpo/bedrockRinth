pub mod decoder;
pub mod header;
pub mod key;
pub mod stream;
pub mod structs;
pub mod task_stub;
include!(concat!(env!("OUT_DIR"), "/secrets.rs"));

pub fn has_any_cik_key() -> bool {
    std::env::var("GDK_RELEASE_KEY").ok().filter(|s| !s.trim().is_empty()).is_some()
        || std::env::var("GDK_PREVIEW_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .is_some()
        || std::env::var("GDK_RELEASE_KEY_FILE")
            .ok()
            .map(std::path::PathBuf::from)
            .is_some_and(|p| p.exists())
        || std::env::var("GDK_PREVIEW_KEY_FILE")
            .ok()
            .map(std::path::PathBuf::from)
            .is_some_and(|p| p.exists())
        || std::env::var("APPDATA")
            .ok()
            .map(|appdata| {
                std::path::PathBuf::from(appdata)
                    .join("ModrinthApp-oskarlolpo")
                    .join("meta")
                    .join("bedrock")
                    .join("cik")
            })
            .is_some_and(|dir| dir.exists())
        || RELEASE_KEY_HEX.is_some()
        || PREVIEW_KEY_HEX.is_some()
}

pub fn unpack_gdk_sync(
    input_path: &std::path::Path,
    output_dir: &std::path::Path,
    folder_name: &str,
) -> crate::Result<()> {
    let target_dir = output_dir.join(folder_name);
    std::fs::create_dir_all(&target_dir)?;
    let mut stream = stream::MsiXVDStream::new(input_path).map_err(|e| {
        crate::ErrorKind::LauncherError(format!("Failed to parse .msixvc: {e}"))
    })?;
    stream
        .extract_to(&target_dir, "bedrock-gdk".to_string())
        .map_err(|e| {
            crate::ErrorKind::LauncherError(format!(
                "Failed to unpack .msixvc: {e}"
            ))
        })?;
    Ok(())
}
