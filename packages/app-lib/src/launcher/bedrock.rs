use crate::event::emit::{emit_loading, init_or_edit_loading};
use crate::event::{LoadingBarId, LoadingBarType};
use crate::event::emit::emit_process;
use crate::event::ProcessPayloadType;
use crate::state::{Profile, ProfileInstallStage};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::SystemTime;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use crate::state::ProcessMetadata;
use chrono::Utc;
use regex::Regex;
use std::sync::LazyLock;
use uuid::Uuid;
use zip::ZipArchive;

static BEDROCK_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+\.\d+\.\d+(?:\.\d+)?)").expect("bedrock version regex")
});
static APPX_IDENTITY_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<Identity\b[^>]*\bVersion="([^"]+)""#)
        .expect("appx identity version regex")
});
const BEDROCK_PENDING_WORLD_IMPORTS_DIR: &str = "pending_world_imports";

#[cfg(windows)]
fn hidden_powershell_command() -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.creation_flags(0x08000000);
    cmd.arg("-WindowStyle").arg("Hidden");
    cmd
}

pub async fn install_bedrock(
    profile: &Profile,
    existing_loading_bar: Option<LoadingBarId>,
) -> crate::Result<()> {
    let loading_bar = init_or_edit_loading(
        existing_loading_bar,
        LoadingBarType::MinecraftDownload {
            profile_name: profile.name.clone(),
            profile_path: profile.path.clone(),
        },
        100.0,
        "Downloading Minecraft Bedrock",
    )
    .await?;

    crate::api::profile::edit(&profile.path, |prof| {
        prof.install_stage = ProfileInstallStage::MinecraftInstalling;
        async { Ok(()) }
    })
    .await?;

    let mut progress = 0.0f64;
    bump_progress(
        &loading_bar,
        &mut progress,
        8.0,
        "Preparing Bedrock profile data",
    )?;

    let state = crate::State::get().await?;
    let profile_full_path = crate::api::profile::get_full_path(&profile.path).await?;
    initialize_profile_game_data(profile, &state, &profile_full_path).await?;

    bump_progress(
        &loading_bar,
        &mut progress,
        15.0,
        "Resolving Bedrock file from releases",
    )?;

    let versions = crate::api::metadata::get_bedrock_versions().await?;
    let selected = versions
        .into_iter()
        .find(|(v, _, _)| v.eq_ignore_ascii_case(&profile.game_version))
        .ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!(
                "Bedrock asset not found in releases: {}",
                profile.game_version
            ))
        })?;

    let download_url = selected.1;
    let file_name = filename_from_url(&download_url)
        .unwrap_or_else(|| format!("{}.pkg", profile.game_version));

    let shared_dir = state.directories.metadata_dir().join("bedrock").join("all_versions");
    tokio::fs::create_dir_all(&shared_dir).await?;
    let package_path = shared_dir.join(&file_name);
    let partial_path = package_path.with_extension(format!(
        "{}.part",
        package_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("download")
    ));

    if tokio::fs::try_exists(&package_path).await.unwrap_or(false) {
        if let Ok(meta) = tokio::fs::metadata(&package_path).await {
            let remote_size = fetch_remote_content_length(&download_url).await?;
            let local_size = meta.len();
            if local_size > 0 && (remote_size == 0 || local_size == remote_size) {
                bump_progress(
                    &loading_bar,
                    &mut progress,
                    70.0,
                    "Bedrock file already downloaded",
                )?;
                maybe_install_on_create(
                    &package_path,
                    &state,
                    &loading_bar,
                    &mut progress,
                )
                .await?;
                bump_progress(
                    &loading_bar,
                    &mut progress,
                    95.0,
                    "Finalizing Bedrock profile",
                )?;
                finalize_profile(profile, &file_name, &loading_bar, &mut progress).await?;
                return Ok(());
            }
            let _ = tokio::fs::remove_file(&package_path).await;
        }
    }
    let _ = tokio::fs::remove_file(&partial_path).await;

    let client = reqwest::Client::builder()
        .user_agent(crate::launcher_user_agent())
        .no_proxy()
        .build()?;

    let response = client.get(&download_url).send().await?.error_for_status()?;
    let total = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&partial_path).await?;
    let mut downloaded: u64 = 0;
    let mut last_emitted_progress = 15.0f64;

    while let Some(chunk) = futures::StreamExt::next(&mut stream).await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if total > 0 {
            let ratio = (downloaded as f64 / total as f64).clamp(0.0, 1.0);
            let target_progress = 15.0 + ratio * 70.0;
            let delta = target_progress - last_emitted_progress;
            if delta > 0.0 {
                emit_loading(
                    &loading_bar,
                    delta,
                    Some("Downloading Minecraft Bedrock"),
                )?;
                last_emitted_progress = target_progress;
            }
        }
    }

    file.flush().await?;
    drop(file);

    if total > 0 && downloaded != total {
        let _ = tokio::fs::remove_file(&partial_path).await;
        return Err(crate::ErrorKind::LauncherError(format!(
            "Incomplete Bedrock download: got {} bytes, expected {} bytes",
            downloaded, total
        ))
        .as_error());
    }

    tokio::fs::rename(&partial_path, &package_path).await?;

    if total == 0 {
        let target_progress = 85.0f64;
        let delta = target_progress - last_emitted_progress;
        if delta > 0.0 {
            emit_loading(
                &loading_bar,
                delta,
                Some("Downloading Minecraft Bedrock"),
            )?;
        }
    }

    maybe_install_on_create(
        &package_path,
        &state,
        &loading_bar,
        &mut progress,
    )
    .await?;
    bump_progress(
        &loading_bar,
        &mut progress,
        95.0,
        "Finalizing Bedrock profile",
    )?;
    finalize_profile(profile, &file_name, &loading_bar, &mut progress).await?;
    Ok(())
}

async fn finalize_profile(
    profile: &Profile,
    file_name: &str,
    loading_bar: &LoadingBarId,
    progress: &mut f64,
) -> crate::Result<()> {
    crate::api::profile::edit(&profile.path, |prof| {
        prof.install_stage = ProfileInstallStage::Installed;
        prof.loader_version = Some(file_name.to_string());
        async { Ok(()) }
    })
    .await?;
    bump_progress(
        loading_bar,
        progress,
        100.0,
        "Finished downloading Minecraft Bedrock",
    )?;
    Ok(())
}

fn filename_from_url(url: &str) -> Option<String> {
    let without_query = url.split('?').next()?;
    let name = without_query.rsplit('/').next()?;
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

async fn fetch_remote_content_length(url: &str) -> crate::Result<u64> {
    let client = reqwest::Client::builder()
        .user_agent(crate::launcher_user_agent())
        .no_proxy()
        .build()?;

    let head = client.head(url).send().await?;
    if head.status().is_success() {
        if let Some(size) = head.content_length() {
            return Ok(size);
        }
    }

    let get = client
        .get(url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await?;
    if get.status().is_success() || get.status().as_u16() == 206 {
        if let Some(size) = get.content_length() {
            return Ok(size);
        }
        if let Some(content_range) = get.headers().get(reqwest::header::CONTENT_RANGE)
        {
            if let Ok(range) = content_range.to_str() {
                if let Some(total) = parse_total_from_content_range(range) {
                    return Ok(total);
                }
            }
        }
    }

    Ok(0)
}

fn parse_total_from_content_range(header_value: &str) -> Option<u64> {
    // Example: "bytes 0-0/1771024384"
    let (_, total) = header_value.rsplit_once('/')?;
    total.trim().parse::<u64>().ok()
}

pub async fn launch_bedrock(profile: &Profile) -> crate::Result<ProcessMetadata> {
    #[cfg(windows)]
    {
        let loading_bar = crate::event::emit::init_loading(
            LoadingBarType::MinecraftDownload {
                profile_name: profile.name.clone(),
                profile_path: profile.path.clone(),
            },
            100.0,
            "Preparing Bedrock launch",
        )
        .await?;
        let mut progress = 0.0f64;

        bump_progress(
            &loading_bar,
            &mut progress,
            8.0,
            "Preparing Bedrock profile files",
        )?;
        let state = crate::State::get().await?;
        let profile_full_path = crate::api::profile::get_full_path(&profile.path).await?;
        let profile_game_data = profile_bedrock_data_dir(&profile_full_path);
        initialize_profile_game_data(profile, &state, &profile_full_path)
            .await
            .map_err(|e| {
                crate::ErrorKind::LauncherError(format!(
                    "Bedrock prepare profile data failed: {}",
                    e
                ))
                .as_error()
            })?;

        bump_progress(
            &loading_bar,
            &mut progress,
            18.0,
            "Checking installed Minecraft Bedrock",
        )?;
        ensure_required_bedrock_version(profile, &state, &loading_bar, &mut progress)
            .await
            .map_err(|e| {
                crate::ErrorKind::LauncherError(format!(
                    "Bedrock ensure required version failed: {}",
                    e
                ))
                .as_error()
            })?;

        bump_progress(
            &loading_bar,
            &mut progress,
            35.0,
            "Switching active Bedrock files",
        )?;
        let system_game_data = current_system_game_data(profile)?;
        activate_profile_game_data(
            &profile.path,
            &profile_game_data,
            &system_game_data,
            &state,
            is_gdk_profile(profile),
        )
        .await
        .map_err(|e| {
            crate::ErrorKind::LauncherError(format!(
                "Bedrock switch active files failed (system='{}', profile='{}'): {}",
                system_game_data.display(),
                profile_game_data.display(),
                e
            ))
            .as_error()
        })?;

        bump_progress(
            &loading_bar,
            &mut progress,
            55.0,
            "Importing pending Bedrock worlds",
        )?;
        import_pending_world_files(&profile_full_path, &profile_game_data).await?;

        bump_progress(
            &loading_bar,
            &mut progress,
            75.0,
            "Launching Minecraft Bedrock",
        )?;
        if is_gdk_profile(profile)
            && let Some(exe_path) =
                ensure_gdk_local_install_ready(profile, &state, &loading_bar, &mut progress)
                    .await?
            && try_launch_bedrock_executable(&exe_path).await
        {
            bump_progress(
                &loading_bar,
                &mut progress,
                100.0,
                "Minecraft Bedrock started (GDK direct launch)",
            )?;
            crate::api::profile::edit(&profile.path, |prof| {
                prof.last_played = Some(Utc::now());
                async { Ok(()) }
            })
            .await?;
            let metadata = ProcessMetadata {
                uuid: Uuid::new_v4(),
                profile_path: profile.path.clone(),
                start_time: Utc::now(),
            };
            emit_process(
                &metadata.profile_path,
                metadata.uuid,
                ProcessPayloadType::Launched,
                "Launched Minecraft Bedrock (GDK direct)",
            )
            .await?;
            spawn_bedrock_exit_monitor(metadata.clone(), profile.path.clone());
            return Ok(metadata);
        }
        let app_ids = discover_bedrock_app_ids().await;
        if app_ids.is_empty() {
            return Err(crate::ErrorKind::LauncherError(
                "Bedrock is not installed as a UWP app on this system (no AppID found). Install the selected APPX/MSIX first."
                    .to_string(),
            )
            .as_error());
        }
        for app_id in &app_ids {
            if try_launch_bedrock_app_id(&app_id).await {
                bump_progress(
                    &loading_bar,
                    &mut progress,
                    100.0,
                    "Minecraft Bedrock started",
                )?;
                crate::api::profile::edit(&profile.path, |prof| {
                    prof.last_played = Some(Utc::now());
                    async { Ok(()) }
                })
                .await?;
                let metadata = ProcessMetadata {
                    uuid: Uuid::new_v4(),
                    profile_path: profile.path.clone(),
                    start_time: Utc::now(),
                };
                emit_process(
                    &metadata.profile_path,
                    metadata.uuid,
                    ProcessPayloadType::Launched,
                    "Launched Minecraft Bedrock",
                )
                .await?;
                spawn_bedrock_exit_monitor(metadata.clone(), profile.path.clone());
                return Ok(metadata);
            }
        }

        let tried = app_ids.join(", ");
        Err(crate::ErrorKind::LauncherError(
            format!(
                "Failed to launch Bedrock UWP app. No valid AppID could be started. Candidates: {}",
                if tried.is_empty() { "<none>".to_string() } else { tried }
            ),
        )
        .as_error())
    }
    #[cfg(not(windows))]
    {
        let _ = profile;
        Err(crate::ErrorKind::LauncherError(
            "Bedrock launch is only supported on Windows".to_string(),
        )
        .as_error())
    }
}

fn spawn_bedrock_exit_monitor(metadata: ProcessMetadata, profile_path: String) {
    tokio::spawn(async move {
        let mut seen_running = false;
        let mut ticks: u16 = 0;

        loop {
            let running = is_bedrock_process_running().await;
            if running {
                seen_running = true;
            } else if seen_running {
                let _ = emit_process(
                    &metadata.profile_path,
                    metadata.uuid,
                    ProcessPayloadType::Finished,
                    "Minecraft Bedrock exited",
                )
                .await;

                let elapsed_seconds = Utc::now()
                    .signed_duration_since(metadata.start_time)
                    .num_seconds()
                    .max(0) as u64;
                if elapsed_seconds > 0 {
                    let _ = crate::api::profile::edit(&profile_path, |prof| {
                        prof.recent_time_played =
                            prof.recent_time_played.saturating_add(elapsed_seconds);
                        prof.last_played = Some(Utc::now());
                        async { Ok(()) }
                    })
                    .await;
                    let profile_path_clone = profile_path.clone();
                    tokio::spawn(async move {
                        let _ = crate::api::profile::try_update_playtime(
                            &profile_path_clone,
                        )
                        .await;
                    });
                }

                if let Ok(Some(profile)) = crate::api::profile::get(&profile_path).await {
                    let _ = sync_profile_data_from_system(&profile).await;
                }
                break;
            }

            // Avoid endless watcher if launch failed before process window appeared.
            ticks = ticks.saturating_add(1);
            if !seen_running && ticks > 120 {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

#[cfg(windows)]
async fn is_bedrock_process_running() -> bool {
    let output = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "$ErrorActionPreference='SilentlyContinue'; if (Get-Process -Name 'Minecraft.Windows') { exit 0 } else { exit 1 }",
        ])
        .status()
        .await;
    matches!(output, Ok(status) if status.success())
}

fn is_gdk_profile(profile: &Profile) -> bool {
    profile
        .game_version
        .trim()
        .to_ascii_lowercase()
        .ends_with(".msixvc")
        || profile
            .loader_version
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(".msixvc")
}

#[cfg(windows)]
async fn find_installed_bedrock_exe_path(
    desired_version: Option<&str>,
) -> crate::Result<Option<PathBuf>> {
    let desired = desired_version.unwrap_or("").trim().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         $pkgs = Get-AppxPackage -Name 'Microsoft.Minecraft*' | Sort-Object Version -Descending; \
         if ('{desired}' -ne '') {{ \
            $match = $pkgs | Where-Object {{ $_.Version.ToString().StartsWith('{desired}') }}; \
            if ($match) {{ $pkgs = @($match) + @($pkgs | Where-Object {{ -not $_.Version.ToString().StartsWith('{desired}') }}) }} \
         }}; \
         foreach ($p in $pkgs) {{ \
            if ($p.InstallLocation) {{ \
               $exe = Join-Path $p.InstallLocation 'Minecraft.Windows.exe'; \
               if (Test-Path $exe) {{ $exe; break }} \
            }} \
         }}"
    );
    let output = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
        .await?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let exe = text.lines().next().unwrap_or_default().trim();
    if exe.is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(exe)))
}

#[cfg(not(windows))]
async fn find_installed_bedrock_exe_path(
    _desired_version: Option<&str>,
) -> crate::Result<Option<PathBuf>> {
    Ok(None)
}

#[cfg(windows)]
async fn try_launch_bedrock_executable(exe_path: &Path) -> bool {
    let escaped = exe_path.to_string_lossy().replace('\'', "''");
    let status = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!(
                "try {{ Start-Process -FilePath '{}' -ErrorAction Stop; exit 0 }} catch {{ exit 1 }}",
                escaped
            ),
        ])
        .status()
        .await;
    matches!(status, Ok(s) if s.success())
}

#[cfg(not(windows))]
async fn try_launch_bedrock_executable(_exe_path: &Path) -> bool {
    false
}

#[cfg(not(windows))]
async fn is_bedrock_process_running() -> bool {
    false
}

#[cfg(windows)]
async fn discover_bedrock_app_ids() -> Vec<String> {
    let mut uniq = HashSet::<String>::new();
    let mut out = Vec::<String>::new();

    let scripts = [
        // Primary source: Start menu registered AppIDs.
        "$ErrorActionPreference='SilentlyContinue'; Get-StartApps | ForEach-Object { $_.AppID } | Where-Object { $_ -match 'Minecraft' }",
        // Better-style source: build AppUserModelID from installed package manifest Application Id.
        "$ErrorActionPreference='SilentlyContinue'; Get-AppxPackage -Name 'Microsoft.Minecraft*' | ForEach-Object { $pf=$_.PackageFamilyName; $manifest=Join-Path $_.InstallLocation 'AppxManifest.xml'; if (Test-Path $manifest) { try { [xml]$x=Get-Content -LiteralPath $manifest; foreach($app in $x.Package.Applications.Application){ if($app.Id){ \"$pf!$($app.Id)\" } } } catch {} } }",
        // Fallback source: installed package family names mapped to conventional entrypoint.
        "$ErrorActionPreference='SilentlyContinue'; Get-AppxPackage -Name 'Microsoft.Minecraft*' | ForEach-Object { \"$($_.PackageFamilyName)!App\" }",
    ];

    for script in scripts {
        let output = hidden_powershell_command()
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .output()
            .await;

        if let Ok(output) = output
            && output.status.success()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let app_id = line.trim();
                if app_id.is_empty() || !app_id.contains('!') {
                    continue;
                }
                if uniq.insert(app_id.to_string()) {
                    out.push(app_id.to_string());
                }
            }
        }
    }

    out
}

#[cfg(windows)]
async fn try_launch_bedrock_app_id(app_id: &str) -> bool {
    let shell_target = format!("shell:AppsFolder\\{app_id}");
    let escaped = shell_target.replace('\'', "''");
    let powershell_start = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!("try {{ Start-Process '{}' -ErrorAction Stop; exit 0 }} catch {{ exit 1 }}", escaped),
        ])
        .status()
        .await;
    if let Ok(status) = powershell_start
        && status.success()
    {
        return true;
    }

    false
}

async fn install_downloaded_bedrock(
    package_path: &Path,
    _state: &crate::State,
    loading_bar: &LoadingBarId,
) -> crate::Result<()> {
    let ext = package_path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "appx" | "msix" | "appxbundle" | "msixbundle" => {
            emit_loading(
                loading_bar,
                10.0,
                Some("Installing APPX package"),
            )?;
            crate::launcher::appx::install_appx_package(package_path).await?;
        }
        "msixvc" => {
            emit_loading(
                loading_bar,
                10.0,
                Some("GDK package downloaded (install on Play)"),
            )?;
        }
        _ => {
            emit_loading(
                loading_bar,
                10.0,
                Some("Downloaded package is ready"),
            )?;
        }
    }

    Ok(())
}

async fn find_manifest_parent_dir(root: &Path) -> Option<PathBuf> {
    if !tokio::fs::try_exists(root).await.ok()? {
        return None;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let manifest = dir.join("AppxManifest.xml");
        if manifest.exists() {
            return Some(dir);
        }
        let mut entries = tokio::fs::read_dir(&dir).await.ok()?;
        while let Some(entry) = entries.next_entry().await.ok()? {
            let path = entry.path();
            if entry.file_type().await.ok()?.is_dir() {
                stack.push(path);
            }
        }
    }
    None
}

fn bump_progress(
    loading_bar: &LoadingBarId,
    progress: &mut f64,
    target: f64,
    message: &str,
) -> crate::Result<()> {
    let clamped_target = target.clamp(0.0, 100.0);
    let delta = clamped_target - *progress;
    if delta > 0.0 {
        emit_loading(loading_bar, delta, Some(message))?;
        *progress = clamped_target;
    }
    Ok(())
}

fn profile_bedrock_data_dir(profile_full_path: &Path) -> PathBuf {
    profile_full_path.join("bedrock").join("com.mojang")
}

fn profile_pending_world_imports_dir(profile_full_path: &Path) -> PathBuf {
    profile_full_path
        .join("bedrock")
        .join(BEDROCK_PENDING_WORLD_IMPORTS_DIR)
}

async fn import_pending_world_files(
    profile_full_path: &Path,
    profile_game_data: &Path,
) -> crate::Result<()> {
    let imports_dir = profile_pending_world_imports_dir(profile_full_path);
    if !tokio::fs::try_exists(&imports_dir).await.unwrap_or(false) {
        return Ok(());
    }

    let worlds_root = profile_game_data.join("minecraftWorlds");
    let _ = tokio::fs::create_dir_all(&worlds_root).await;

    let mut entries = tokio::fs::read_dir(&imports_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let is_mcworld = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("mcworld"))
            .unwrap_or(false);
        if !is_mcworld {
            continue;
        }

        let marker = path.with_extension("mcworld.imported");
        if tokio::fs::try_exists(&marker).await.unwrap_or(false) {
            continue;
        }

        let world_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if world_appears_imported(&worlds_root, &world_stem).await {
            let _ = tokio::fs::write(&marker, b"already-present").await;
            continue;
        }

        if launch_world_import_file(&path).await {
            let _ = tokio::fs::write(&marker, b"import-triggered").await;
        } else {
            tracing::warn!(
                "Failed to trigger Bedrock world import for {}",
                path.display()
            );
        }
    }

    Ok(())
}

async fn world_appears_imported(worlds_root: &Path, world_stem: &str) -> bool {
    if !tokio::fs::try_exists(worlds_root).await.unwrap_or(false) {
        return false;
    }

    let needle = world_stem.to_ascii_lowercase();
    let mut entries = match tokio::fs::read_dir(worlds_root).await {
        Ok(v) => v,
        Err(_) => return false,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name == needle {
            return true;
        }
    }
    false
}

#[cfg(windows)]
async fn launch_world_import_file(path: &Path) -> bool {
    let escaped = path.to_string_lossy().replace('\'', "''");
    let status = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!(
                "try {{ Start-Process -FilePath '{}' -ErrorAction Stop; exit 0 }} catch {{ exit 1 }}",
                escaped
            ),
        ])
        .status()
        .await;
    matches!(status, Ok(s) if s.success())
}

#[cfg(not(windows))]
async fn launch_world_import_file(_path: &Path) -> bool {
    false
}

fn current_mojang_dir() -> crate::Result<PathBuf> {
    let local_app_data = std::env::var("LOCALAPPDATA").map_err(|e| {
        crate::ErrorKind::LauncherError(format!(
            "Unable to resolve LOCALAPPDATA: {}",
            e
        ))
    })?;
    let packages_root = PathBuf::from(local_app_data).join("Packages");

    // Prefer actually existing Minecraft package directories instead of a single hardcoded PFN.
    if let Ok(entries) = std::fs::read_dir(&packages_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if !name.starts_with("microsoft.minecraft") {
                continue;
            }
            return Ok(path.join("LocalState").join("games").join("com.mojang"));
        }
    }

    Ok(packages_root
        .join("Microsoft.MinecraftUWP_8wekyb3d8bbwe")
        .join("LocalState")
        .join("games")
        .join("com.mojang"))
}

fn current_gdk_mojang_dir() -> crate::Result<Option<PathBuf>> {
    let app_data = std::env::var("APPDATA").map_err(|e| {
        crate::ErrorKind::LauncherError(format!(
            "Unable to resolve APPDATA: {}",
            e
        ))
    })?;

    let users_root = PathBuf::from(app_data)
        .join("Minecraft Bedrock")
        .join("Users");
    if !users_root.exists() {
        return Ok(None);
    }

    let mut best: Option<(SystemTime, PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(&users_root) {
        for entry in entries.flatten() {
            let user_dir = entry.path();
            if !user_dir.is_dir() {
                continue;
            }
            let candidate = user_dir.join("games").join("com.mojang");
            if !candidate.exists() {
                continue;
            }
            let modified = std::fs::metadata(&candidate)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            match &best {
                Some((best_time, _)) if *best_time >= modified => {}
                _ => best = Some((modified, candidate)),
            }
        }
    }

    Ok(best.map(|(_, p)| p))
}

fn current_system_game_data(profile: &Profile) -> crate::Result<PathBuf> {
    if is_gdk_profile(profile)
        && let Some(path) = current_gdk_mojang_dir()?
    {
        return Ok(path);
    }
    current_mojang_dir()
}

async fn initialize_profile_game_data(
    profile: &Profile,
    state: &crate::State,
    profile_full_path: &Path,
) -> crate::Result<()> {
    remove_java_profile_artifacts(profile_full_path).await?;

    let target = profile_bedrock_data_dir(profile_full_path);
    tokio::fs::create_dir_all(&target).await?;
    ensure_standard_game_layout(&target).await?;
    ensure_profile_access_permissions(&target).await?;

    if !directory_is_effectively_empty(&target).await? {
        return Ok(());
    }

    let source = current_system_game_data(profile)?;
    if tokio::fs::try_exists(&source).await.unwrap_or(false) {
        copy_dir_contents(&source, &target).await?;
        ensure_standard_game_layout(&target).await?;
    } else {
        let fallback = state
            .directories
            .metadata_dir()
            .join("bedrock")
            .join("default_profile_seed");
        if tokio::fs::try_exists(&fallback).await.unwrap_or(false) {
            copy_dir_contents(&fallback, &target).await?;
            ensure_standard_game_layout(&target).await?;
        }
    }

    Ok(())
}

async fn ensure_standard_game_layout(base: &Path) -> crate::Result<()> {
    let required_dirs = [
        "behavior_packs",
        "custom_skins",
        "development_behavior_packs",
        "development_resource_packs",
        "development_skin_packs",
        "minecraftpe",
        "minecraftWorlds",
        "resource_packs",
        "Screenshots",
        "skin_packs",
        "world_templates",
    ];

    for dir in required_dirs {
        tokio::fs::create_dir_all(base.join(dir)).await?;
    }
    Ok(())
}

async fn directory_is_effectively_empty(path: &Path) -> crate::Result<bool> {
    let mut entries = tokio::fs::read_dir(path).await?;
    Ok(entries.next_entry().await?.is_none())
}

async fn copy_dir_contents(src: &Path, dst: &Path) -> crate::Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut stack = vec![src.to_path_buf()];
    while let Some(current) = stack.pop() {
        let rel = current
            .strip_prefix(src)
            .map_err(|e| crate::ErrorKind::LauncherError(e.to_string()))?;
        tokio::fs::create_dir_all(dst.join(rel)).await?;
        let mut entries = tokio::fs::read_dir(&current).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let rel = path
                .strip_prefix(src)
                .map_err(|e| crate::ErrorKind::LauncherError(e.to_string()))?;
            let target = dst.join(rel);
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                stack.push(path);
                tokio::fs::create_dir_all(&target).await?;
            } else if file_type.is_file() {
                if let Some(parent) = target.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::copy(&path, &target).await?;
            }
        }
    }
    Ok(())
}

async fn copy_dir_contents_for_gdk(src: &Path, dst: &Path) -> crate::Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut stack = vec![src.to_path_buf()];
    while let Some(current) = stack.pop() {
        let rel = current
            .strip_prefix(src)
            .map_err(|e| crate::ErrorKind::LauncherError(e.to_string()))?;
        tokio::fs::create_dir_all(dst.join(rel)).await?;
        let mut entries = tokio::fs::read_dir(&current).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let rel = path
                .strip_prefix(src)
                .map_err(|e| crate::ErrorKind::LauncherError(e.to_string()))?;
            let target = dst.join(rel);
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                stack.push(path);
                tokio::fs::create_dir_all(&target).await?;
            } else if file_type.is_file() {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if name == "minecraft.windows.exe" {
                    continue;
                }
                if let Some(parent) = target.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                if let Err(err) = tokio::fs::copy(&path, &target).await {
                    if err.kind() == std::io::ErrorKind::PermissionDenied {
                        continue;
                    }
                    return Err(err.into());
                }
            }
        }
    }
    Ok(())
}

async fn move_or_copy_directory(src: &Path, dst: &Path) -> crate::Result<()> {
    if tokio::fs::try_exists(dst).await.unwrap_or(false) {
        remove_dir_or_symlink(dst).await?;
    }
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let same_drive = src
        .components()
        .next()
        .and_then(|c| match c {
            std::path::Component::Prefix(p) => Some(p.as_os_str().to_owned()),
            _ => None,
        })
        == dst.components().next().and_then(|c| match c {
            std::path::Component::Prefix(p) => Some(p.as_os_str().to_owned()),
            _ => None,
        });

    if same_drive && tokio::fs::rename(src, dst).await.is_ok() {
        return Ok(());
    }

    copy_dir_contents(src, dst).await?;
    let _ = tokio::fs::remove_dir_all(src).await;
    Ok(())
}

async fn remove_dir_or_symlink(path: &Path) -> crate::Result<()> {
    if !tokio::fs::try_exists(path).await.unwrap_or(false) {
        return Ok(());
    }
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if metadata.file_type().is_symlink() {
        tokio::fs::remove_dir(path).await?;
    } else {
        tokio::fs::remove_dir_all(path).await?;
    }
    Ok(())
}

async fn activate_profile_game_data(
    profile_id: &str,
    profile_data: &Path,
    system_data: &Path,
    state: &crate::State,
    allow_missing_local_state: bool,
) -> crate::Result<()> {
    ensure_profile_access_permissions(profile_data).await?;

    // Do not attempt to create package-family root directories under Local\\Packages:
    // Windows controls these ACLs and manual creation can fail with ACCESS_DENIED.
    let local_state_dir = system_data
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!(
                "Invalid Bedrock LocalState path: {}",
                system_data.display()
            ))
        })?;
    if !tokio::fs::try_exists(&local_state_dir).await.unwrap_or(false) {
        if allow_missing_local_state {
            tracing::warn!(
                "Bedrock LocalState directory missing for GDK launch, skipping LocalState sync: {}",
                local_state_dir.display()
            );
            write_active_profile_marker(state, profile_id, profile_data).await?;
            return Ok(());
        }
        return Err(crate::ErrorKind::LauncherError(
            "Bedrock LocalState directory does not exist. Install/start Minecraft once, then retry."
                .to_string(),
        )
        .as_error());
    }

    if let Some(games_dir) = system_data.parent() {
        tokio::fs::create_dir_all(games_dir).await?;
    }

    // Save data from currently active profile before switching to another one.
    // Important: do not copy system -> profile when the target profile is already active.
    // The launcher should treat profile data as source of truth on launch and sync back on exit.
    if tokio::fs::try_exists(system_data).await.unwrap_or(false) {
        if let Some(previous_profile_data) = read_active_profile_data_path(state).await? {
            if previous_profile_data != profile_data {
                tokio::fs::create_dir_all(&previous_profile_data).await?;
                mirror_copy_dir_contents(system_data, &previous_profile_data).await?;
                ensure_profile_access_permissions(&previous_profile_data).await?;
            }
        }
    }

    // Use real folder copy into LocalState instead of junction redirection.
    // UWP storage quota can break (0/0) when com.mojang is redirected outside package location.
    sync_dir_contents_without_root_replace(profile_data, system_data).await?;
    ensure_profile_access_permissions(system_data).await?;
    write_active_profile_marker(state, profile_id, profile_data).await?;
    Ok(())
}

pub async fn sync_profile_data_from_system(
    profile: &Profile,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    let profile_full_path = crate::api::profile::get_full_path(&profile.path).await?;
    let profile_data = profile_bedrock_data_dir(&profile_full_path);
    let system_data = current_system_game_data(profile)?;

    if !tokio::fs::try_exists(&system_data).await.unwrap_or(false) {
        return Ok(());
    }

    tokio::fs::create_dir_all(&profile_data).await?;
    mirror_copy_dir_contents(&system_data, &profile_data).await?;
    ensure_profile_access_permissions(&profile_data).await?;
    write_active_profile_marker(&state, &profile.path, &profile_data).await?;
    Ok(())
}

async fn mirror_copy_dir_contents(src: &Path, dst: &Path) -> crate::Result<()> {
    if tokio::fs::try_exists(dst).await.unwrap_or(false) {
        remove_dir_or_symlink(dst).await?;
    }
    tokio::fs::create_dir_all(dst).await?;
    copy_dir_contents(src, dst).await?;
    Ok(())
}

async fn sync_dir_contents_without_root_replace(
    src: &Path,
    dst: &Path,
) -> crate::Result<()> {
    tokio::fs::create_dir_all(dst).await?;
    purge_bedrock_runtime_content_dirs(dst).await?;

    let mut stack = vec![src.to_path_buf()];
    while let Some(current) = stack.pop() {
        let rel = current
            .strip_prefix(src)
            .map_err(|e| crate::ErrorKind::LauncherError(e.to_string()))?;
        let dst_dir = dst.join(rel);
        tokio::fs::create_dir_all(&dst_dir).await?;

        let mut entries = tokio::fs::read_dir(&current).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let rel = path
                .strip_prefix(src)
                .map_err(|e| crate::ErrorKind::LauncherError(e.to_string()))?;
            let target = dst.join(rel);
            let file_type = entry.file_type().await?;

            if file_type.is_dir() {
                stack.push(path);
                tokio::fs::create_dir_all(&target).await?;
            } else if file_type.is_file() {
                if let Some(parent) = target.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                if tokio::fs::try_exists(&target).await.unwrap_or(false) {
                    let _ = tokio::fs::remove_file(&target).await;
                }
                tokio::fs::copy(&path, &target).await?;
            }
        }
    }
    Ok(())
}

async fn purge_bedrock_runtime_content_dirs(dst: &Path) -> crate::Result<()> {
    // Keep LocalState root, but clear runtime content dirs so profiles do not leak into each other.
    let runtime_dirs = [
        "behavior_packs",
        "custom_skins",
        "development_behavior_packs",
        "development_resource_packs",
        "development_skin_packs",
        "minecraftWorlds",
        "resource_packs",
        "Screenshots",
        "skin_packs",
        "world_templates",
    ];
    for rel in runtime_dirs {
        let path = dst.join(rel);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let _ = remove_dir_or_symlink(&path).await;
        }
    }
    Ok(())
}

fn active_profile_marker_path(state: &crate::State) -> PathBuf {
    state
        .directories
        .metadata_dir()
        .join("bedrock")
        .join("active_profile.txt")
}

async fn read_active_profile_data_path(
    state: &crate::State,
) -> crate::Result<Option<PathBuf>> {
    let marker = active_profile_marker_path(state);
    if !tokio::fs::try_exists(&marker).await.unwrap_or(false) {
        return Ok(None);
    }
    let raw = tokio::fs::read_to_string(&marker).await?;
    let data_path = raw
        .lines()
        .nth(1)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if data_path.is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(data_path)))
}

async fn write_active_profile_marker(
    state: &crate::State,
    profile_id: &str,
    profile_data: &Path,
) -> crate::Result<()> {
    let marker = active_profile_marker_path(state);
    if let Some(parent) = marker.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = format!(
        "{}\n{}",
        profile_id.trim(),
        profile_data.to_string_lossy()
    );
    tokio::fs::write(marker, content).await?;
    Ok(())
}

#[derive(Clone, Debug)]
struct InstalledBedrockInfo {
    version: String,
}

async fn get_installed_bedrock_info() -> crate::Result<Option<InstalledBedrockInfo>> {
    #[cfg(windows)]
    {
        let script = "$ErrorActionPreference='SilentlyContinue'; Get-AppxPackage -Name 'Microsoft.Minecraft*' | Sort-Object -Property Version -Descending | Select-Object -First 1 | ForEach-Object { \"$($_.Version)|$($_.PackageFullName)\" }";
        let output = hidden_powershell_command()
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .output()
            .await?;
        if !output.status.success() {
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let line = text.lines().next().unwrap_or_default().trim();
        if line.is_empty() {
            return Ok(None);
        }
        let mut parts = line.splitn(2, '|');
        let version = parts.next().unwrap_or_default().trim().to_string();
        if version.is_empty() {
            return Ok(None);
        }
        return Ok(Some(InstalledBedrockInfo { version }));
    }
    #[cfg(not(windows))]
    {
        Ok(None)
    }
}

fn desired_version_prefix(game_version: &str) -> Option<String> {
    BEDROCK_VERSION_RE
        .captures(game_version)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

fn installed_matches_desired(installed: &str, desired_prefix: &str) -> bool {
    installed == desired_prefix
        || installed.starts_with(&format!("{desired_prefix}."))
        || installed.starts_with(desired_prefix)
}

#[cfg(windows)]
async fn remove_installed_bedrock_packages() -> crate::Result<()> {
    let script = "$ErrorActionPreference='SilentlyContinue'; \
        Get-AppxPackage -AllUsers -Name 'Microsoft.Minecraft*' | ForEach-Object { \
            try { Remove-AppxPackage -Package $_.PackageFullName -AllUsers -ErrorAction Stop } catch {} \
        }; \
        Get-AppxPackage -Name 'Microsoft.Minecraft*' | ForEach-Object { \
            try { Remove-AppxPackage -Package $_.PackageFullName -ErrorAction Stop } catch {} \
        }; \
        Start-Sleep -Milliseconds 600";
    let status = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .status()
        .await?;
    if !status.success() {
        return Err(crate::ErrorKind::LauncherError(
            "Failed to remove currently installed Bedrock package".to_string(),
        )
        .as_error());
    }
    Ok(())
}

#[cfg(not(windows))]
async fn remove_installed_bedrock_packages() -> crate::Result<()> {
    Err(crate::ErrorKind::LauncherError(
        "Bedrock package removal is only supported on Windows".to_string(),
    )
    .as_error())
}

fn resolve_selected_package_path(
    profile: &Profile,
    state: &crate::State,
) -> Option<PathBuf> {
    let raw = profile.loader_version.as_ref()?.trim();
    let name = resolve_loader_version_filename(raw)?;
    if name.is_empty() {
        return None;
    }
    Some(
        state
            .directories
            .metadata_dir()
            .join("bedrock")
            .join("all_versions")
            .join(name),
    )
}

fn resolve_loader_version_filename(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    // New format: file name
    if !raw.contains("://") && !raw.contains('/') && !raw.contains('\\') {
        return Some(raw.to_string());
    }

    // Legacy format from UI: "<url>_0"
    let url_part = raw.rsplit_once('_').map(|(left, _)| left).unwrap_or(raw);
    let file = filename_from_url(url_part)?;
    Some(file)
}

async fn ensure_required_bedrock_version(
    profile: &Profile,
    state: &crate::State,
    loading_bar: &LoadingBarId,
    progress: &mut f64,
) -> crate::Result<()> {
    if is_gdk_profile(profile) {
        let _ =
            ensure_gdk_local_install_ready(profile, state, loading_bar, progress)
                .await?;
        bump_progress(
            loading_bar,
            progress,
            70.0,
            "GDK package prepared",
        )?;
        return Ok(());
    }

    let desired = desired_version_for_profile(profile, state)
        .await
        .or_else(|| desired_version_prefix(&profile.game_version));
    let installed = get_installed_bedrock_info().await?;
    let needs_install = match (&desired, &installed) {
        (Some(desired), Some(installed)) => {
            !installed_matches_desired(&installed.version, desired)
        }
        (Some(_), None) => true,
        (None, _) => false,
    };

    if !needs_install {
        if let Some(info) = installed {
            bump_progress(
                loading_bar,
                progress,
                30.0,
                &format!("Installed Bedrock matches: {}", info.version),
            )?;
        } else {
            bump_progress(
                loading_bar,
                progress,
                30.0,
                "Bedrock package check complete",
            )?;
        }
        return Ok(());
    }

    let package_path = resolve_selected_package_path(profile, state).ok_or_else(|| {
        crate::ErrorKind::LauncherError(
            "Selected Bedrock package is not available locally. Create/install the profile first."
                .to_string(),
        )
    })?;
    if !tokio::fs::try_exists(&package_path).await.unwrap_or(false) {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Selected Bedrock package file is missing: {}",
            package_path.display()
        ))
        .as_error());
    }

    bump_progress(
        loading_bar,
        progress,
        45.0,
        "Bedrock version mismatch detected, reinstalling",
    )?;
    remove_installed_bedrock_packages().await?;
    bump_progress(
        loading_bar,
        progress,
        62.0,
        "Installing required Bedrock package",
    )?;
    install_downloaded_bedrock(&package_path, state, loading_bar).await?;
    bump_progress(
        loading_bar,
        progress,
        70.0,
        "Bedrock package installed",
    )?;
    Ok(())
}

async fn desired_version_for_profile(
    profile: &Profile,
    state: &crate::State,
) -> Option<String> {
    let package_path = resolve_selected_package_path(profile, state)?;
    if !tokio::fs::try_exists(&package_path).await.ok()? {
        return None;
    }

    // For APPX/MSIX, prefer exact package manifest version (eg 1.12.28.0)
    // over short UI version (eg 1.12.0.appx), to avoid reinstall loops.
    let ext = package_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if ext == "appx" || ext == "msix" || ext == "appxbundle" || ext == "msixbundle" {
        if let Some(v) = read_package_manifest_identity_version(&package_path).await {
            return Some(v);
        }
    }

    desired_version_prefix(&profile.game_version)
}

async fn read_package_manifest_identity_version(package_path: &Path) -> Option<String> {
    let package_path = package_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&package_path).ok()?;
        let mut archive = ZipArchive::new(file).ok()?;

        let candidates = ["AppxManifest.xml", "appxmanifest.xml"];
        for name in candidates {
            if let Ok(mut manifest) = archive.by_name(name) {
                let mut content = String::new();
                if manifest.read_to_string(&mut content).is_ok() {
                    if let Some(caps) = APPX_IDENTITY_VERSION_RE.captures(&content) {
                        if let Some(v) = caps.get(1) {
                            return Some(v.as_str().to_string());
                        }
                    }
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

fn gdk_profile_install_dir(profile: &Profile, state: &crate::State) -> PathBuf {
    let version = desired_version_prefix(&profile.game_version).unwrap_or_else(|| {
        profile
            .game_version
            .replace(".msixvc", "")
            .replace(".MSIXVC", "")
            .trim()
            .to_string()
    });
    state
        .directories
        .metadata_dir()
        .join("bedrock")
        .join("gdk_versions")
        .join(version)
}

async fn ensure_gdk_local_install_ready(
    profile: &Profile,
    state: &crate::State,
    loading_bar: &LoadingBarId,
    progress: &mut f64,
) -> crate::Result<Option<PathBuf>> {
    if !is_gdk_profile(profile) {
        return Ok(None);
    }

    let package_path = resolve_selected_package_path(profile, state).ok_or_else(|| {
        crate::ErrorKind::LauncherError(
            "Selected GDK package is not available locally".to_string(),
        )
    })?;
    if !tokio::fs::try_exists(&package_path).await.unwrap_or(false) {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Selected GDK package file is missing: {}",
            package_path.display()
        ))
        .as_error());
    }

    let install_dir = gdk_profile_install_dir(profile, state);
    let exe_path = install_dir.join("Minecraft.Windows.exe");
    let desired = desired_version_prefix(&profile.game_version);
    if tokio::fs::try_exists(&exe_path).await.unwrap_or(false) {
        if gdk_local_install_matches_desired(&install_dir, desired.as_deref()).await
        {
            bump_progress(
                loading_bar,
                progress,
                68.0,
                "GDK local install is ready",
            )?;
            return Ok(Some(exe_path));
        }

        bump_progress(
            loading_bar,
            progress,
            60.0,
            "GDK local install version mismatch, reinstalling selected version",
        )?;
        let _ = tokio::fs::remove_dir_all(&install_dir).await;
    }

    run_gdk_helper_install(
        &package_path,
        &install_dir,
        loading_bar,
        progress,
    )
    .await?;
    if !tokio::fs::try_exists(&exe_path).await.unwrap_or(false) {
        return Err(crate::ErrorKind::LauncherError(format!(
            "GDK helper finished but Minecraft.Windows.exe is missing: {}",
            exe_path.display()
        ))
        .as_error());
    }
    bump_progress(
        loading_bar,
        progress,
        70.0,
        "GDK package installed by helper",
    )?;
    Ok(Some(exe_path))
}

async fn gdk_local_install_matches_desired(
    install_dir: &Path,
    desired_prefix: Option<&str>,
) -> bool {
    let Some(desired) = desired_prefix else {
        return true;
    };

    let manifest_path = install_dir.join("AppxManifest.xml");
    let Ok(manifest_exists) = tokio::fs::try_exists(&manifest_path).await else {
        return false;
    };
    if !manifest_exists {
        return false;
    }
    let Ok(content) = tokio::fs::read_to_string(&manifest_path).await else {
        return false;
    };

    let Some(caps) = APPX_IDENTITY_VERSION_RE.captures(&content) else {
        return false;
    };
    let Some(found) = caps.get(1) else {
        return false;
    };
    installed_matches_desired(found.as_str(), desired)
}

fn gdk_helper_project_path() -> PathBuf {
    let app_lib_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    app_lib_dir.join("..").join("..").join("tools").join("gdk-helper").join("GdkHelper.csproj")
}

enum GdkHelperLaunch {
    Exe(PathBuf),
    DotnetDll(PathBuf),
    DotnetProject(PathBuf),
}

fn find_gdk_helper_launch() -> GdkHelperLaunch {
    if let Ok(env_path) = std::env::var("BEDROCK_GDK_HELPER_EXE") {
        let path = PathBuf::from(env_path);
        if path.exists() {
            return GdkHelperLaunch::Exe(path);
        }
    }

    let project_dir = gdk_helper_project_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let dll_candidates = [
        project_dir
            .join("bin")
            .join("Release")
            .join("net8.0-windows10.0.19041.0")
            .join("GdkHelper.dll"),
        project_dir
            .join("bin")
            .join("Debug")
            .join("net8.0-windows10.0.19041.0")
            .join("GdkHelper.dll"),
    ];
    if let Some(dll) = dll_candidates.into_iter().find(|p| p.exists()) {
        return GdkHelperLaunch::DotnetDll(dll);
    }

    let exe_candidates = [
        project_dir
            .join("bin")
            .join("Release")
            .join("net8.0-windows10.0.19041.0")
            .join("win-x64")
            .join("publish")
            .join("GdkHelper.exe"),
        project_dir
            .join("bin")
            .join("Release")
            .join("net8.0-windows10.0.19041.0")
            .join("GdkHelper.exe"),
        project_dir
            .join("bin")
            .join("Debug")
            .join("net8.0-windows10.0.19041.0")
            .join("GdkHelper.exe"),
    ];
    if let Some(exe) = exe_candidates.into_iter().find(|p| p.exists()) {
        return GdkHelperLaunch::Exe(exe);
    }

    GdkHelperLaunch::DotnetProject(gdk_helper_project_path())
}

async fn run_gdk_helper_install(
    package_path: &Path,
    install_dir: &Path,
    loading_bar: &LoadingBarId,
    progress: &mut f64,
) -> crate::Result<()> {
    match run_gdk_helper_install_once(
        package_path,
        install_dir,
        loading_bar,
        progress,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("helper produced no stdout/stderr diagnostics") {
                bump_progress(
                    loading_bar,
                    progress,
                    53.0,
                    "Retrying GDK helper after silent failure",
                )?;
                run_gdk_helper_install_once(
                    package_path,
                    install_dir,
                    loading_bar,
                    progress,
                )
                .await
            } else {
                Err(err)
            }
        }
    }
}

async fn run_gdk_helper_install_once(
    package_path: &Path,
    install_dir: &Path,
    loading_bar: &LoadingBarId,
    progress: &mut f64,
) -> crate::Result<()> {
    bump_progress(
        loading_bar,
        progress,
        52.0,
        "Starting GDK helper",
    )?;

    let launch = find_gdk_helper_launch();
    let launch_desc: String;
    let mut command = match launch {
        GdkHelperLaunch::Exe(exe) => {
            launch_desc = format!("exe:{}", exe.display());
            let mut c = tokio::process::Command::new(exe);
            c.arg("install-gdk");
            c
        }
        GdkHelperLaunch::DotnetDll(dll) => {
            launch_desc = format!("dotnet-dll:{}", dll.display());
            let mut c = tokio::process::Command::new("dotnet");
            c.arg(dll).arg("install-gdk");
            c
        }
        GdkHelperLaunch::DotnetProject(project) => {
            launch_desc = format!("dotnet-project:{}", project.display());
            let mut c = tokio::process::Command::new("dotnet");
            c.arg("run")
                .arg("--project")
                .arg(project)
                .arg("--")
                .arg("install-gdk");
            c
        }
    };
    command
        .arg("--msixvc")
        .arg(package_path)
        .arg("--output")
        .arg(install_dir);
    let helper_log_path = std::env::temp_dir().join(format!(
        "modrinth-gdk-helper-{}.log",
        Uuid::new_v4()
    ));
    command.env("BEDROCK_GDK_HELPER_LOG", &helper_log_path);

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| {
        crate::ErrorKind::LauncherError(format!(
            "Failed to start gdk helper: {}",
            e
        ))
    })?;

    let stdout = child.stdout.take().ok_or_else(|| {
        crate::ErrorKind::LauncherError("gdk helper stdout not piped".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        crate::ErrorKind::LauncherError("gdk helper stderr not piped".to_string())
    })?;

    let stderr_task = tokio::spawn(async move {
        let mut err_reader = BufReader::new(stderr).lines();
        let mut lines = Vec::new();
        while let Ok(Some(line)) = err_reader.next_line().await {
            lines.push(line);
        }
        lines
    });

    let mut out_reader = BufReader::new(stdout).lines();
    let mut helper_result_line: Option<String> = None;
    let mut stdout_lines = Vec::new();
    let mut helper_error_lines = Vec::new();
    while let Ok(Some(line)) = out_reader.next_line().await {
        stdout_lines.push(line.clone());
        if let Some(payload) = line.strip_prefix("PROGRESS|") {
            let mut parts = payload.splitn(2, '|');
            let pct = parts
                .next()
                .and_then(|v| v.trim().parse::<f64>().ok())
                .unwrap_or(0.0)
                .clamp(0.0, 100.0);
            let msg = parts.next().unwrap_or("Installing GDK package");
            let target = 52.0 + (pct * 0.16);
            let _ = bump_progress(loading_bar, progress, target, msg);
        } else if line.starts_with("RESULT|") {
            helper_result_line = Some(line);
        } else if let Some(msg) = line.strip_prefix("ERROR|") {
            helper_error_lines.push(msg.trim().to_string());
        }
    }

    let status = child.wait().await.map_err(|e| {
        crate::ErrorKind::LauncherError(format!("Failed waiting for gdk helper: {}", e))
    })?;
    let stderr_lines = stderr_task.await.unwrap_or_default();

    if !status.success() {
        let mut details = Vec::new();
        let exit_code = status
            .code()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string());
        details.push(format!("exit_code={}", exit_code));
        details.push(format!("launch={}", launch_desc));

        if !helper_error_lines.is_empty() {
            details.push(format!(
                "helper_errors={}",
                helper_error_lines.join(" | ")
            ));
        }
        if !stderr_lines.is_empty() {
            details.push(format!("stderr={}", stderr_lines.join(" | ")));
        }
        if !stdout_lines.is_empty() {
            let non_progress: Vec<String> = stdout_lines
                .into_iter()
                .filter(|l| !l.starts_with("PROGRESS|"))
                .collect();
            if !non_progress.is_empty() {
                details.push(format!("stdout={}", non_progress.join(" | ")));
            }
        }
        if details.len() == 2 {
            details.push(
                "helper produced no stdout/stderr diagnostics".to_string(),
            );
        }
        if let Ok(log_text) = tokio::fs::read_to_string(&helper_log_path).await {
            if !log_text.trim().is_empty() {
                details.push(format!(
                    "helper_log={}",
                    log_text.replace('\n', " | ").replace('\r', "")
                ));
            }
        }
        return Err(crate::ErrorKind::LauncherError(format!(
            "GDK helper failed: {}",
            details.join("; ")
        ))
        .as_error());
    }

    if helper_result_line.is_none() {
        bump_progress(
            loading_bar,
            progress,
            68.0,
            "GDK helper completed",
        )?;
    }

    Ok(())
}

async fn find_xboxgames_minecraft_content_dir() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from(
        r"C:\XboxGames\Minecraft for Windows\Content",
    )];
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        candidates.push(
            PathBuf::from(program_files)
                .join("ModifiableWindowsApps")
                .join("Minecraft for Windows")
                .join("Content"),
        );
    }

    for dir in candidates {
        let exe = dir.join("Minecraft.Windows.exe");
        if tokio::fs::try_exists(&exe).await.unwrap_or(false) {
            return Some(dir);
        }
    }
    None
}

async fn resolve_package_family_for_install_location(
    install_location: &Path,
) -> crate::Result<Option<String>> {
    let loc = install_location.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         $loc = '{}'; \
         Get-AppxPackage -Name 'Microsoft.Minecraft*' | ForEach-Object {{ \
           if ($_.InstallLocation -and ([string]::Equals($_.InstallLocation, $loc, [System.StringComparison]::OrdinalIgnoreCase))) {{ \
             $_.PackageFamilyName \
           }} \
         }} | Select-Object -First 1",
        loc
    );
    let output = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
        .await?;
    if !output.status.success() {
        return Ok(None);
    }
    let pfn = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if pfn.is_empty() {
        return Ok(None);
    }
    Ok(Some(pfn))
}

async fn copy_minecraft_exe_via_desktop_package(
    package_family: &str,
    staged_exe: &Path,
    target_exe: &Path,
) -> crate::Result<()> {
    let tmp_dir = std::env::temp_dir().join("modrinth-bedrock-exe");
    tokio::fs::create_dir_all(&tmp_dir).await?;
    let tmp_exe = tmp_dir.join(format!("Minecraft.Windows.{}.exe", Uuid::new_v4()));
    let tmp_partial = tmp_exe.with_extension("exe.tmp");

    let pf = package_family.replace('\'', "''");
    let src = staged_exe.to_string_lossy().replace('\'', "''");
    let tmp = tmp_exe.to_string_lossy().replace('\'', "''");
    let tmp_p = tmp_partial.to_string_lossy().replace('\'', "''");
    let command = format!(
        "Invoke-CommandInDesktopPackage -PackageFamilyName '{pf}' -App Game -Command 'powershell.exe' -Args \"-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command Copy-Item -LiteralPath ''{src}'' -Destination ''{tmp_p}'' -Force; Move-Item -LiteralPath ''{tmp_p}'' -Destination ''{tmp}'' -Force\""
    );

    let output = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &command,
        ])
        .output()
        .await?;
    if !output.status.success() {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Invoke-CommandInDesktopPackage failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
        .as_error());
    }

    for _ in 0..300u16 {
        if tokio::fs::try_exists(&tmp_exe).await.unwrap_or(false) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !tokio::fs::try_exists(&tmp_exe).await.unwrap_or(false) {
        return Err(crate::ErrorKind::LauncherError(
            "Desktop-package copy did not produce Minecraft.Windows.exe"
                .to_string(),
        )
        .as_error());
    }

    if let Some(parent) = target_exe.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if tokio::fs::try_exists(target_exe).await.unwrap_or(false) {
        let _ = tokio::fs::remove_file(target_exe).await;
    }
    tokio::fs::rename(&tmp_exe, target_exe).await?;
    Ok(())
}

async fn maybe_install_on_create(
    package_path: &Path,
    state: &crate::State,
    loading_bar: &LoadingBarId,
    progress: &mut f64,
) -> crate::Result<()> {
    let should_install_now = std::env::var("BEDROCK_INSTALL_ON_CREATE")
        .ok()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes" || v == "on"
        })
        .unwrap_or(false);

    if should_install_now {
        bump_progress(
            loading_bar,
            progress,
            85.0,
            "Installing Bedrock package",
        )?;
        install_downloaded_bedrock(package_path, state, loading_bar).await?;
    } else {
        bump_progress(
            loading_bar,
            progress,
            85.0,
            "Bedrock package downloaded (install on Play)",
        )?;
    }
    Ok(())
}

async fn remove_java_profile_artifacts(profile_full_path: &Path) -> crate::Result<()> {
    let java_dirs = [
        "mods",
        "config",
        "logs",
        "crash-reports",
        "resourcepacks",
        "shaderpacks",
        "saves",
        "datapacks",
    ];

    for dir in java_dirs {
        let path = profile_full_path.join(dir);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let _ = tokio::fs::remove_dir_all(&path).await;
        }
    }

    for file in ["options.txt", "launcher_log.txt"] {
        let path = profile_full_path.join(file);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }
    Ok(())
}

async fn ensure_profile_access_permissions(profile_data: &Path) -> crate::Result<()> {
    #[cfg(windows)]
    {
        let path = profile_data.to_string_lossy().replace('\'', "''");
        let script = format!(
            "$ErrorActionPreference='SilentlyContinue'; \
             icacls '{}' /grant *S-1-5-11:(OI)(CI)F /T /C | Out-Null; \
             icacls '{}' /grant *S-1-15-2-1:(OI)(CI)F /T /C | Out-Null; \
             icacls '{}' /grant *S-1-15-2-2:(OI)(CI)F /T /C | Out-Null",
            path,
            path,
            path
        );
        let _ = hidden_powershell_command()
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .status()
            .await;
    }
    Ok(())
}

async fn is_already_redirected_to_profile(
    system_data: &Path,
    profile_data: &Path,
) -> bool {
    let Ok(exists) = tokio::fs::try_exists(system_data).await else {
        return false;
    };
    if !exists {
        return false;
    }

    let system_abs = std::fs::canonicalize(system_data).ok();
    let profile_abs = std::fs::canonicalize(profile_data).ok();

    match (system_abs, profile_abs) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

#[cfg(windows)]
async fn create_windows_link_or_junction(
    target: &Path,
    link: &Path,
) -> crate::Result<()> {
    if is_already_redirected_to_profile(link, target).await {
        return Ok(());
    }

    if tokio::fs::try_exists(link).await.unwrap_or(false) {
        remove_dir_or_symlink(link).await?;
    }
    if let Some(parent) = link.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let link_s = link.to_string_lossy().to_string();
    let target_s = target.to_string_lossy().to_string();
    let mklink_cmd = format!("mklink /J \"{}\" \"{}\"", link_s, target_s);
    let output = tokio::process::Command::new("cmd")
        .args(["/C", &mklink_cmd])
        .output()
        .await?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(crate::ErrorKind::LauncherError(format!(
            "Failed to create bedrock junction. stdout='{}' stderr='{}'",
            stdout.trim(),
            stderr.trim()
        ))
        .as_error())
    }
}

#[cfg(not(windows))]
async fn create_windows_link_or_junction(
    _target: &Path,
    _link: &Path,
) -> crate::Result<()> {
    Err(crate::ErrorKind::LauncherError(
        "Bedrock link creation is only supported on Windows".to_string(),
    )
    .as_error())
}

#[cfg(windows)]
async fn find_com_mojang_link_target() -> crate::Result<Option<String>> {
    let system_data = current_mojang_dir()?;
    let path = system_data.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         if (Test-Path -LiteralPath '{}') {{ \
             $i = Get-Item -LiteralPath '{}'; \
             if ($i.LinkType -or $i.Target) {{ $i.Target }} else {{ '' }} \
         }}",
        path, path
    );
    let output = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
        .await?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

#[cfg(not(windows))]
async fn find_com_mojang_link_target() -> crate::Result<Option<String>> {
    Ok(None)
}

async fn verify_bedrock_redirection(profile_data: &Path) -> crate::Result<()> {
    #[cfg(windows)]
    {
        if let Some(target) = find_com_mojang_link_target().await? {
            let profile = std::fs::canonicalize(profile_data)
                .unwrap_or_else(|_| profile_data.to_path_buf())
                .to_string_lossy()
                .to_string();
            let target_norm = target.to_ascii_lowercase().replace('/', "\\");
            let profile_norm = profile.to_ascii_lowercase().replace('/', "\\");
            if !target_norm.contains(&profile_norm) {
                return Err(crate::ErrorKind::LauncherError(format!(
                    "Bedrock redirection mismatch: com.mojang points to '{}' instead of '{}'",
                    target, profile
                ))
                .as_error());
            }
        }
    }
    Ok(())
}
