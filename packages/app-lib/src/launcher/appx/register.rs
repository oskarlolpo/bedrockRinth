use std::io;
use tracing::{error, info};
use windows::core::{Error as WinError, HRESULT, HSTRING, Result as WinResult};
use windows::Foundation::Uri;
use windows::Management::Deployment::{DeploymentOptions, DeploymentResult, PackageManager};

pub async fn register_appx_package_async(package_folder: &str) -> WinResult<DeploymentResult> {
    // Build absolute manifest path.
    let mut manifest_path = package_folder.replace('\\', "/");
    if manifest_path.ends_with('/') {
        manifest_path.pop();
    }

    let manifest_file = format!("{}/AppxManifest.xml", manifest_path);
    let absolute_path = std::fs::canonicalize(&manifest_file).map_err(|e| {
        windows::core::Error::from(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to resolve absolute manifest path: {}", e),
        ))
    })?;

    let mut uri_path = absolute_path.to_string_lossy().to_string();
    if uri_path.starts_with(r"\\?\") {
        uri_path = uri_path[4..].to_string();
    }

    let uri_str = format!("file:///{}", uri_path.replace("\\", "/"));
    info!("Registering APPX with URI: {}", uri_str);

    let package_manager = PackageManager::new().expect("Failed to create PackageManager");
    let uri = Uri::CreateUri(&HSTRING::from(uri_str))?;

    // Start registration and wait for completion.
    let async_op =
        package_manager.RegisterPackageAsync(&uri, None, DeploymentOptions::DevelopmentMode)?;
    let result: DeploymentResult = async_op.get()?;

    // Check deployment result.
    let extended_error = result.ExtendedErrorCode()?;
    let error_text = result.ErrorText()?.to_string_lossy();

    if extended_error == HRESULT(0) {
        info!("APPX registration completed successfully");
        Ok(result)
    } else {
        error!("APPX registration failed: {:?} - {}", extended_error, error_text);
        Err(WinError::new(extended_error, error_text))
    }
}
