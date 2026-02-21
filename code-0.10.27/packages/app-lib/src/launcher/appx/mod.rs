pub mod register;
pub mod remove;
pub mod utils;

#[cfg(windows)]
use regex::Regex;
#[cfg(windows)]
use std::cmp::Ordering;
#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use std::sync::LazyLock;
#[cfg(windows)]
use windows::core::{HRESULT, HSTRING};
#[cfg(windows)]
use windows::Foundation::Uri;
#[cfg(windows)]
use windows::Management::Deployment::PackageManager;
#[cfg(windows)]
use uuid::Uuid;

#[cfg(windows)]
static APPX_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<a\s+href=\"(?P<href>[^\"]+)\"[^>]*>(?P<name>[^<]+)</a>"#)
        .expect("appx link regex")
});
#[cfg(windows)]
static VER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+\.\d+\.\d+\.\d+)").expect("version regex")
});

#[cfg(windows)]
fn hidden_powershell_command() -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.creation_flags(0x08000000);
    cmd.arg("-WindowStyle").arg("Hidden");
    cmd
}

pub async fn install_appx_package(path: &std::path::Path) -> crate::Result<()> {
    #[cfg(windows)]
    {
        ensure_uwp_dependencies().await?;
        let path_str = path.to_string_lossy().replace('\'', "''");
        let output = add_appx_package_cmd(&path_str).await?;

        if !output.status.success() {
            let stderr_first = String::from_utf8_lossy(&output.stderr).to_string();
            let mut retried_with_cleanup = false;

            if should_retry_minecraft_after_cleanup(&stderr_first) {
                retried_with_cleanup = true;
                tracing::warn!(
                    "Add-AppxPackage failed with likely Minecraft package conflict; trying cleanup + retry: {}",
                    stderr_first.trim()
                );
                let _ = cleanup_minecraft_package_conflicts().await;
            }

            ensure_uwp_dependencies().await?;
            let retry = add_appx_package_cmd(&path_str).await?;
            if !retry.status.success() {
                let stderr_retry = String::from_utf8_lossy(&retry.stderr);
                let extra = if retried_with_cleanup {
                    " (cleanup+retry attempted)"
                } else {
                    ""
                };
                return Err(crate::ErrorKind::LauncherError(format!(
                    "APPX install failed{}. Add-AppxPackage: {}",
                    extra,
                    stderr_retry.trim()
                ))
                .as_error());
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(crate::ErrorKind::LauncherError(
            "APPX install is only available on Windows".to_string(),
        )
        .as_error())
    }
}

#[cfg(windows)]
fn should_retry_minecraft_after_cleanup(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("0x80073d06")
        || lower.contains("0x80073cfb")
        || lower.contains("microsoft.minecraft")
}

#[cfg(windows)]
async fn cleanup_minecraft_package_conflicts() -> crate::Result<()> {
    let script = "$ErrorActionPreference='SilentlyContinue'; \
        Get-AppxPackage -AllUsers -Name 'Microsoft.Minecraft*' | ForEach-Object { \
            try { Remove-AppxPackage -Package $_.PackageFullName -AllUsers -ErrorAction Stop } catch {} \
        }; \
        Get-AppxPackage -Name 'Microsoft.Minecraft*' | ForEach-Object { \
            try { Remove-AppxPackage -Package $_.PackageFullName -ErrorAction Stop } catch {} \
        }; \
        Start-Sleep -Milliseconds 600";

    let _ = hidden_powershell_command()
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

    Ok(())
}

pub async fn install_msixvc_package(path: &std::path::Path) -> crate::Result<()> {
    #[cfg(windows)]
    {
        let path_str = path.to_string_lossy().replace('\'', "''");
        let output = hidden_powershell_command()
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(format!(
                "Add-AppxPackage -Path '{}' -ForceApplicationShutdown",
                path_str
            ))
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(crate::ErrorKind::LauncherError(format!(
                "MSIXVC install failed. stdout='{}' stderr='{}'",
                stdout.trim(),
                stderr.trim()
            ))
            .as_error());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(crate::ErrorKind::LauncherError(
            "MSIXVC install is only available on Windows".to_string(),
        )
        .as_error())
    }
}

pub async fn stage_msixvc_package(path: &std::path::Path) -> crate::Result<()> {
    #[cfg(windows)]
    {
        if !path.exists() {
            return Err(crate::ErrorKind::LauncherError(format!(
                "MSIXVC file does not exist: {}",
                path.display()
            ))
            .as_error());
        }
        let canonical = std::fs::canonicalize(path)?;
        let mut normalized = canonical.to_string_lossy().to_string();
        if normalized.starts_with(r"\\?\") {
            normalized = normalized[4..].to_string();
        }
        let uri_str = format!("file:///{}", normalized.replace('\\', "/"));

        let pm = PackageManager::new().map_err(|e| {
            crate::ErrorKind::LauncherError(format!(
                "Unable to create PackageManager: {}",
                e
            ))
        })?;
        let uri = Uri::CreateUri(&HSTRING::from(uri_str)).map_err(|e| {
            crate::ErrorKind::LauncherError(format!(
                "Unable to create file URI for MSIXVC: {}",
                e
            ))
        })?;

        let async_op = pm.StagePackageAsync(&uri, None).map_err(|e| {
            crate::ErrorKind::LauncherError(format!(
                "StagePackageAsync failed to start: {}",
                e
            ))
        })?;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1800),
            tokio::task::spawn_blocking(move || {
                async_op.get().map_err(|e| {
                    crate::ErrorKind::LauncherError(format!(
                        "StagePackageAsync failed: {}",
                        e
                    ))
                    .as_error()
                })
            }),
        )
        .await
        .map_err(|_| {
            crate::ErrorKind::LauncherError(
                "MSIXVC staging timed out after 30 minutes".to_string(),
            )
            .as_error()
        })?
        .map_err(|e| {
            crate::ErrorKind::LauncherError(format!(
                "MSIXVC staging worker failed: {}",
                e
            ))
            .as_error()
        })??;

        let extended = result.ExtendedErrorCode().unwrap_or(HRESULT(0));
        if extended != HRESULT(0) {
            let error_text = result
                .ErrorText()
                .map(|h| h.to_string_lossy())
                .unwrap_or_else(|_| String::new());
            return Err(crate::ErrorKind::LauncherError(format!(
                "MSIXVC staging failed: {:?} {}",
                extended, error_text
            ))
            .as_error());
        }

        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(crate::ErrorKind::LauncherError(
            "MSIXVC staging is only available on Windows".to_string(),
        )
        .as_error())
    }
}

pub async fn find_latest_staged_minecraft_location(
) -> crate::Result<Option<std::path::PathBuf>> {
    #[cfg(windows)]
    {
        let script = "$ErrorActionPreference='SilentlyContinue'; \
            $candidates = New-Object System.Collections.Generic.List[string]; \
            # Prefer staged/modifiable roots first (mc-w10-style), avoid protected WindowsApps picks. \
            $roots = @('C:\\XboxGames', \"$env:ProgramFiles\\ModifiableWindowsApps\"); \
            if (Test-Path 'C:\\XboxGames\\Minecraft for Windows\\Content\\Minecraft.Windows.exe') { \
              $candidates.Add('C:\\XboxGames\\Minecraft for Windows\\Content') \
            }; \
            foreach ($r in $roots) { \
              if (-not [string]::IsNullOrWhiteSpace($r) -and (Test-Path $r)) { \
                try { \
                  Get-ChildItem -LiteralPath $r -Directory -Recurse -ErrorAction SilentlyContinue | ForEach-Object { \
                    if (Test-Path (Join-Path $_.FullName 'Minecraft.Windows.exe')) { $candidates.Add($_.FullName) } \
                  } \
                } catch {} \
              } \
            }; \
            Get-AppxPackage -Name 'Microsoft.Minecraft*' | Sort-Object Version -Descending | ForEach-Object { if ($_.InstallLocation -and ($_.InstallLocation -notmatch '\\\\WindowsApps(\\\\|$)')) { $candidates.Add($_.InstallLocation) } }; \
            Get-AppxPackage -AllUsers -Name 'Microsoft.Minecraft*' | Sort-Object Version -Descending | ForEach-Object { if ($_.InstallLocation -and ($_.InstallLocation -notmatch '\\\\WindowsApps(\\\\|$)')) { $candidates.Add($_.InstallLocation) } }; \
            $seen = New-Object System.Collections.Generic.HashSet[string]([System.StringComparer]::OrdinalIgnoreCase); \
            foreach ($p in $candidates) { \
              if (-not [string]::IsNullOrWhiteSpace($p) -and $seen.Add($p)) { \
                $exe = Join-Path $p 'Minecraft.Windows.exe'; \
                if (-not (Test-Path $exe)) { continue }; \
                try { Get-ChildItem -LiteralPath $p -ErrorAction Stop | Out-Null } catch { continue }; \
                $p; break \
              } \
            }";
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
        let path = text.lines().next().unwrap_or_default().trim();
        if path.is_empty() {
            return Ok(None);
        }
        Ok(Some(std::path::PathBuf::from(path)))
    }
    #[cfg(not(windows))]
    {
        Ok(None)
    }
}

#[cfg(windows)]
async fn add_appx_package_cmd(path_str: &str) -> crate::Result<std::process::Output> {
    let output = hidden_powershell_command()
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(format!(
            "Add-AppxPackage -Path '{}' -ForceApplicationShutdown",
            path_str
        ))
        .output()
        .await?;
    Ok(output)
}

#[cfg(windows)]
async fn expand_archive(input: &Path, output: &Path) -> crate::Result<()> {
    let archive_for_expand = if input
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
    {
        input.to_path_buf()
    } else {
        let temp_zip = std::env::temp_dir().join(format!(
            "modrinth-bedrock-expand-{}.zip",
            Uuid::new_v4()
        ));
        tokio::fs::copy(input, &temp_zip).await?;
        temp_zip
    };

    let input_str = archive_for_expand
        .to_string_lossy()
        .replace('\'', "''");
    let output_str = output.to_string_lossy().replace('\'', "''");

    let script = format!(
        "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
        input_str, output_str
    );
    let status = hidden_powershell_command()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .status()
        .await?;

    if !status.success() {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Expand-Archive failed for {}",
            input.display()
        ))
        .as_error());
    }

    if archive_for_expand != input {
        let _ = tokio::fs::remove_file(&archive_for_expand).await;
    }

    Ok(())
}

#[cfg(windows)]
async fn ensure_uwp_dependencies() -> crate::Result<()> {
    let deps: &[(&str, Option<&str>)] = &[
        ("Microsoft.VCLibs.140.00", Some("14.0.33519.0")),
        ("Microsoft.NET.Native.Runtime.1.4", None),
        ("Microsoft.NET.Native.Runtime.2.2", Some("2.2.28604.0")),
        ("Microsoft.VCLibs.140.00.UWPDesktop", None),
        ("Microsoft.Services.Store.Engagement", None),
        ("Microsoft.NET.Native.Framework.1.3", None),
        ("Microsoft.NET.Native.Framework.2.2", Some("2.2.29512.0")),
        ("Microsoft.GamingServices", Some("33.108.12001.0")),
    ];

    for (pkg, min_ver) in deps {
        if !is_installed_with_min(pkg, *min_ver) {
            if let Err(err) =
                install_dependency_from_rgadguard(pkg, *min_ver).await
            {
                tracing::warn!(
                    "Failed to auto-install dependency {}: {}",
                    pkg,
                    err
                );
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
fn is_installed_with_min(prefix: &str, min_version: Option<&str>) -> bool {
    let pm = match PackageManager::new() {
        Ok(v) => v,
        Err(_) => return false,
    };

    let packages = match pm.FindPackages() {
        Ok(v) => v,
        Err(_) => return false,
    };

    for pkg in packages {
        let Ok(id) = pkg.Id() else {
            continue;
        };
        let Ok(name) = id.Name() else {
            continue;
        };
        let name = name.to_string();
        if !name.starts_with(prefix) {
            continue;
        }
        if min_version.is_none() {
            return true;
        }
        let min_version = min_version.unwrap_or_default();

        if let Ok(ver) = id.Version() {
            let installed = format!(
                "{}.{}.{}.{}",
                ver.Major, ver.Minor, ver.Build, ver.Revision
            );
            if compare_versions(&installed, min_version) != Ordering::Less {
                return true;
            }
        } else if let Some(installed) = extract_version(&name)
            && compare_versions(&installed, min_version) != Ordering::Less
        {
            return true;
        }
    }

    false
}

#[cfg(windows)]
fn extract_version(s: &str) -> Option<String> {
    VER_RE
        .captures(s)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

#[cfg(windows)]
fn compare_versions(a: &str, b: &str) -> Ordering {
    let pa = a
        .split('.')
        .map(|x| x.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    let pb = b
        .split('.')
        .map(|x| x.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();

    for i in 0..pa.len().max(pb.len()) {
        let av = *pa.get(i).unwrap_or(&0);
        let bv = *pb.get(i).unwrap_or(&0);
        match av.cmp(&bv) {
            Ordering::Equal => continue,
            v => return v,
        }
    }
    Ordering::Equal
}

#[cfg(windows)]
async fn install_dependency_from_rgadguard(
    pkg: &str,
    min_version: Option<&str>,
) -> crate::Result<()> {
    let pkg_family = if pkg.contains("_8wekyb3d8bbwe") {
        pkg.to_string()
    } else {
        format!("{pkg}_8wekyb3d8bbwe")
    };
    let client = reqwest::Client::builder()
        .user_agent(crate::launcher_user_agent())
        .build()?;

    let html = client
        .post("https://store.rg-adguard.net/api/GetFiles")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Origin", "https://store.rg-adguard.net")
        .header("Referer", "https://store.rg-adguard.net/")
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.9")
        .form(&[
            ("type", "PackageFamilyName"),
            ("url", pkg_family.as_str()),
            ("ring", "RP"),
            ("lang", "en-US"),
        ])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let mut candidates = Vec::<(String, String)>::new();
    for cap in APPX_LINK_RE.captures_iter(&html) {
        let name = cap
            .name("name")
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let href = cap
            .name("href")
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let lower = name.to_ascii_lowercase();
        if (lower.contains("x64") || lower.contains("neutral"))
            && (lower.ends_with(".appx")
                || lower.ends_with(".appxbundle")
                || lower.ends_with(".msix")
                || lower.ends_with(".msixbundle"))
        {
            candidates.push((name, href));
        }
    }

    candidates.sort_by(|a, b| {
        let va = extract_version(&a.0).unwrap_or_else(|| "0.0.0.0".to_string());
        let vb = extract_version(&b.0).unwrap_or_else(|| "0.0.0.0".to_string());
        compare_versions(&vb, &va)
    });

    let picked = if let Some(min) = min_version {
        candidates.into_iter().find(|(name, _)| {
            extract_version(name)
                .map(|v| compare_versions(&v, min) != Ordering::Less)
                .unwrap_or(false)
        })
    } else {
        candidates.into_iter().next()
    }
    .ok_or_else(|| {
        crate::ErrorKind::LauncherError(format!(
            "Dependency package not found online: {pkg_family}"
        ))
    })?;

    let (name, url) = picked;
    let bytes = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let path = std::env::temp_dir().join(&name);
    tokio::fs::write(&path, bytes).await?;

    let path_str = path.to_string_lossy().replace('\'', "''");
    let output = add_appx_package_cmd(&path_str).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::ErrorKind::LauncherError(format!(
                "Dependency install failed for {pkg}: {}",
                stderr.trim()
            ))
            .as_error());
        }
    Ok(())
}
