use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::Runtime;
use tauri_plugin_opener::OpenerExt;
use theseus::{
    handler,
    prelude::{CommandPayload, DirectoryInfo},
};

use crate::api::{Result, TheseusSerializableError};
use dashmap::DashMap;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use theseus::prelude::canonicalize;
use url::Url;

fn hidden_powershell_command() -> Command {
    let mut cmd = Command::new("powershell");
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
        cmd.arg("-WindowStyle").arg("Hidden");
    }
    cmd
}

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("utils")
        .invoke_handler(tauri::generate_handler![
            get_os,
            is_network_metered,
            should_disable_mouseover,
            highlight_in_folder,
            open_path,
            show_launcher_logs_folder,
            progress_bars_list,
            get_opening_command,
            import_bedrock_package_file,
            bedrock_user_data_exists,
            backup_bedrock_userdata_zip,
            check_runtime_dependencies,
            install_runtime_dependencies
        ])
        .build()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
pub enum OS {
    Windows,
    Linux,
    MacOS,
}

/// Gets OS
#[tauri::command]
pub fn get_os() -> OS {
    #[cfg(target_os = "windows")]
    let os = OS::Windows;
    #[cfg(target_os = "linux")]
    let os = OS::Linux;
    #[cfg(target_os = "macos")]
    let os = OS::MacOS;
    os
}

#[tauri::command]
pub async fn is_network_metered() -> Result<bool> {
    Ok(theseus::prelude::is_network_metered().await?)
}

// Lists active progress bars
// Create a new HashMap with the same keys
// Values provided should not be used directly, as they are not guaranteed to be up-to-date
#[tauri::command]
pub async fn progress_bars_list()
-> Result<DashMap<uuid::Uuid, theseus::LoadingBar>> {
    let res = theseus::EventState::list_progress_bars().await?;
    Ok(res)
}

// disables mouseover and fixes a random crash error only fixed by recent versions of macos
#[tauri::command]
pub async fn should_disable_mouseover() -> bool {
    if cfg!(target_os = "macos") {
        // We try to match version to 12.2 or higher. If unrecognizable to pattern or lower, we default to the css with disabled mouseover for safety
        if let tauri_plugin_os::Version::Semantic(major, minor, _) =
            tauri_plugin_os::version()
            && major >= 12
            && minor >= 3
        {
            // Mac os version is 12.3 or higher, we allow mouseover
            return false;
        }
        true
    } else {
        // Not macos, we allow mouseover
        false
    }
}

#[tauri::command]
pub fn highlight_in_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) {
    if let Err(e) = app.opener().reveal_item_in_dir(path) {
        tracing::error!("Failed to highlight file in folder: {}", e);
    }
}

#[tauri::command]
pub async fn open_path<R: Runtime>(app: tauri::AppHandle<R>, path: PathBuf) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) =
            app.opener().open_path(path.to_string_lossy(), None::<&str>)
        {
            tracing::error!("Failed to open path: {}", e);
        }
    })
    .await
    .ok();
}

#[tauri::command]
pub async fn show_launcher_logs_folder<R: Runtime>(app: tauri::AppHandle<R>) {
    let path = DirectoryInfo::launcher_logs_dir().unwrap_or_default();
    // failure to get folder just opens filesystem
    // (ie: if in debug mode only and launcher_logs never created)
    open_path(app, path).await;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedBedrockPackage {
    pub game_version: String,
    pub loader_version: String,
    pub suggested_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDependencyStatus {
    pub has_winget: bool,
    pub has_dotnet8: bool,
    pub has_webview2: bool,
    pub missing: Vec<String>,
}

fn bedrock_user_data_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(local)
            .join("Packages")
            .join("Microsoft.MinecraftUWP_8wekyb3d8bbwe")
            .join("LocalState")
            .join("games")
            .join("com.mojang"),
    )
}

#[tauri::command]
pub async fn import_bedrock_package_file(
    path: PathBuf,
) -> Result<ImportedBedrockPackage> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let allowed = ["appx", "msix", "appxbundle", "msixbundle", "msixvc"];
    if !allowed.contains(&ext.as_str()) {
        return Err(TheseusSerializableError::Theseus(
            theseus::ErrorKind::InputError(format!(
                "Unsupported Bedrock package extension: .{}",
                ext
            ))
            .into(),
        ));
    }
    if !path.exists() {
        return Err(TheseusSerializableError::Theseus(
            theseus::ErrorKind::InputError(format!(
                "Package file does not exist: {}",
                path.display()
            ))
            .into(),
        ));
    }

    let state = theseus::State::get().await?;
    let all_versions = state
        .directories
        .metadata_dir()
        .join("bedrock")
        .join("all_versions");
    tokio::fs::create_dir_all(&all_versions).await?;

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bedrock")
        .to_string();
    let normalized_name = format!("{}.{}", stem, ext);
    let target = all_versions.join(&normalized_name);

    if target != path {
        tokio::fs::copy(&path, &target).await?;
    }

    let version_prefix = stem
        .split('_')
        .next()
        .unwrap_or(stem.as_str())
        .trim()
        .to_string();
    let game_version = format!("{}.{}", version_prefix, ext);

    Ok(ImportedBedrockPackage {
        game_version,
        loader_version: normalized_name,
        suggested_name: version_prefix,
    })
}

#[tauri::command]
pub async fn bedrock_user_data_exists() -> Result<bool> {
    Ok(bedrock_user_data_dir()
        .map(|p| p.exists() && p.is_dir())
        .unwrap_or(false))
}

#[tauri::command]
pub async fn backup_bedrock_userdata_zip(output_path: PathBuf) -> Result<()> {
    let source = bedrock_user_data_dir().ok_or_else(|| {
        TheseusSerializableError::Theseus(
            theseus::ErrorKind::InputError(
                "LOCALAPPDATA is not available".to_string(),
            )
            .into(),
        )
    })?;
    if !source.exists() {
        return Err(TheseusSerializableError::Theseus(
            theseus::ErrorKind::InputError(format!(
                "Bedrock user data folder does not exist: {}",
                source.display()
            ))
            .into(),
        ));
    }

    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let src = source.to_string_lossy().replace('\'', "''");
    let out = output_path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $src='{src}'; $out='{out}'; \
         if (Test-Path -LiteralPath $out) {{ Remove-Item -LiteralPath $out -Force }}; \
         Compress-Archive -LiteralPath (Join-Path $src '*') -DestinationPath $out -Force"
    );

    let output = tokio::task::spawn_blocking(move || {
        hidden_powershell_command()
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
    })
    .await
    .map_err(|e| std::io::Error::other(format!("Task join error: {e}")))??;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TheseusSerializableError::Theseus(
            theseus::ErrorKind::OtherError(format!(
                "Failed to create Bedrock backup zip: {stderr}"
            ))
            .into(),
        ));
    }

    Ok(())
}

fn has_winget() -> bool {
    Command::new("where")
        .arg("winget")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn has_dotnet8_runtime() -> bool {
    let Ok(output) = Command::new("dotnet").arg("--list-runtimes").output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    stdout.contains("microsoft.netcore.app 8.")
        || stdout.contains("microsoft.windowsdesktop.app 8.")
}

fn has_webview2_runtime() -> bool {
    let script = "$ids=@('HKLM:\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}','HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'); \
                  foreach($i in $ids){$v=(Get-ItemProperty -Path $i -Name pv -ErrorAction SilentlyContinue).pv; if($v){Write-Output $v; exit 0}}; exit 1";
    hidden_powershell_command()
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tauri::command]
pub async fn check_runtime_dependencies() -> Result<RuntimeDependencyStatus> {
    let status = tokio::task::spawn_blocking(|| {
        let has_winget = has_winget();
        let has_dotnet8 = has_dotnet8_runtime();
        let has_webview2 = has_webview2_runtime();
        let mut missing = Vec::new();
        if !has_dotnet8 {
            missing.push(".NET Runtime 8".to_string());
        }
        if !has_webview2 {
            missing.push("WebView2 Runtime".to_string());
        }
        if !has_winget {
            missing.push("WinGet".to_string());
        }
        RuntimeDependencyStatus {
            has_winget,
            has_dotnet8,
            has_webview2,
            missing,
        }
    })
    .await
    .map_err(|e| std::io::Error::other(format!("Task join error: {e}")))?;
    Ok(status)
}

#[tauri::command]
pub async fn install_runtime_dependencies() -> Result<String> {
    let output = tokio::task::spawn_blocking(|| {
        let script = "$ErrorActionPreference='Continue'; \
                      if (-not (Get-Command winget -ErrorAction SilentlyContinue)) { throw 'winget is not installed or not available in PATH' }; \
                      $common='--accept-package-agreements --accept-source-agreements --disable-interactivity'; \
                      winget install --id Microsoft.DotNet.Runtime.8 -e $common --silent; \
                      winget install --id Microsoft.EdgeWebView2Runtime -e $common --silent; \
                      winget install --id Microsoft.VCRedist.2015+.x64 -e $common --silent; \
                      Write-Output 'Runtime dependency installation completed.'";
        hidden_powershell_command()
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
    })
    .await
    .map_err(|e| std::io::Error::other(format!("Task join error: {e}")))??;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(TheseusSerializableError::Theseus(
            theseus::ErrorKind::OtherError(format!(
                "Dependency installation failed: {}",
                if stderr.trim().is_empty() {
                    stdout
                } else {
                    stderr
                }
            ))
            .into(),
        ));
    }
    Ok(stdout)
}

// Get opening command
// For example, if a user clicks on an .mrpack to open the app.
// This should be called once and only when the app is done booting up and ready to receive a command
// Returns a Command struct- see events.js
#[tauri::command]
#[cfg(target_os = "macos")]
pub async fn get_opening_command(
    state: tauri::State<'_, crate::macos::deep_link::InitialPayload>,
) -> Result<Option<CommandPayload>> {
    let payload = state.payload.lock().await;

    return if let Some(payload) = payload.as_ref() {
        tracing::info!("opening command {payload}");

        Ok(Some(handler::parse_command(payload).await?))
    } else {
        Ok(None)
    };
}

#[tauri::command]
#[cfg(not(target_os = "macos"))]
pub async fn get_opening_command() -> Result<Option<CommandPayload>> {
    // Tauri is not CLI, we use arguments as path to file to call
    let cmd_arg = std::env::args_os().nth(1);

    tracing::info!("opening command {cmd_arg:?}");

    let cmd_arg = cmd_arg.map(|path| path.to_string_lossy().to_string());
    if let Some(cmd) = cmd_arg {
        tracing::debug!("Opening command: {:?}", cmd);
        return Ok(Some(handler::parse_command(&cmd).await?));
    }
    Ok(None)
}

// helper function called when redirected by a weblink (ie: modrith://do-something) or when redirected by a .mrpack file (in which case its a filepath)
// We hijack the deep link library (which also contains functionality for instance-checking)
pub async fn handle_command(command: String) -> Result<()> {
    tracing::info!("handle command: {command}");
    Ok(theseus::handler::parse_and_emit_command(&command).await?)
}

// Remove when (and if) https://github.com/tauri-apps/tauri/issues/12022 is implemented
pub(crate) fn tauri_convert_file_src(path: &Path) -> Result<Url> {
    #[cfg(any(windows, target_os = "android"))]
    const BASE: &str = "http://asset.localhost/";
    #[cfg(not(any(windows, target_os = "android")))]
    const BASE: &str = "asset://localhost/";

    macro_rules! theseus_try {
        ($test:expr) => {
            match $test {
                Ok(val) => val,
                Err(e) => {
                    return Err(TheseusSerializableError::Theseus(e.into()))
                }
            }
        };
    }

    let path = theseus_try!(canonicalize(path));
    let path = path.to_string_lossy();
    let encoded = urlencoding::encode(&path);

    Ok(theseus_try!(Url::parse(&format!("{BASE}{encoded}"))))
}
