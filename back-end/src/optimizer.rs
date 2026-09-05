use crate::config::EmuType;
use crate::utils::io_error;
use include_dir::{include_dir, Dir};
use ini::Ini;
use regex::RegexBuilder;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{CleanupOption, OptimizerConfig, SelectedModSourceOption};
use crate::profile::UserProfile;

static BUNDLED_SAVE_DATA: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundled_data/save");
static BUNDLED_SSBU_SETTINGS: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundled_data/ssbu_settings");

const DEFAULT_MOD_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/saad-script/ssbu-emu-optimizer/refs/heads/main/mod_manifest.toml";
const DEV_LOCAL_MOD_MANIFEST_FILE: &str = "mod_manifest.local.toml";

#[derive(Debug, Deserialize)]
struct ModsUpdateManifest {
    mods: Vec<ModEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManifestModEntry {
    pub name: String,
    pub id: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub source_options: Vec<String>,
    pub default_source_option: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ModSource {
    Github,
    Gamebanana,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ManifestVersion {
    Tag(String),
    Index(usize),
}

#[derive(Debug, Clone, Deserialize)]
struct ModSourceEntry {
    source: ModSource,
    id: String,
    version: Option<ManifestVersion>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModSourcesOption {
    name: String,
    sources: Vec<ModSourceEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModEntry {
    name: String,
    description: Option<String>,
    sources_options: Vec<ModSourcesOption>,
    enabled: Option<bool>,
    #[serde(default)]
    dependencies: Vec<String>,
    install_to: Vec<(String, String)>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone)]
struct DownloadAsset {
    name: String,
    url: String,
}

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> io::Result<Self> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_millis();
        let path = std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), now));
        fs::create_dir_all(path.as_path())?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_dir_all(self.path.as_path()) {
            log::warn!(
                "Unable to cleanup temporary directory {}: {}",
                self.path.to_string_lossy(),
                err
            );
        }
    }
}

fn load_bundled_dir(bundled_dir: &Dir, target: PathBuf) -> io::Result<()> {
    log::info!("Creating directory path: {}", target.to_string_lossy());
    fs::create_dir_all(&target)?;
    for entry in bundled_dir.entries() {
        if let Some(dir) = entry.as_dir() {
            let relative_path =
                dir.path()
                    .strip_prefix(bundled_dir.path())
                    .ok()
                    .ok_or(io_error!(
                        NotFound,
                        "Unable to load bundled directory: {}",
                        dir.path().to_string_lossy()
                    ))?;
            load_bundled_dir(dir, target.join(relative_path))?;
        } else if let Some(file) = entry.as_file() {
            log::info!("Writing file: {}", file.path().to_string_lossy());
            fs::write(
                target.join(file.path().file_name().ok_or(io_error!(
                    NotFound,
                    "Unable to load bundled file: {}",
                    file.path().to_string_lossy()
                ))?),
                file.contents(),
            )?;
        }
    }
    Ok(())
}

fn get_mod_manifest_url() -> &'static str {
    option_env!("SSBU_MOD_UPDATE_TOML_URL").unwrap_or(DEFAULT_MOD_MANIFEST_URL)
}

fn build_http_client() -> io::Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| io_error!(Other, "Unable to build HTTP client: {}", err))
}

fn fetch_text(client: &Client, url: &str) -> io::Result<String> {
    let response = client
        .get(url)
        .header(USER_AGENT, "ssbu-emu-optimizer")
        .header(ACCEPT, "application/json, text/plain, */*")
        .send()
        .map_err(|err| io_error!(Other, "Unable to fetch {}: {}", url, err))?;

    if !response.status().is_success() {
        return Err(io_error!(
            Other,
            "Request to {} failed with status {}",
            url,
            response.status()
        ));
    }

    response
        .text()
        .map_err(|err| io_error!(Other, "Unable to read response body: {}", err))
}

fn load_mod_manifest(client: &Client) -> io::Result<ModsUpdateManifest> {
    let dev_manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEV_LOCAL_MOD_MANIFEST_FILE);
    let update_toml = if cfg!(debug_assertions) && dev_manifest_path.is_file() {
        log::info!(
            "Loading local update manifest from {}",
            dev_manifest_path.to_string_lossy()
        );
        fs::read_to_string(dev_manifest_path.as_path()).map_err(|err| {
            io_error!(
                Other,
                "Unable to read local update manifest {}: {}",
                dev_manifest_path.to_string_lossy(),
                err
            )
        })?
    } else {
        let url = get_mod_manifest_url();
        log::info!("Loading update manifest from {}", url);
        fetch_text(client, url)?
    };

    let manifest: ModsUpdateManifest = toml::from_str(update_toml.as_str())
        .map_err(|err| io_error!(InvalidData, "Invalid update.toml format: {}", err))?;

    if manifest.mods.is_empty() {
        return Err(io_error!(
            InvalidData,
            "update.toml does not define any mods"
        ));
    }

    Ok(manifest)
}

fn download_asset(client: &Client, url: &str, destination: &Path) -> io::Result<()> {
    let mut response = client
        .get(url)
        .header(USER_AGENT, "ssbu-emu-optimizer")
        .send()
        .map_err(|err| io_error!(Other, "Unable to download {}: {}", url, err))?;

    if !response.status().is_success() {
        return Err(io_error!(
            Other,
            "Download from {} failed with status {}",
            url,
            response.status()
        ));
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::File::create(destination)?;
    io::copy(&mut response, &mut file)?;
    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, output_dir: &Path) -> io::Result<()> {
    let zip_file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(zip_file)
        .map_err(|err| io_error!(InvalidData, "Unable to read zip archive: {}", err))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| io_error!(InvalidData, "Invalid zip entry at {}: {}", index, err))?;
        let Some(relative_path) = file.enclosed_name() else {
            continue;
        };

        let output_path = output_dir.join(relative_path);
        if file.name().ends_with('/') {
            fs::create_dir_all(output_path.as_path())?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out_file = fs::File::create(output_path.as_path())?;
        io::copy(&mut file, &mut out_file)?;
    }

    Ok(())
}

fn extract_7z_to_dir(archive_path: &Path, output_dir: &Path) -> io::Result<()> {
    sevenz_rust::decompress_file(archive_path, output_dir)
        .map_err(|err| io_error!(InvalidData, "Unable to read 7z archive: {}", err))
}

fn extract_archive_contents_to_dir(archive_path: &Path, destination_dir: &Path) -> io::Result<()> {
    let archive_name = archive_path.file_stem().ok_or(io_error!(
        InvalidFilename,
        "Unable to infer archive base name for {}",
        archive_path.to_string_lossy()
    ))?;
    let archive_destination = destination_dir.join(archive_name);
    let extension = archive_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();

    fs::create_dir_all(archive_destination.as_path())?;

    match extension.as_str() {
        "zip" => extract_zip_to_dir(archive_path, archive_destination.as_path())?,
        "7z" => extract_7z_to_dir(archive_path, archive_destination.as_path())?,
        _ => {
            return Err(io_error!(
                InvalidInput,
                "Unsupported archive type for {}",
                archive_path.to_string_lossy()
            ));
        }
    }

    Ok(())
}

fn collect_files_recursive(path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if path.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_files_recursive(entry_path.as_path(), files)?;
        } else if entry_path.is_file() {
            files.push(entry_path);
        }
    }
    Ok(())
}

fn collect_dirs_recursive(path: &Path, dirs: &mut Vec<PathBuf>) -> io::Result<()> {
    if path.is_dir() {
        dirs.push(path.to_path_buf());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_dirs_recursive(entry_path.as_path(), dirs)?;
        }
    }

    Ok(())
}

fn copy_directory_contents(source_dir: &Path, destination_dir: &Path) -> io::Result<()> {
    let mut dirs = Vec::new();
    collect_dirs_recursive(source_dir, &mut dirs)?;
    for source_subdir in dirs.iter() {
        let relative_path = source_subdir.strip_prefix(source_dir).map_err(|err| {
            io_error!(
                InvalidData,
                "Unable to derive directory path {}: {}",
                source_subdir.to_string_lossy(),
                err
            )
        })?;
        fs::create_dir_all(destination_dir.join(relative_path).as_path())?;
    }

    let mut files = Vec::new();
    collect_files_recursive(source_dir, &mut files)?;
    for source_file in files.iter() {
        let relative_path = source_file.strip_prefix(source_dir).map_err(|err| {
            io_error!(
                InvalidData,
                "Unable to derive directory file path {}: {}",
                source_file.to_string_lossy(),
                err
            )
        })?;

        let destination_file = destination_dir.join(relative_path);
        if let Some(parent) = destination_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source_file, destination_file.as_path())?;
    }

    Ok(())
}

fn resolve_install_sources(
    source_spec: &str,
    extracted_root: &Path,
) -> io::Result<Vec<PathBuf>> {
    let normalized_source = source_spec.trim();
    if normalized_source.is_empty() {
        return Err(io_error!(
            InvalidInput,
            "install_to source pattern is empty: {}",
            source_spec
        ));
    }

    let pattern = format!("^(?:{})$", normalized_source);
    let regex = RegexBuilder::new(pattern.as_str())
        .case_insensitive(true)
        .build()
        .map_err(|err| {
        io_error!(
            InvalidInput,
            "Invalid source regex pattern '{}': {}",
            source_spec,
            err
        )
    })?;

    let mut files = Vec::new();
    collect_files_recursive(extracted_root, &mut files)?;
    let mut dirs = Vec::new();
    collect_dirs_recursive(extracted_root, &mut dirs)?;

    let mut matches = Vec::new();
    for path in files.into_iter().chain(dirs.into_iter()) {
        let Ok(relative_path) = path.strip_prefix(extracted_root) else {
            continue;
        };
        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let normalized_relative = relative_path.to_string_lossy().replace('\\', "/");
        if regex.is_match(normalized_relative.as_str()) {
            matches.push(path);
        }
    }
    matches.sort();

    if matches.is_empty() {
        return Err(io_error!(
            NotFound,
            "Unable to find files or directories matching source regex {}",
            source_spec
        ));
    }

    Ok(matches)
}

fn install_mod_files(
    mod_entry: &ModEntry,
    extracted_root: &Path,
    sdmc_path: &Path,
) -> io::Result<()> {
    for (source_spec, destination_spec) in mod_entry.install_to.iter() {
        let source_paths = resolve_install_sources(source_spec.as_str(), extracted_root)?;
        let destination_relative = Path::new(destination_spec);
        if destination_relative.is_absolute() {
            return Err(io_error!(
                InvalidInput,
                "Destination path cannot be absolute: {}",
                destination_spec
            ));
        }

        if destination_relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
        {
            return Err(io_error!(
                InvalidInput,
                "Destination path cannot contain parent/root segments: {}",
                destination_spec
            ));
        }

        for source_path in source_paths.iter() {
            if source_path.is_file() {
                let source_file = source_path;
                let destination_is_directory =
                    destination_spec.ends_with('/') || destination_spec.ends_with('\\');
                let destination = if destination_is_directory {
                    let file_name = source_file.file_name().ok_or(io_error!(
                        NotFound,
                        "Unable to infer source file name for {}",
                        source_file.to_string_lossy()
                    ))?;
                    sdmc_path.join(destination_relative).join(file_name)
                } else {
                    sdmc_path.join(destination_relative)
                };

                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(source_file.as_path(), destination.as_path())?;
                continue;
            }

            if source_path.is_dir() {
                let source_dir = source_path;
                let source_dir_name = source_dir.file_name().ok_or(io_error!(
                    NotFound,
                    "Unable to infer source directory name for {}",
                    source_dir.to_string_lossy()
                ))?;
                let destination_dir = sdmc_path.join(destination_relative).join(source_dir_name);
                fs::create_dir_all(destination_dir.as_path())?;
                copy_directory_contents(source_dir.as_path(), destination_dir.as_path())?;
                continue;
            }

            return Err(io_error!(
                NotFound,
                "Source path no longer exists: {}",
                source_path.to_string_lossy()
            ));
        }
    }

    Ok(())
}

fn gamebanana_version_index(
    version: Option<&ManifestVersion>,
    mod_id: &str,
) -> io::Result<Option<usize>> {
    match version {
        None => Ok(None),
        Some(ManifestVersion::Index(index)) => Ok(Some(*index)),
        Some(ManifestVersion::Tag(tag)) => tag.parse::<usize>().map(Some).map_err(|_| {
            io_error!(
                InvalidData,
                "GameBanana version for {} must be a non-negative integer index",
                mod_id
            )
        }),
    }
}

fn github_release_assets(
    client: &Client,
    repo_id: &str,
    version: Option<&ManifestVersion>,
) -> io::Result<Vec<DownloadAsset>> {
    let api_url = match version {
        Some(ManifestVersion::Tag(tag)) => {
            let mut url =
                reqwest::Url::parse(format!("https://api.github.com/repos/{}/", repo_id).as_str())
                    .map_err(|err| {
                        io_error!(InvalidInput, "Invalid GitHub repo id {}: {}", repo_id, err)
                    })?;
            {
                let mut segments = url.path_segments_mut().map_err(|_| {
                    io_error!(
                        InvalidInput,
                        "Invalid GitHub API base URL for repo {}",
                        repo_id
                    )
                })?;
                segments.push("releases");
                segments.push("tags");
                segments.push(tag.as_str());
            }
            url.to_string()
        }
        Some(ManifestVersion::Index(_)) => {
            return Err(io_error!(
                InvalidData,
                "GitHub version for {} must be a release tag string",
                repo_id
            ));
        }
        None => format!("https://api.github.com/repos/{}/releases/latest", repo_id),
    };
    let response = client
        .get(api_url.as_str())
        .header(USER_AGENT, "ssbu-emu-optimizer")
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .map_err(|err| {
            io_error!(
                Other,
                "Unable to query latest GitHub release for {}: {}",
                repo_id,
                err
            )
        })?;

    if !response.status().is_success() {
        return Err(io_error!(
            Other,
            "GitHub API request failed for {} with status {}",
            repo_id,
            response.status()
        ));
    }

    let release: GithubRelease = response
        .json()
        .map_err(|err| io_error!(InvalidData, "Invalid GitHub release payload: {}", err))?;

    if release.assets.is_empty() {
        return Err(io_error!(
            InvalidData,
            "Latest release for {} has no downloadable assets",
            repo_id
        ));
    }

    Ok(release
        .assets
        .into_iter()
        .map(|asset| DownloadAsset {
            name: asset.name,
            url: asset.browser_download_url,
        })
        .collect())
}

fn gamebanana_latest_asset(
    client: &Client,
    mod_id: &str,
    version: Option<&ManifestVersion>,
) -> io::Result<Vec<DownloadAsset>> {
    let api_url = format!(
        "https://gamebanana.com/apiv11/Mod/{}?_csvProperties=_aFiles",
        mod_id
    );
    let response = client
        .get(api_url.as_str())
        .header(USER_AGENT, "ssbu-emu-optimizer")
        .send()
        .map_err(|err| io_error!(Other, "Unable to query GameBanana mod {}: {}", mod_id, err))?;

    if !response.status().is_success() {
        return Err(io_error!(
            Other,
            "GameBanana API request failed for {} with status {}",
            mod_id,
            response.status()
        ));
    }

    let payload: serde_json::Value = response
        .json()
        .map_err(|err| io_error!(InvalidData, "Invalid GameBanana response: {}", err))?;

    let files = payload.get("_aFiles").ok_or(io_error!(
        InvalidData,
        "GameBanana response for {} does not include _aFiles",
        mod_id
    ))?;

    let parse_file = |file: &serde_json::Value| {
        let file_name = file.get("_sFile")?.as_str()?.to_string();
        let download_url = file.get("_sDownloadUrl")?.as_str()?.to_string();
        let timestamp = file
            .get("_tsDateAdded")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        Some((
            timestamp,
            DownloadAsset {
                name: file_name,
                url: download_url,
            },
        ))
    };

    let selected_index = gamebanana_version_index(version, mod_id)?;

    let mut ordered_assets: Vec<(i64, DownloadAsset)> = match files {
        serde_json::Value::Array(entries) => entries.iter().filter_map(parse_file).collect(),
        serde_json::Value::Object(entries) => entries.values().filter_map(parse_file).collect(),
        _ => {
            return Err(io_error!(
                InvalidData,
                "GameBanana _aFiles for {} has unexpected format",
                mod_id
            ));
        }
    };

    ordered_assets.sort_by(|a, b| a.0.cmp(&b.0));

    if ordered_assets.is_empty() {
        return Err(io_error!(
            InvalidData,
            "GameBanana mod {} has no downloadable files",
            mod_id
        ));
    }

    let latest = if let Some(selected_index) = selected_index {
        ordered_assets
            .into_iter()
            .nth(selected_index)
            .map(|(_, asset)| asset)
            .ok_or(io_error!(
                InvalidData,
                "GameBanana mod {} does not have file index {}",
                mod_id,
                selected_index
            ))?
    } else {
        ordered_assets
            .into_iter()
            .last()
            .map(|(_, asset)| asset)
            .ok_or(io_error!(
                InvalidData,
                "GameBanana mod {} has no downloadable files",
                mod_id
            ))?
    };

    Ok(vec![latest])
}

fn mod_sources(
    mod_entry: &ModEntry,
    selected_source_options: &HashMap<String, String>,
) -> io::Result<Vec<ModSourceEntry>> {
    let selected_option_name = selected_source_options.get(mod_entry.name.as_str());
    let selected_option = if let Some(option_name) = selected_option_name {
        mod_entry
            .sources_options
            .iter()
            .find(|option| option.name == *option_name)
            .ok_or(io_error!(
                InvalidData,
                "Mod {} does not define source option '{}'",
                mod_entry.name,
                option_name
            ))?
    } else {
        mod_entry.sources_options.first().ok_or(io_error!(
            InvalidData,
            "Mod {} does not define any source options",
            mod_entry.name
        ))?
    };

    if selected_option.sources.is_empty() {
        return Err(io_error!(
            InvalidData,
            "Mod {} source option '{}' does not define any sources",
            mod_entry.name,
            selected_option.name
        ));
    }

    Ok(selected_option.sources.clone())
}

fn mod_assets(
    client: &Client,
    mod_entry: &ModEntry,
    selected_source_options: &HashMap<String, String>,
) -> io::Result<Vec<DownloadAsset>> {
    let mut all_assets = Vec::new();
    for source in mod_sources(mod_entry, selected_source_options)?.iter() {
        let mut source_assets = match source.source {
            ModSource::Github => {
                github_release_assets(client, source.id.as_str(), source.version.as_ref())
            }
            ModSource::Gamebanana => {
                gamebanana_latest_asset(client, source.id.as_str(), source.version.as_ref())
            }
        }?;
        all_assets.append(&mut source_assets);
    }
    Ok(all_assets)
}

fn fetch_and_extract_assets(
    client: &Client,
    mod_entry: &ModEntry,
    download_dir: &Path,
    extracted_dir: &Path,
    selected_source_options: &HashMap<String, String>,
) -> io::Result<()> {
    fs::create_dir_all(download_dir)?;
    fs::create_dir_all(extracted_dir)?;

    let assets = mod_assets(client, mod_entry, selected_source_options)?;
    for asset in assets.iter() {
        let safe_name = Path::new(asset.name.as_str())
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(io_error!(
                InvalidFilename,
                "Invalid asset name '{}' for mod {}",
                asset.name,
                mod_entry.name
            ))?;

        let downloaded_file = download_dir.join(safe_name);
        log::info!(
            "Downloading mod asset {} from {}",
            downloaded_file.to_string_lossy(),
            asset.url
        );
        download_asset(client, asset.url.as_str(), downloaded_file.as_path())?;

        let extension = downloaded_file
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();

        if extension == "zip" || extension == "7z" {
            extract_archive_contents_to_dir(downloaded_file.as_path(), extracted_dir)?;
        } else {
            let extracted_file = extracted_dir.join(safe_name);
            fs::copy(downloaded_file.as_path(), extracted_file.as_path())?;
        }
    }

    Ok(())
}

fn collect_atmosphere_app_ids(manifest: &ModsUpdateManifest) -> HashSet<String> {
    let mut app_ids = HashSet::new();

    for mod_entry in manifest.mods.iter() {
        for (_, destination) in mod_entry.install_to.iter() {
            let mut components = Path::new(destination).components();
            let first = components.next();
            let second = components.next();
            let third = components.next();
            if let (Some(a), Some(c), Some(app_id)) = (first, second, third) {
                if a.as_os_str() == "atmosphere"
                    && c.as_os_str() == "contents"
                    && !app_id.as_os_str().is_empty()
                {
                    app_ids.insert(app_id.as_os_str().to_string_lossy().to_string());
                }
            }
        }
    }

    app_ids
}

fn resolve_mod_install_order(
    manifest: &ModsUpdateManifest,
    selected_mod_names: Option<HashSet<String>>,
) -> io::Result<ModsUpdateManifest> {
    let mut by_name: HashMap<String, ModEntry> = HashMap::new();
    for mod_entry in manifest.mods.iter() {
        by_name.insert(mod_entry.name.clone(), mod_entry.clone());
    }

    let requested_mods: Vec<String> = match selected_mod_names {
        Some(names) => names.into_iter().collect(),
        None => manifest
            .mods
            .iter()
            .filter(|mod_entry| mod_entry.enabled == Some(true))
            .map(|mod_entry| mod_entry.name.clone())
            .collect(),
    };

    let mut state: HashMap<String, u8> = HashMap::new();
    let mut ordered_names = Vec::new();

    fn visit(
        mod_name: &str,
        by_name: &HashMap<String, ModEntry>,
        state: &mut HashMap<String, u8>,
        ordered_names: &mut Vec<String>,
    ) -> io::Result<()> {
        let current_state = state.get(mod_name).copied().unwrap_or(0);
        if current_state == 2 {
            return Ok(());
        }
        if current_state == 1 {
            return Err(io_error!(
                InvalidData,
                "Circular mod dependency detected at {}",
                mod_name
            ));
        }

        let mod_entry = by_name.get(mod_name).ok_or(io_error!(
            InvalidData,
            "Mod dependency '{}' is not defined in manifest",
            mod_name
        ))?;

        state.insert(mod_name.to_string(), 1);
        for dependency in mod_entry.dependencies.iter() {
            visit(dependency.as_str(), by_name, state, ordered_names)?;
        }
        state.insert(mod_name.to_string(), 2);
        ordered_names.push(mod_name.to_string());
        Ok(())
    }

    for mod_name in requested_mods.iter() {
        visit(mod_name.as_str(), &by_name, &mut state, &mut ordered_names)?;
    }

    let mut ordered_mods = Vec::new();
    for mod_name in ordered_names.into_iter() {
        if let Some(mod_entry) = by_name.get(mod_name.as_str()) {
            ordered_mods.push(mod_entry.clone());
        }
    }

    Ok(ModsUpdateManifest { mods: ordered_mods })
}

fn install_remote_mods(
    manifest: &ModsUpdateManifest,
    sdmc_path: &Path,
    selected_source_options: &HashMap<String, String>,
    status_callback: Option<&dyn Fn(&str, usize, usize)>,
) -> io::Result<()> {
    let client = build_http_client()?;
    let temp_root = TempDirGuard::new("ssbu-mod-assets")?;
    let total_mods = manifest.mods.len();

    for (index, mod_entry) in manifest.mods.iter().enumerate() {
        if let Some(callback) = status_callback {
            let status = format!(
                "Installing mod {} ({}/{})",
                mod_entry.name,
                index + 1,
                total_mods
            );
            callback(status.as_str(), index + 1, total_mods);
        }
        log::info!("Installing mod {} ({})", mod_entry.name, index + 1);
        let mod_work_dir = temp_root.path().join(format!("mod-{}", index));
        let download_dir = mod_work_dir.join("downloads");
        let assets_dir = mod_work_dir.join("assets");

        fetch_and_extract_assets(
            &client,
            mod_entry,
            download_dir.as_path(),
            assets_dir.as_path(),
            selected_source_options,
        )?;
        install_mod_files(mod_entry, assets_dir.as_path(), sdmc_path)?;
    }

    Ok(())
}

fn selected_mod_names_from_cleanup_options(
    cleanup_options: &[CleanupOption],
) -> Option<HashSet<String>> {
    cleanup_options.iter().find_map(|option| match option {
        CleanupOption::EnableMods(ids) => Some(ids.iter().cloned().collect::<HashSet<String>>()),
        _ => None,
    })
}

fn selected_mod_source_options_from_cleanup_options(
    cleanup_options: &[CleanupOption],
) -> HashMap<String, String> {
    let mut selected = HashMap::new();
    for option in cleanup_options.iter() {
        if let CleanupOption::SelectModSources(entries) = option {
            for SelectedModSourceOption {
                mod_id,
                source_option,
            } in entries.iter()
            {
                selected.insert(mod_id.clone(), source_option.clone());
            }
        }
    }
    selected
}

fn install_manifest_from_cleanup_options(
    client: &Client,
    cleanup_options: &[CleanupOption],
) -> io::Result<(ModsUpdateManifest, ModsUpdateManifest, HashMap<String, String>)> {
    let manifest = load_mod_manifest(client)?;
    let selected_mod_names = selected_mod_names_from_cleanup_options(cleanup_options);
    let selected_mod_sources = selected_mod_source_options_from_cleanup_options(cleanup_options);
    let install_manifest = resolve_mod_install_order(&manifest, selected_mod_names)?;
    Ok((manifest, install_manifest, selected_mod_sources))
}

pub fn optimize_settings(config: &OptimizerConfig, user_profile: &UserProfile) -> io::Result<()> {
    let config_folder = config.emu_filesystem.config_folder.as_ref();
    let emu_type = config.get_emulator_type();
    let emu_name = config.get_emulator_name();

    let ssbu_settings_path_src = BUNDLED_SSBU_SETTINGS
        .get_dir(emu_name.as_str())
        .or_else(|| BUNDLED_SSBU_SETTINGS.get_dir("common"))
        .ok_or(io_error!(
            NotFound,
            "Bundled custom SSBU settings not found"
        ))?;
    let ssbu_settings_path_dest = config_folder
        .ok_or(io_error!(NotFound, "Emulator config folder not found"))?
        .join("custom");
    load_bundled_dir(&ssbu_settings_path_src, ssbu_settings_path_dest)?;

    if emu_type == EmuType::Yuzu {
        let main_config_settings_path = config_folder
            .ok_or(io_error!(NotFound, "Emulator config folder not found"))?
            .join("qt-config.ini");
        let mut main_config = Ini::load_from_file_noescape(main_config_settings_path.as_path())
            .ok()
            .ok_or(io_error!(NotFound, "Unable to load main config"))?;
        let section = main_config
            .section_mut(Some("WebService"))
            .ok_or(io_error!(
                NotFound,
                "Unable to find WebService section in config"
            ))?;
        section.insert("enable_telemetry\\default", "false");
        section.insert("enable_telemetry", "false");
        section.insert("web_api_url\\default", "false");
        section.insert("web_api_url", "api.ynet-fun.xyz");
        section.insert(format!("{}_username\\default", emu_name), "false");
        section.insert(format!("{}_username", emu_name), user_profile.name.as_str());
        section.insert(format!("{}_token\\default", emu_name), "false");
        section.insert(
            format!("{}_token", emu_name),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );
        main_config
            .write_to_file_policy(
                main_config_settings_path.as_path(),
                ini::EscapePolicy::Nothing,
            )
            .ok()
            .ok_or(io_error!(NotFound, "Unable to save main config settings"))?;
    }
    Ok(())
}

pub fn optimize_mods(
    config: &OptimizerConfig,
    _user_profile: &UserProfile,
    cleanup_options: Vec<CleanupOption>,
    status_callback: Option<&dyn Fn(&str, usize, usize)>,
) -> io::Result<()> {
    let client = build_http_client()?;
    let (manifest, install_manifest, selected_mod_sources) =
        install_manifest_from_cleanup_options(&client, cleanup_options.as_slice())?;

    let sdmc_path = config
        .emu_filesystem
        .sdmc_folder
        .as_ref()
        .ok_or(io_error!(NotFound, "Unable to find sdmc directory"))?;
    let atmosphere_apps_path = sdmc_path.join("atmosphere").join("contents");
    if cleanup_options.contains(&CleanupOption::CleanAtmosphereAppDir)
        && atmosphere_apps_path.is_dir()
    {
        log::info!("Removing atmosphere files listed in manifest...");
        for app_id in collect_atmosphere_app_ids(&manifest).iter() {
            let app_dir = atmosphere_apps_path.join(app_id);
            if app_dir.is_dir() {
                log::info!("Removing atmosphere dir: {}", app_dir.to_string_lossy());
                fs::remove_dir_all(app_dir.as_path())?;
            }
        }
    }

    let ultimate_dir_path = sdmc_path.join("ultimate");
    if cleanup_options.contains(&CleanupOption::CleanUltimateDir) && ultimate_dir_path.is_dir() {
        log::info!("Removing ultimate files...");
        fs::remove_dir_all(ultimate_dir_path.as_path())?;
    }

    install_remote_mods(
        &install_manifest,
        sdmc_path.as_path(),
        &selected_mod_sources,
        status_callback,
    )?;

    Ok(())
}

pub fn generate_sdcard_folder(
    output_dir: &Path,
    cleanup_options: Vec<CleanupOption>,
    status_callback: Option<&dyn Fn(&str, usize, usize)>,
) -> io::Result<PathBuf> {
    if let Some(parent) = output_dir.parent() {
        fs::create_dir_all(parent)?;
    }

    if output_dir.is_dir() {
        fs::remove_dir_all(output_dir)?;
    }
    fs::create_dir_all(output_dir)?;

    let client = build_http_client()?;
    let (_, install_manifest, selected_mod_sources) =
        install_manifest_from_cleanup_options(&client, cleanup_options.as_slice())?;

    install_remote_mods(
        &install_manifest,
        output_dir,
        &selected_mod_sources,
        status_callback,
    )?;

    Ok(output_dir.to_path_buf())
}

pub fn list_manifest_mods() -> io::Result<Vec<ManifestModEntry>> {
    let client = build_http_client()?;
    let manifest = load_mod_manifest(&client)?;
    Ok(manifest
        .mods
        .into_iter()
        .filter_map(|mod_entry| {
            let source_options = if mod_entry.sources_options.is_empty() {
                vec![]
            } else {
                mod_entry
                    .sources_options
                    .iter()
                    .map(|option| option.name.clone())
                    .collect::<Vec<String>>()
            };
            let default_source_option = source_options.first().cloned();
            mod_entry.enabled.map(|enabled| ManifestModEntry {
                id: mod_entry.name.clone(),
                name: mod_entry.name,
                description: mod_entry.description,
                enabled,
                source_options,
                default_source_option,
            })
        })
        .collect())
}

pub fn optimize_save(config: &OptimizerConfig, user_profile: &UserProfile) -> io::Result<()> {
    let save_file_path = config.get_save_folder(user_profile)?;
    load_bundled_dir(&BUNDLED_SAVE_DATA, save_file_path)?;
    Ok(())
}
