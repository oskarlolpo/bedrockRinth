pub async fn bedrock_scan(_n: usize) -> crate::Result<()> {
    Err(crate::ErrorKind::LauncherError(
        "Bedrock install/download is disabled in this build".to_string(),
    )
    .as_error())
}

pub async fn bedrock_scan_until_ok(
    _max: usize,
) -> crate::Result<Option<(String, String, String, String)>> {
    Err(crate::ErrorKind::LauncherError(
        "Bedrock install/download is disabled in this build".to_string(),
    )
    .as_error())
}

pub async fn bedrock_resolve_url(
    _version: &str,
    _update_id: Option<&str>,
    _revision: Option<u8>,
) -> crate::Result<String> {
    Err(crate::ErrorKind::LauncherError(
        "Bedrock install/download is disabled in this build".to_string(),
    )
    .as_error())
}

pub async fn bedrock_download(
    _version: &str,
    _out: Option<std::path::PathBuf>,
) -> crate::Result<std::path::PathBuf> {
    Err(crate::ErrorKind::LauncherError(
        "Bedrock install/download is disabled in this build".to_string(),
    )
    .as_error())
}

pub async fn bedrock_resolve_and_probe(_version: &str) -> crate::Result<String> {
    Err(crate::ErrorKind::LauncherError(
        "Bedrock install/download is disabled in this build".to_string(),
    )
    .as_error())
}
