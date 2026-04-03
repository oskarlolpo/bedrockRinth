use tracing::{error, info};
use windows::core::{Error as WinError, HRESULT, HSTRING, Result as WinResult};
use windows::Management::Deployment::{DeploymentResult, PackageManager, RemovalOptions};

/// Uninstall an Appx package by package family name.
pub async fn remove_package(package_family_name: &str) -> WinResult<()> {
    let package_manager = PackageManager::new().map_err(|e| {
        error!("Failed to create PackageManager: {:?}", e);
        e
    })?;

    // Start asynchronous uninstall request.
    let async_op = package_manager.RemovePackageWithOptionsAsync(
        &HSTRING::from(package_family_name),
        RemovalOptions::PreserveApplicationData,
    )?;

    // Await uninstall completion.
    let result: DeploymentResult = async_op.get()?;

    // Read extended HRESULT and optional text.
    let extended_hr: HRESULT = result.ExtendedErrorCode()?;
    let error_text: String = match result.ErrorText() {
        Ok(h) => h.to_string_lossy(),
        Err(_) => String::new(),
    };

    if extended_hr == HRESULT(0) {
        info!("Package removed successfully: {}", package_family_name);
        Ok(())
    } else {
        error!(
            "Package removal failed, extended error code: {:?}, error text: {}",
            extended_hr, error_text
        );
        Err(WinError::new(extended_hr, error_text))
    }
}
