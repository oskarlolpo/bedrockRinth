use crate::State;
use crate::state::CachedEntry;
pub use daedalus::minecraft::VersionManifest;
pub use daedalus::modded::Manifest;
use regex::Regex;
use reqwest::header::{CONTENT_DISPOSITION, REFERER};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;
use zip::ZipArchive;

pub type BedrockVersionEntry = (String, String, u8);
const BEDROCK_CACHE_FILE_NAME: &str = "onix_releases_versions.json";
const BEDROCK_CONTENT_CACHE_DIR: &str = "bedrock_content";
const BEDROCK_CONTENT_INDEX_FILE_NAME: &str = "installed_index.json";
const BEDROCK_PENDING_WORLD_IMPORTS_DIR: &str = "pending_world_imports";
const BEDROCK_VERSIONS_JSON_URL: &str =
    "https://raw.githubusercontent.com/OnixClient/onix_compatible_appx/main/versions.json";
const BEDROCK_VERSIONS_TXT_URL: &str =
    "https://raw.githubusercontent.com/OnixClient/onix_compatible_appx/main/versions.txt";

static VERSION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+\.\d+\.\d+(?:\.\d+)?)").expect("version regex")
});
static MCPEHUB_CARD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<a class="news-item[^"]*" href="(?P<href>(?:https?://(?:www\.)?mcpehub\.org)?/(?:textures|shaders|maps)/[^"]+)" title="(?P<title>[^"]+)"[^>]*>.*?<img src="(?P<img>[^"]+)""#)
        .expect("mcpehub card regex")
});
static MCPEHUB_CARD_DESC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<article class="col-xs-12 col-sm-4 col-md-6">.*?<a class="news-item[^"]*" href="(?P<href>(?:https?://(?:www\.)?mcpehub\.org)?/(?:textures|shaders|maps)/[^"]+)".*?</a>\s*<p[^>]*>(?P<desc>.*?)</p>"#)
        .expect("mcpehub card desc regex")
});
static MCPEHUB_DFILE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"href="(?P<url>https?://(?:www\.)?mcpehub\.org/engine/dlfile\.php\?id=\d+)""#,
    )
    .expect("mcpehub dlfile regex")
});
static MCPEHUB_GFILE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"href="(?P<url>(?:https?:)?//(?:www\.)?mcpehub\.org/engine/getfile\.php\?id=\d+|/engine/getfile\.php\?id=\d+)""#,
    )
    .expect("mcpehub getfile regex")
});
static MCPEHUB_PAGE_COUNT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"/(?:textures|shaders|maps)/page/(?P<page>\d+)/"#)
        .expect("mcpehub page count regex")
});
static MCPEHUB_SUPPORT_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div class="fullstory-support[^"]*".*?<div class="block list medium green">(?P<body>.*?)</div>"#,
    )
    .expect("mcpehub support block regex")
});
static MCPEHUB_ARTICLE_BODY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div class="fullstory-content regular black"[^>]*>(?P<body>.*?)</div>"#,
    )
    .expect("mcpehub article body regex")
});
static MCPEHUB_GALLERY_IMAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div class="fullstory-image"[^>]*>.*?<img[^>]+src="(?P<src>[^"]+)""#,
    )
    .expect("mcpehub gallery image regex")
});
static MCPEHUB_VERSION_SHORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(\d+\.\d+)"#).expect("mcpehub short version regex")
});
static CONTENT_DISPOSITION_FILENAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"filename="?([^";]+)"?"#).expect("content disposition regex")
});

#[derive(Debug, Deserialize)]
struct OnixVersionJsonEntry {
    version: String,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockContentEntry {
    pub title: String,
    pub page_url: String,
    pub image_url: Option<String>,
    #[serde(default)]
    pub description: String,
    pub kind: String,
    #[serde(default)]
    pub supported_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockContentPage {
    pub page: u32,
    #[serde(default = "default_one_u32")]
    pub total_pages: u32,
    pub has_next: bool,
    #[serde(default)]
    pub items: Vec<BedrockContentEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockContentDetails {
    pub title: String,
    pub page_url: String,
    pub image_url: Option<String>,
    pub description: String,
    pub supported_versions: Vec<String>,
    pub gallery_images: Vec<String>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockInstalledContentEntry {
    pub name: String,
    pub file_name: String,
    pub kind: String,
    pub rel_path: String,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub page_url: Option<String>,
    #[serde(default)]
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BedrockInstalledContentIndexEntry {
    page_url: String,
    kind: String,
    title: String,
    rel_path: String,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    description: String,
}

#[tracing::instrument]
pub async fn get_minecraft_versions() -> crate::Result<VersionManifest> {
    let state = State::get().await?;
    let minecraft_versions = CachedEntry::get_minecraft_manifest(
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::NoValueFor("minecraft versions".to_string())
    })?;

    Ok(minecraft_versions)
}

// #[tracing::instrument]
pub async fn get_loader_versions(loader: &str) -> crate::Result<Manifest> {
    let state = State::get().await?;
    let loaders = CachedEntry::get_loader_manifest(
        loader,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::NoValueFor(format!("{loader} loader versions"))
    })?;

    Ok(loaders.manifest)
}

#[tracing::instrument]
pub async fn get_bedrock_versions() -> crate::Result<Vec<BedrockVersionEntry>> {
    let state = State::get().await?;
    let cache_path = bedrock_cache_path(&state);

    match refresh_bedrock_versions_from_github().await {
        Ok(mut versions) if !versions.is_empty() => {
            if let Some(parent) = cache_path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if let Ok(bytes) = serde_json::to_vec_pretty(&versions) {
                let _ = tokio::fs::write(&cache_path, bytes).await;
            }
            normalize_bedrock_entries(&mut versions);
            Ok(versions)
        }
        Ok(_) | Err(_) => {
            let body = tokio::fs::read(&cache_path).await.map_err(|_| {
                crate::ErrorKind::NoValueFor(
                    "bedrock versions (online + cache)".to_string(),
                )
            })?;
            let mut versions: Vec<BedrockVersionEntry> =
                serde_json::from_slice(&body)?;
            normalize_bedrock_entries(&mut versions);
            if versions.is_empty() {
                return Err(crate::ErrorKind::NoValueFor(
                    "bedrock versions".to_string(),
                )
                .as_error());
            }
            Ok(versions)
        }
    }
}

fn bedrock_cache_path(state: &State) -> PathBuf {
    state
        .directories
        .metadata_dir()
        .join("bedrock")
        .join(BEDROCK_CACHE_FILE_NAME)
}

async fn refresh_bedrock_versions_from_github(
) -> crate::Result<Vec<BedrockVersionEntry>> {
    match refresh_bedrock_versions_via_json().await {
        Ok(entries) if !entries.is_empty() => Ok(entries),
        Ok(_) | Err(_) => refresh_bedrock_versions_via_txt().await,
    }
}

async fn refresh_bedrock_versions_via_json(
) -> crate::Result<Vec<BedrockVersionEntry>> {
    let client = reqwest::Client::builder()
        .user_agent(crate::launcher_user_agent())
        .build()?;
    let response = client
        .get(BEDROCK_VERSIONS_JSON_URL)
        .send()
        .await?
        .error_for_status()?;
    let raw_entries: Vec<OnixVersionJsonEntry> = response.json().await?;

    let mut out = Vec::new();
    for entry in raw_entries {
        if let Some((version, ext)) =
            parse_bedrock_asset_version_and_ext(&entry.url)
        {
            out.push((format!("{version}.{ext}"), entry.url, 0));
        } else if !entry.version.trim().is_empty() && !entry.url.trim().is_empty()
        {
            out.push((entry.version, entry.url, 0));
        }
    }
    Ok(out)
}

async fn refresh_bedrock_versions_via_txt(
) -> crate::Result<Vec<BedrockVersionEntry>> {
    let client = reqwest::Client::builder()
        .user_agent(crate::launcher_user_agent())
        .build()?;
    let body = client
        .get(BEDROCK_VERSIONS_TXT_URL)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let mut out = Vec::new();
    for chunk in lines.chunks(3) {
        if chunk.len() < 3 {
            continue;
        }
        let version = chunk[0].to_string();
        let url = chunk[2].to_string();
        if let Some((parsed_version, ext)) =
            parse_bedrock_asset_version_and_ext(&url)
        {
            out.push((format!("{parsed_version}.{ext}"), url, 0));
        } else if !version.is_empty() && !url.is_empty() {
            out.push((version, url, 0));
        }
    }
    Ok(out)
}

fn compare_dotted_versions(left: &str, right: &str) -> Ordering {
    let parse_parts = |value: &str| {
        value
            .split('.')
            .map(|part| part.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };

    let left_parts = parse_parts(left);
    let right_parts = parse_parts(right);
    let max_len = left_parts.len().max(right_parts.len());

    for idx in 0..max_len {
        let left_part = *left_parts.get(idx).unwrap_or(&0);
        let right_part = *right_parts.get(idx).unwrap_or(&0);

        match left_part.cmp(&right_part) {
            Ordering::Equal => continue,
            other => return other,
        }
    }

    Ordering::Equal
}

fn parse_bedrock_asset_version_and_ext(name: &str) -> Option<(String, String)> {
    let lowered = name.to_ascii_lowercase();
    let valid_ext = ["appx", "msix", "appxbundle", "msixbundle", "msixvc"];
    let ext = valid_ext
        .iter()
        .find(|ext| lowered.ends_with(&format!(".{ext}")))?;

    let cap = VERSION_REGEX.captures(name)?;
    let version = cap.get(1)?.as_str().to_string();
    Some((version, (*ext).to_string()))
}

fn normalize_bedrock_entries(versions: &mut Vec<BedrockVersionEntry>) {
    versions.retain(|(version, url, _)| {
        !version.trim().is_empty() && !url.trim().is_empty()
    });

    versions.sort_by(|a, b| {
        let (a_ver, a_ext) = split_version_and_ext(&a.0);
        let (b_ver, b_ext) = split_version_and_ext(&b.0);

        compare_dotted_versions(&b_ver, &a_ver)
            .then_with(|| extension_rank(&a_ext).cmp(&extension_rank(&b_ext)))
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut seen = HashSet::new();
    versions.retain(|entry| seen.insert(entry.clone()));
}

fn split_version_and_ext(version_with_ext: &str) -> (String, String) {
    if let Some((v, ext)) = version_with_ext.rsplit_once('.') {
        return (v.to_string(), ext.to_ascii_lowercase());
    }
    (version_with_ext.to_string(), String::new())
}

fn extension_rank(ext: &str) -> u8 {
    match ext {
        "appx" => 0,
        "msix" => 1,
        "appxbundle" => 2,
        "msixbundle" => 3,
        "msixvc" => 4,
        _ => 10,
    }
}

#[tracing::instrument]
pub async fn get_bedrock_content(
    kind: &str,
    page: Option<u32>,
) -> crate::Result<BedrockContentPage> {
    let kind = normalize_bedrock_content_kind(kind).ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unsupported bedrock content kind: {kind}"
        ))
        .as_error()
    })?;

    let page = page.unwrap_or(1).max(1);
    let list_url = if page <= 1 {
        format!("https://mcpehub.org/{kind}/")
    } else {
        format!("https://mcpehub.org/{kind}/page/{page}/")
    };
    let list_url_variants = vec![
        list_url.clone(),
        list_url.replace("https://mcpehub.org", "https://www.mcpehub.org"),
        list_url.replace("https://mcpehub.org", "http://mcpehub.org"),
        list_url.replace("https://mcpehub.org", "http://www.mcpehub.org"),
    ];

    let client = reqwest::Client::builder()
        .user_agent(crate::launcher_user_agent())
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let state = State::get().await?;
    let cache_path = bedrock_content_cache_path(&state, kind, page);
    let html = match fetch_text_with_fallback(&client, &list_url_variants).await
    {
        Ok(value) => value,
        Err(fetch_error) => {
            if let Ok(body) = tokio::fs::read(&cache_path).await {
                if let Ok(cached) =
                    serde_json::from_slice::<BedrockContentPage>(&body)
                {
                    return Ok(cached);
                }
            }
            return Err(fetch_error);
        }
    };

    let mut descriptions_by_url = std::collections::HashMap::<String, String>::new();
    for cap in MCPEHUB_CARD_DESC_RE.captures_iter(&html) {
        let href = cap
            .name("href")
            .map(|m| absolutize_mcpehub_url(m.as_str()))
            .unwrap_or_default();
        let desc = cap
            .name("desc")
            .map(|m| clean_html_text(m.as_str()))
            .unwrap_or_default();
        if !href.is_empty() && !desc.is_empty() {
            descriptions_by_url.insert(href, truncate_chars(&desc, 220));
        }
    }

    let mut items = Vec::new();
    for cap in MCPEHUB_CARD_RE.captures_iter(&html) {
        let Some(href_match) = cap.name("href") else {
            continue;
        };
        let Some(title_match) = cap.name("title") else {
            continue;
        };
        let page_url = absolutize_mcpehub_url(href_match.as_str());
        let raw_img = cap.name("img").map(|m| m.as_str().to_string());
        let image_url = raw_img.map(|raw| absolutize_mcpehub_url(&raw));

        items.push(BedrockContentEntry {
            title: html_escape_decode(title_match.as_str()),
            page_url: page_url.clone(),
            image_url,
            description: descriptions_by_url
                .get(&page_url)
                .cloned()
                .unwrap_or_default(),
            kind: kind.to_string(),
            supported_versions: Vec::new(),
        });
    }

    // Parse supported versions from each article page (1.19, 1.20, ...)
    for item in &mut items {
        let variants = url_variants(&item.page_url);
        if let Ok(article_html) = fetch_text_with_fallback(&client, &variants).await
        {
            item.supported_versions =
                parse_supported_versions_from_article(&article_html);
        }
    }

    let next_url = format!("https://mcpehub.org/{kind}/page/{}/", page + 1);
    let has_next = html.contains(&next_url);
    let total_pages = MCPEHUB_PAGE_COUNT_RE
        .captures_iter(&html)
        .filter_map(|cap| {
            cap.name("page")
                .and_then(|m| m.as_str().parse::<u32>().ok())
        })
        .max()
        .unwrap_or(page.max(1));

    let result = BedrockContentPage {
        page,
        total_pages,
        has_next,
        items,
    };
    if let Some(parent) = cache_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(body) = serde_json::to_vec_pretty(&result) {
        let _ = tokio::fs::write(&cache_path, body).await;
    }

    Ok(result)
}

#[tracing::instrument]
pub async fn install_bedrock_content(
    profile_path: &str,
    kind: &str,
    page_url: &str,
) -> crate::Result<String> {
    let kind = normalize_bedrock_content_kind(kind).ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unsupported bedrock content kind: {kind}"
        ))
        .as_error()
    })?;

    let client = reqwest::Client::builder()
        .user_agent(crate::launcher_user_agent())
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let article_html =
        fetch_text_with_fallback(&client, &url_variants(page_url)).await?;
    let parsed_title = Regex::new(r#"(?is)<h1[^>]*>(?P<t>.*?)</h1>"#)
        .ok()
        .and_then(|re| re.captures(&article_html))
        .and_then(|cap| cap.name("t").map(|m| clean_html_text(m.as_str())))
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| infer_title_from_page_url(page_url));
    let parsed_description = MCPEHUB_ARTICLE_BODY_RE
        .captures(&article_html)
        .and_then(|cap| cap.name("body").map(|m| clean_html_text(m.as_str())))
        .unwrap_or_default();
    let parsed_image_url = MCPEHUB_GALLERY_IMAGE_RE
        .captures(&article_html)
        .and_then(|cap| cap.name("src").map(|m| absolutize_mcpehub_url(m.as_str())));

    let mut download_url = MCPEHUB_GFILE_RE
        .captures(&article_html)
        .and_then(|cap| cap.name("url").map(|m| m.as_str().to_string()));

    if download_url.is_none() {
        if let Some(dlfile_url) = MCPEHUB_DFILE_RE
            .captures(&article_html)
            .and_then(|cap| cap.name("url").map(|m| m.as_str().to_string()))
        {
            let dl_page_html = client
                .get(absolutize_mcpehub_url(&dlfile_url))
                .header(REFERER, page_url)
                .send()
                .await?
                .error_for_status()?;
            let dl_page_html = decode_response_body_lossy(dl_page_html).await?;
            download_url = MCPEHUB_GFILE_RE
                .captures(&dl_page_html)
                .and_then(|cap| cap.name("url").map(|m| m.as_str().to_string()));
        }
    }

    let download_url = download_url
        .map(|u| absolutize_mcpehub_url(&u))
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Unable to resolve mcpehub download url".to_string(),
            )
            .as_error()
        })?;

    let response = client
        .get(&download_url)
        .header(REFERER, page_url)
        .send()
        .await?
        .error_for_status()?;

    let header_filename = response
        .headers()
        .get(CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            CONTENT_DISPOSITION_FILENAME_RE
                .captures(v)
                .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        });
    let bytes = response.bytes().await?;
    let fallback_name = format!(
        "{}-{}.zip",
        kind,
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    );
    let file_name = header_filename.unwrap_or(fallback_name);

    let state = State::get().await?;
    let profile_dir = state.directories.profiles_dir().join(profile_path);
    let bedrock_root = profile_dir.join("bedrock").join("com.mojang");
    tokio::fs::create_dir_all(&bedrock_root).await?;

    let installed_path = if kind == "maps" {
        let imports_root = profile_dir
            .join("bedrock")
            .join(BEDROCK_PENDING_WORLD_IMPORTS_DIR);
        tokio::fs::create_dir_all(&imports_root).await?;
        install_world_import_file(bytes.as_ref(), &file_name, &imports_root)
            .await?
    } else {
        let packs_root = bedrock_root.join("resource_packs");
        tokio::fs::create_dir_all(&packs_root).await?;
        install_archive_content(
            bytes.as_ref(),
            &file_name,
            &packs_root,
            false,
        )
        .await?
    };

    let rel_path = installed_path
        .strip_prefix(&bedrock_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| installed_path.to_string_lossy().replace('\\', "/"));
    update_installed_content_index(
        &profile_dir,
        BedrockInstalledContentIndexEntry {
            page_url: page_url.to_string(),
            kind: kind.to_string(),
            title: parsed_title,
            rel_path,
            image_url: parsed_image_url,
            description: parsed_description,
        },
    )
    .await?;

    Ok(installed_path.to_string_lossy().to_string())
}

#[tracing::instrument]
pub async fn get_bedrock_content_details(
    kind: &str,
    page_url: &str,
) -> crate::Result<BedrockContentDetails> {
    let kind = normalize_bedrock_content_kind(kind).ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unsupported bedrock content kind: {kind}"
        ))
        .as_error()
    })?;

    let client = reqwest::Client::builder()
        .user_agent(crate::launcher_user_agent())
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let article_html =
        fetch_text_with_fallback(&client, &url_variants(page_url)).await?;

    let title = Regex::new(r#"(?is)<h1[^>]*>(?P<t>.*?)</h1>"#)
        .ok()
        .and_then(|re| re.captures(&article_html))
        .and_then(|cap| cap.name("t").map(|m| clean_html_text(m.as_str())))
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| "Bedrock content".to_string());

    let description = MCPEHUB_ARTICLE_BODY_RE
        .captures(&article_html)
        .and_then(|cap| cap.name("body").map(|m| clean_html_text(m.as_str())))
        .unwrap_or_default();

    let image_url = MCPEHUB_GALLERY_IMAGE_RE
        .captures(&article_html)
        .and_then(|cap| cap.name("src").map(|m| absolutize_mcpehub_url(m.as_str())));
    let supported_versions = parse_supported_versions_from_article(&article_html);

    let mut gallery_images = Vec::new();
    for cap in MCPEHUB_GALLERY_IMAGE_RE.captures_iter(&article_html) {
        if let Some(src) = cap.name("src") {
            gallery_images.push(absolutize_mcpehub_url(src.as_str()));
        }
    }
    gallery_images.sort();
    gallery_images.dedup();

    Ok(BedrockContentDetails {
        title,
        page_url: page_url.to_string(),
        image_url,
        description,
        supported_versions,
        gallery_images,
        kind: kind.to_string(),
    })
}

#[tracing::instrument]
pub async fn get_installed_bedrock_content(
    profile_path: &str,
) -> crate::Result<Vec<BedrockInstalledContentEntry>> {
    let state = State::get().await?;
    let profile_dir = state.directories.profiles_dir().join(profile_path);
    let bedrock_root = profile_dir.join("bedrock").join("com.mojang");
    let mut out = Vec::new();
    let index = read_installed_content_index(&profile_dir).await.unwrap_or_default();
    let mut meta_by_rel = std::collections::HashMap::<
        String,
        (String, Option<String>, String),
    >::new();
    for entry in index {
        meta_by_rel.insert(
            entry.rel_path.replace('\\', "/"),
            (entry.page_url, entry.image_url, entry.description),
        );
    }

    collect_installed_entries(
        &bedrock_root.join("resource_packs"),
        "resourcepack",
        "resource_packs",
        &meta_by_rel,
        &mut out,
    )
    .await?;
    collect_installed_entries(
        &bedrock_root.join("minecraftWorlds"),
        "world",
        "minecraftWorlds",
        &meta_by_rel,
        &mut out,
    )
    .await?;

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn normalize_bedrock_content_kind(kind: &str) -> Option<&'static str> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "textures" => Some("textures"),
        "shaders" => Some("shaders"),
        "maps" => Some("maps"),
        _ => None,
    }
}

fn bedrock_content_cache_path(state: &State, kind: &str, page: u32) -> PathBuf {
    state
        .directories
        .metadata_dir()
        .join("bedrock")
        .join(BEDROCK_CONTENT_CACHE_DIR)
        .join(format!("{kind}-{page}.json"))
}

fn default_one_u32() -> u32 {
    1
}

async fn fetch_text_with_fallback(
    client: &reqwest::Client,
    urls: &[String],
) -> crate::Result<String> {
    let mut last_error = String::new();
    for url in urls {
        match client.get(url).send().await {
            Ok(resp) => match resp.error_for_status() {
                Ok(ok) => match decode_response_body_lossy(ok).await {
                    Ok(text) => return Ok(text),
                    Err(err) => {
                        last_error =
                            format!("Error reading response body ({url}): {err}");
                    }
                },
                Err(err) => {
                    last_error =
                        format!("HTTP error while fetching {url}: {err}");
                }
            },
            Err(err) => {
                last_error = format!("Error fetching URL ({url}): {err}");
            }
        }
    }

    Err(crate::ErrorKind::InputError(
        if last_error.is_empty() {
            "Unable to fetch content from all fallback URLs".to_string()
        } else {
            last_error
        },
    )
    .as_error())
}

async fn decode_response_body_lossy(
    response: reqwest::Response,
) -> crate::Result<String> {
    let bytes = response.bytes().await?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn url_variants(url: &str) -> Vec<String> {
    let mut out = vec![url.to_string()];
    if url.contains("://mcpehub.org") {
        out.push(url.replace("://mcpehub.org", "://www.mcpehub.org"));
        out.push(url.replace("https://mcpehub.org", "http://mcpehub.org"));
    } else if url.contains("://www.mcpehub.org") {
        out.push(url.replace("://www.mcpehub.org", "://mcpehub.org"));
        out.push(url.replace("https://www.mcpehub.org", "http://www.mcpehub.org"));
    }
    out
}

fn absolutize_mcpehub_url(url: &str) -> String {
    if url.starts_with("https://") || url.starts_with("http://") {
        url.to_string()
    } else if url.starts_with("//") {
        format!("https:{url}")
    } else if url.starts_with('/') {
        format!("https://mcpehub.org{url}")
    } else {
        format!("https://mcpehub.org/{url}")
    }
}

fn html_escape_decode(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn truncate_chars(value: &str, max: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max {
            break;
        }
        out.push(ch);
    }
    if value.chars().count() > max {
        out.push_str("...");
    }
    out
}

fn clean_html_text(text: &str) -> String {
    let without_tags = Regex::new(r"(?is)<[^>]+>")
        .ok()
        .map(|re| re.replace_all(text, " ").to_string())
        .unwrap_or_else(|| text.to_string());
    html_escape_decode(
        &without_tags
            .replace("&nbsp;", " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

async fn collect_installed_entries(
    dir: &std::path::Path,
    kind: &str,
    rel_root: &str,
    meta_by_rel: &std::collections::HashMap<String, (String, Option<String>, String)>,
    out: &mut Vec<BedrockInstalledContentEntry>,
) -> crate::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let mut rd = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = rd.next_entry().await? {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata().await?;
        if !meta.is_dir() && !meta.is_file() {
            continue;
        }
        let rel_path = {
            let leaf = path
                .file_name()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            format!("{rel_root}/{leaf}")
        };
        let meta = meta_by_rel.get(&rel_path).cloned();
        out.push(BedrockInstalledContentEntry {
            name: file_name.clone(),
            file_name,
            kind: kind.to_string(),
            image_url: meta.as_ref().and_then(|(_, image_url, _)| image_url.clone()),
            description: meta
                .as_ref()
                .map(|(_, _, description)| description.clone())
                .unwrap_or_default(),
            page_url: meta.map(|(page_url, _, _)| page_url),
            installed: true,
            rel_path,
        });
    }
    Ok(())
}

fn installed_content_index_path(profile_dir: &Path) -> PathBuf {
    profile_dir
        .join("bedrock")
        .join(BEDROCK_CONTENT_INDEX_FILE_NAME)
}

async fn read_installed_content_index(
    profile_dir: &Path,
) -> crate::Result<Vec<BedrockInstalledContentIndexEntry>> {
    let path = installed_content_index_path(profile_dir);
    if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(Vec::new());
    }
    let bytes = tokio::fs::read(path).await?;
    let parsed = serde_json::from_slice::<Vec<BedrockInstalledContentIndexEntry>>(&bytes)?;
    Ok(parsed)
}

async fn update_installed_content_index(
    profile_dir: &Path,
    entry: BedrockInstalledContentIndexEntry,
) -> crate::Result<()> {
    let path = installed_content_index_path(profile_dir);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut rows = read_installed_content_index(profile_dir).await.unwrap_or_default();
    rows.retain(|x| !(x.page_url == entry.page_url && x.kind == entry.kind));
    rows.push(entry);
    let body = serde_json::to_vec_pretty(&rows)?;
    tokio::fs::write(path, body).await?;
    Ok(())
}

fn infer_title_from_page_url(page_url: &str) -> String {
    page_url
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim_end_matches(".html")
        .replace('-', " ")
}

fn sanitize_fs_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let out = out.trim().trim_matches('.').to_string();
    if out.is_empty() {
        "bedrock_content".to_string()
    } else {
        out
    }
}

fn unique_dir(base: &std::path::Path, name: &str) -> PathBuf {
    let mut candidate = base.join(name);
    let mut idx = 1u32;
    while candidate.exists() {
        candidate = base.join(format!("{name}_{idx}"));
        idx += 1;
    }
    candidate
}

fn unique_file(base: &std::path::Path, name: &str) -> PathBuf {
    let mut candidate = base.join(name);
    let mut idx = 1u32;
    while candidate.exists() {
        let (stem, ext) = name
            .rsplit_once('.')
            .map(|(s, e)| (s.to_string(), Some(e.to_string())))
            .unwrap_or_else(|| (name.to_string(), None));
        let next_name = if let Some(ext) = &ext {
            format!("{stem}_{idx}.{ext}")
        } else {
            format!("{stem}_{idx}")
        };
        candidate = base.join(next_name);
        idx += 1;
    }
    candidate
}

async fn install_archive_content(
    bytes: &[u8],
    file_name: &str,
    root: &std::path::Path,
    is_world: bool,
) -> crate::Result<PathBuf> {
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "content".to_string());
    let safe_stem = sanitize_fs_name(&stem);
    let ext = std::path::Path::new(file_name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    let looks_like_archive =
        ext == "zip" || ext == "mcpack" || ext == "mcworld" || ext == "mcaddon";

    if looks_like_archive {
        let target_dir = unique_dir(root, &safe_stem);
        tokio::fs::create_dir_all(&target_dir).await?;
        let payload = bytes.to_vec();
        let target_dir_clone = target_dir.clone();
        let extraction = tokio::task::spawn_blocking(move || {
            extract_zip_bytes_to_dir(&payload, &target_dir_clone)
        })
        .await
        .map_err(|e| {
            crate::ErrorKind::OtherError(format!(
                "bedrock content extraction thread failed: {e}"
            ))
            .as_error()
        })?;

        if extraction.is_ok() {
            if is_world {
                let normalized = normalize_world_layout(&target_dir).await?;
                return Ok(normalized);
            }
            return Ok(target_dir);
        }

        let _ = tokio::fs::remove_dir_all(&target_dir).await;
    }

    if is_world {
        return Err(crate::ErrorKind::InputError(
            "Map archive could not be extracted into minecraftWorlds".to_string(),
        )
        .as_error());
    }

    let fallback_name = if is_world {
        format!("{safe_stem}.mcworld")
    } else {
        format!("{safe_stem}.mcpack")
    };
    let target_file = unique_file(root, &fallback_name);
    tokio::fs::write(&target_file, bytes).await?;
    Ok(target_file)
}

async fn install_world_import_file(
    bytes: &[u8],
    file_name: &str,
    imports_root: &Path,
) -> crate::Result<PathBuf> {
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "world".to_string());
    let safe_stem = sanitize_fs_name(&stem);
    let target_name = format!("{safe_stem}.mcworld");
    let target_file = unique_file(imports_root, &target_name);
    tokio::fs::write(&target_file, bytes).await?;
    Ok(target_file)
}

fn extract_zip_bytes_to_dir(bytes: &[u8], target_dir: &std::path::Path) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("invalid archive: {e}"))?;
    let strip_prefix = detect_zip_common_root(bytes);

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("zip entry read failed: {e}"))?;

        let mut rel_parts = Vec::new();
        for comp in std::path::Path::new(file.name()).components() {
            if let Component::Normal(value) = comp {
                rel_parts.push(value.to_string_lossy().to_string());
            }
        }

        if rel_parts.is_empty() {
            continue;
        }

        if let Some(prefix) = &strip_prefix {
            if rel_parts.first() == Some(prefix) {
                rel_parts.remove(0);
            }
        }

        if rel_parts.is_empty() {
            continue;
        }

        let mut rel = PathBuf::new();
        for part in rel_parts {
            rel.push(part);
        }
        let out_path = target_dir.join(rel);

        if file.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("create dir failed: {e}"))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create parent dir failed: {e}"))?;
        }

        let mut out = std::fs::File::create(&out_path)
            .map_err(|e| format!("create file failed: {e}"))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| format!("read archive entry failed: {e}"))?;
        out.write_all(&buffer)
            .map_err(|e| format!("write extracted file failed: {e}"))?;
    }

    Ok(())
}

fn detect_zip_common_root(bytes: &[u8]) -> Option<String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec())).ok()?;
    let mut first_root: Option<String> = None;

    for i in 0..archive.len() {
        let file = archive.by_index(i).ok()?;
        if file.is_dir() {
            continue;
        }
        let mut parts = std::path::Path::new(file.name())
            .components()
            .filter_map(|comp| match comp {
                Component::Normal(v) => Some(v.to_string_lossy().to_string()),
                _ => None,
            });
        let root = parts.next()?;
        if parts.next().is_none() {
            return None;
        }
        match &first_root {
            None => first_root = Some(root),
            Some(existing) if *existing == root => {}
            Some(_) => return None,
        }
    }

    first_root
}

fn parse_supported_versions_from_article(article_html: &str) -> Vec<String> {
    let mut versions = Vec::<String>::new();

    if let Some(block) = MCPEHUB_SUPPORT_BLOCK_RE
        .captures(article_html)
        .and_then(|cap| cap.name("body").map(|m| m.as_str().to_string()))
    {
        for cap in MCPEHUB_VERSION_SHORT_RE.captures_iter(&block) {
            if let Some(v) = cap.get(1) {
                let value = v.as_str().to_string();
                if !versions.contains(&value) {
                    versions.push(value);
                }
            }
        }
    }

    versions
}

async fn normalize_world_layout(target_dir: &PathBuf) -> crate::Result<PathBuf> {
    if world_root_has_level_data(target_dir).await {
        return Ok(target_dir.clone());
    }

    let mut entries = tokio::fs::read_dir(target_dir).await?;
    let mut dirs = Vec::<PathBuf>::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }

    if dirs.len() == 1 && world_root_has_level_data(&dirs[0]).await {
        let nested = dirs.remove(0);
        let mut nested_entries = tokio::fs::read_dir(&nested).await?;
        while let Some(entry) = nested_entries.next_entry().await? {
            let from = entry.path();
            let to = target_dir.join(entry.file_name());
            tokio::fs::rename(&from, &to).await?;
        }
        let _ = tokio::fs::remove_dir_all(&nested).await;
    }

    if !world_root_has_level_data(target_dir).await {
        return Err(crate::ErrorKind::InputError(
            "Extracted map does not contain level.dat".to_string(),
        )
        .as_error());
    }

    Ok(target_dir.clone())
}

async fn world_root_has_level_data(root: &PathBuf) -> bool {
    let level = root.join("level.dat");
    let db = root.join("db");
    tokio::fs::try_exists(&level).await.unwrap_or(false)
        || tokio::fs::try_exists(&db).await.unwrap_or(false)
}
