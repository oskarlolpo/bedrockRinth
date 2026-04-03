use crate::api::Result;
use daedalus::minecraft::VersionManifest;
use daedalus::modded::Manifest;
use theseus::metadata::{
    BedrockContentDetails, BedrockContentPage, BedrockInstalledContentEntry,
    BedrockVersionEntry,
};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("metadata")
        .invoke_handler(tauri::generate_handler![
            metadata_get_game_versions,
            metadata_get_loader_versions,
            metadata_get_bedrock_versions,
            metadata_get_bedrock_content,
            metadata_get_bedrock_content_details,
            metadata_install_bedrock_content,
            metadata_get_installed_bedrock_content,
        ])
        .build()
}

/// Gets the game versions from daedalus
#[tauri::command]
pub async fn metadata_get_game_versions() -> Result<VersionManifest> {
    Ok(theseus::metadata::get_minecraft_versions().await?)
}

/// Gets the fabric versions from daedalus
#[tauri::command]
pub async fn metadata_get_loader_versions(loader: &str) -> Result<Manifest> {
    Ok(theseus::metadata::get_loader_versions(loader).await?)
}

/// Gets Bedrock versions from the remote LiteLDev version database
#[tauri::command]
pub async fn metadata_get_bedrock_versions() -> Result<Vec<BedrockVersionEntry>>
{
    Ok(theseus::metadata::get_bedrock_versions().await?)
}

#[tauri::command]
pub async fn metadata_get_bedrock_content(
    kind: &str,
    page: Option<u32>,
) -> Result<BedrockContentPage> {
    Ok(theseus::metadata::get_bedrock_content(kind, page).await?)
}

#[tauri::command]
pub async fn metadata_get_bedrock_content_details(
    kind: &str,
    page_url: &str,
) -> Result<BedrockContentDetails> {
    Ok(
        theseus::metadata::get_bedrock_content_details(kind, page_url)
            .await?,
    )
}

#[tauri::command]
pub async fn metadata_install_bedrock_content(
    profile_path: &str,
    kind: &str,
    page_url: &str,
) -> Result<String> {
    Ok(
        theseus::metadata::install_bedrock_content(
            profile_path,
            kind,
            page_url,
        )
        .await?,
    )
}

#[tauri::command]
pub async fn metadata_get_installed_bedrock_content(
    profile_path: &str,
) -> Result<Vec<BedrockInstalledContentEntry>> {
    Ok(
        theseus::metadata::get_installed_bedrock_content(profile_path)
            .await?,
    )
}
