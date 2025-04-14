// src-tauri/src/lib.rs
mod github_repo;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tar::Archive;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;
use walkdir::WalkDir;
use zip::ZipArchive;

// use tauri::WebviewUrl;
// use tauri::WebviewWindowBuilder;
// use std::collections::HashMap;
//
use std::collections::HashSet;
use std::fs::File;
// use std::panic;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use std::{fs, io::Cursor};

use bmm_lib::balamod::find_balatros;
use bmm_lib::cache;
use bmm_lib::cache::Mod;
use bmm_lib::database::Database;
use bmm_lib::database::InstalledMod;
use bmm_lib::discord_rpc::DiscordRpcManager;
use bmm_lib::errors::AppError;
use bmm_lib::finder::get_lovely_mods_dir;
use bmm_lib::finder::is_balatro_running;
use bmm_lib::finder::is_steam_running;
use bmm_lib::local_mod_detection;
use bmm_lib::lovely;
use bmm_lib::smods_installer::{ModInstaller, ModType};

fn map_error<T>(result: Result<T, AppError>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}

// Create a state structure to hold the database
struct AppState {
    db: Mutex<Database>,
    discord_rpc: Mutex<DiscordRpcManager>,
}

#[derive(Clone, serde::Serialize)]
struct Payload {
    args: Vec<String>,
    cwd: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModMeta {
    #[serde(rename = "requires-steamodded")]
    pub requires_steamodded: bool,
    #[serde(rename = "requires-talisman")]
    pub requires_talisman: bool,
    pub categories: Vec<String>,
    pub author: String,
    pub repo: String,
    pub title: String,
    #[serde(rename = "downloadURL")]
    pub download_url: Option<String>,
    #[serde(rename = "folderName", default)]
    pub folder_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(rename = "automatic-version-check", default)]
    automatic_version_check: bool,
}

#[tauri::command]
async fn check_steam_running() -> bool {
    is_steam_running()
}

#[tauri::command]
async fn check_balatro_running() -> bool {
    is_balatro_running()
}

#[tauri::command]
async fn save_versions_cache(mod_type: String, versions: Vec<String>) -> Result<(), String> {
    map_error(cache::save_versions_cache(&mod_type, &versions))
}

#[tauri::command]
async fn mod_update_available(
    mod_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let last_installed_version = db
        .get_last_installed_version(&mod_name)
        .map_err(|e| e.to_string())?;

    // If no version is installed, we can't determine if an update is available
    if last_installed_version.is_empty() {
        return Ok(false);
    }

    // Try to get the cached mods
    let cached_mods = match crate::cache::load_cache().map_err(|e| e.to_string())? {
        Some((mods, _)) => mods,
        None => return Ok(false), // No cache available
    };

    // Look for the mod in the cache by matching either title or folderName
    for cached_mod in cached_mods {
        if cached_mod.title == mod_name || (cached_mod.folderName.as_ref() == Some(&mod_name)) {
            // If we found a match and it has a version, compare versions
            if let Some(remote_version) = cached_mod.version {
                // If versions are different, consider an update available
                return Ok(remote_version != last_installed_version);
            }
            break; // Found the mod but it has no version info
        }
    }

    // No update found or couldn't determine
    Ok(false)
}

#[tauri::command]
async fn get_repo_path() -> Result<String, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")).to_string())?;
    let repo_path = config_dir.join("Balatro").join("mod_index");
    Ok(repo_path.to_string_lossy().into_owned())
}

#[tauri::command]
async fn clone_repo(url: &str, path: &str) -> Result<(), String> {
    github_repo::clone_repository(url, path).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModCacheInfo {
    pub path: String,
    pub last_commit: i64,
}

#[allow(non_snake_case)]
#[tauri::command]
async fn get_mod_thumbnail(modPath: String) -> Result<Option<String>, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")).to_string())?;

    let full_path = config_dir
        .join("Balatro")
        .join("mod_index")
        .join("mods")
        .join(modPath)
        .join("thumbnail.jpg");

    // Read the image file
    let image_data = match std::fs::read(&full_path) {
        Ok(data) => data,
        Err(_) => {
            return Ok(None);
        }
    };

    // Convert to base64
    let base64 = STANDARD.encode(image_data);
    Ok(Some(format!("data:image/jpeg;base64,{}", base64)))
}

#[tauri::command]
async fn pull_repo(path: &str) -> Result<(), String> {
    // Check if directory exists
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err(format!("Directory '{}' does not exist", path));
    }

    // Check if it's a repository
    if !github_repo::is_repository_directory(path) {
        // Auto-clone if it doesn't look like a repository
        let repo_url = "https://github.com/skyline69/balatro-mod-index"; // Default repository URL
        return github_repo::clone_repository(repo_url, path).await;
    }

    // Proceed with pull if it's a valid repository
    github_repo::pull_repository(path).await
}

#[tauri::command]
async fn list_directories(path: &str) -> Result<Vec<String>, String> {
    let dir = PathBuf::from(path);
    let entries = std::fs::read_dir(dir).map_err(|e| {
        AppError::FileRead {
            path: PathBuf::from(path),
            source: e.to_string(),
        }
        .to_string()
    })?;

    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| {
            AppError::FileRead {
                path: PathBuf::from(path),
                source: e.to_string(),
            }
            .to_string()
        })?;

        if let Ok(file_type) = entry.file_type() {
            if file_type.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    dirs.push(name.to_string());
                }
            }
        }
    }
    Ok(dirs)
}

#[tauri::command]
async fn read_json_file(path: &str) -> Result<ModMeta, String> {
    let path = PathBuf::from(path);
    let file = File::open(&path).map_err(|e| {
        AppError::FileRead {
            path: path.clone(),
            source: e.to_string(),
        }
        .to_string()
    })?;

    serde_json::from_reader(file).map_err(|e| {
        AppError::JsonParse {
            path,
            source: e.to_string(),
        }
        .to_string()
    })
}

#[tauri::command]
async fn read_text_file(path: &str) -> Result<String, String> {
    let path = PathBuf::from(path);
    std::fs::read_to_string(&path).map_err(|e| {
        AppError::FileRead {
            path,
            source: e.to_string(),
        }
        .to_string()
    })
}

#[tauri::command]
async fn get_last_fetched(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_last_fetched().map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_last_fetched(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_last_fetched(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_versions_cache(mod_type: String) -> Result<Option<(Vec<String>, u64)>, String> {
    cache::load_versions_cache(&mod_type)
        .map(|res| {
            res.map(|versions| {
                (
                    versions,
                    match SystemTime::now().duration_since(UNIX_EPOCH) {
                        Ok(dur) => dur,
                        Err(e) => {
                            log::error!("Failed to get current time: {}", e);
                            std::process::exit(1);
                        }
                    }
                    .as_secs(),
                )
            })
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_mods_cache(mods: Vec<Mod>) -> Result<(), String> {
    map_error(cache::save_cache(&mods))
}

#[tauri::command]
async fn clear_cache() -> Result<(), String> {
    map_error(cache::clear_cache())
}

#[tauri::command]
fn open_directory(path: String) -> Result<(), String> {
    match open::that(path) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to open directory: {}", e)),
    }
}

#[tauri::command]
async fn load_mods_cache() -> Result<Option<(Vec<Mod>, u64)>, String> {
    map_error(cache::load_cache())
}

#[tauri::command]
async fn get_lovely_console_status(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    map_error(db.is_lovely_console_enabled())
}

#[tauri::command]
async fn set_lovely_console_status(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    map_error(db.set_lovely_console_status(enabled))
}

#[tauri::command]
async fn check_untracked_mods() -> Result<bool, String> {
    // Always return false since we're not removing untracked files
    Ok(false)
}

#[tauri::command]
async fn get_mods_folder(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    #[cfg(not(target_os = "linux"))]
    let mods_dir = get_lovely_mods_dir(None);
    #[cfg(target_os = "linux")]
    let mods_dir = get_lovely_mods_dir(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
    );
    Ok(mods_dir.to_string_lossy().into_owned())
}

#[tauri::command]
async fn is_mod_enabled(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
    mod_name: String,
) -> Result<bool, String> {
    #[cfg(not(target_os = "linux"))]
    let mods_dir = get_lovely_mods_dir(None);
    #[cfg(target_os = "linux")]
    let mods_dir = get_lovely_mods_dir(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
    );

    let mod_dir = mods_dir.join(&mod_name);
    let ignore_file_path = mod_dir.join(".lovelyignore");

    Ok(!ignore_file_path.exists())
}

#[tauri::command]
async fn toggle_mod_enabled(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
    mod_name: String,
    enabled: bool,
) -> Result<(), String> {
    #[cfg(not(target_os = "linux"))]
    let mods_dir = get_lovely_mods_dir(None);
    #[cfg(target_os = "linux")]
    let mods_dir = get_lovely_mods_dir(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
    );

    let mod_dir = mods_dir.join(&mod_name);

    if !mod_dir.exists() {
        return Err(format!("Mod directory not found: {}", mod_name));
    }

    let ignore_file_path = mod_dir.join(".lovelyignore");

    if enabled {
        if ignore_file_path.exists() {
            fs::remove_file(&ignore_file_path)
                .map_err(|e| format!("Failed to remove .lovelyignore file: {}", e))?;
        }
    } else {
        fs::write(&ignore_file_path, "")
            .map_err(|e| format!("Failed to create .lovelyignore file: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
async fn is_mod_enabled_by_path(mod_path: String) -> Result<bool, String> {
    let path = PathBuf::from(&mod_path);

    if !path.exists() {
        return Err(format!("Mod path does not exist: {}", mod_path));
    }

    let ignore_file_path = path.join(".lovelyignore");

    Ok(!ignore_file_path.exists())
}

#[tauri::command]
async fn toggle_mod_enabled_by_path(mod_path: String, enabled: bool) -> Result<(), String> {
    let path = PathBuf::from(&mod_path);

    if !path.exists() {
        return Err(format!("Mod path does not exist: {}", mod_path));
    }

    let ignore_file_path = path.join(".lovelyignore");

    if enabled {
        if ignore_file_path.exists() {
            fs::remove_file(&ignore_file_path)
                .map_err(|e| format!("Failed to remove .lovelyignore file: {}", e))?;
        }
    } else {
        fs::write(&ignore_file_path, "")
            .map_err(|e| format!("Failed to create .lovelyignore file: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
async fn process_dropped_file(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    #[cfg(not(target_os = "linux"))]
    let mods_dir = get_lovely_mods_dir(None);
    #[cfg(target_os = "linux")]
    let mods_dir = get_lovely_mods_dir(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
    );

    fs::create_dir_all(&mods_dir).map_err(|e| format!("Failed to create mods directory: {}", e))?;

    let file_path = Path::new(&path);
    let file_name = file_path
        .file_name()
        .ok_or_else(|| "Invalid file path".to_string())?
        .to_str()
        .ok_or_else(|| "Invalid file name".to_string())?;

    let mod_name = file_name
        .trim_end_matches(".zip")
        .trim_end_matches(".tar")
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".tgz");

    let mod_dir = mods_dir.join(mod_name);

    if mod_dir.exists() {
        fs::remove_dir_all(&mod_dir)
            .map_err(|e| format!("Failed to remove existing mod directory: {}", e))?;
    }

    if file_name.ends_with(".zip") {
        extract_zip(&path, &mod_dir)?;
    } else if file_name.ends_with(".tar") {
        extract_tar(&path, &mod_dir)?;
    } else if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        extract_tar_gz(&path, &mod_dir)?;
    } else {
        return Err(
            "Unsupported file format. Only ZIP, TAR, and TAR.GZ are supported.".to_string(),
        );
    }

    if let Ok(entries) = fs::read_dir(&mod_dir) {
        let dirs: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();

        if dirs.len() == 1 && fs::read_dir(&mod_dir).map(|e| e.count()).unwrap_or(0) == 1 {
            let nested_dir = dirs[0].path();

            for entry in fs::read_dir(&nested_dir)
                .map_err(|e| format!("Failed to read nested directory: {}", e))?
            {
                let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
                let target_path = mod_dir.join(entry.file_name());

                if entry
                    .file_type()
                    .map_err(|e| format!("Failed to get file type: {}", e))?
                    .is_dir()
                {
                    fs::rename(entry.path(), &target_path)
                        .map_err(|e| format!("Failed to move directory: {}", e))?;
                } else {
                    fs::rename(entry.path(), &target_path)
                        .map_err(|e| format!("Failed to move file: {}", e))?;
                }
            }

            fs::remove_dir_all(&nested_dir)
                .map_err(|e| format!("Failed to remove nested directory: {}", e))?;
        }
    }

    let has_lua_files = check_for_lua_files(&mod_dir)?;

    if !has_lua_files {
        fs::remove_dir_all(&mod_dir)
            .map_err(|e| format!("Failed to remove invalid mod directory: {}", e))?;
        return Err(
            "No Lua files found in the archive. This doesn't appear to be a valid Balatro mod."
                .to_string(),
        );
    }

    Ok(mod_dir.to_string_lossy().to_string())
}

#[tauri::command]
fn process_mod_archive(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
    filename: String,
    data: Vec<u8>,
) -> Result<String, String> {
    #[cfg(not(target_os = "linux"))]
    let mods_dir = get_lovely_mods_dir(None);
    #[cfg(target_os = "linux")]
    let mods_dir = get_lovely_mods_dir(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
    );

    fs::create_dir_all(&mods_dir).map_err(|e| format!("Failed to create mods directory: {}", e))?;

    let mod_name = filename
        .trim_end_matches(".zip")
        .trim_end_matches(".tar")
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".tgz");

    let mod_dir = mods_dir.join(mod_name);

    if mod_dir.exists() {
        fs::remove_dir_all(&mod_dir)
            .map_err(|e| format!("Failed to remove existing mod directory: {}", e))?;
    }

    let cursor = Cursor::new(data);

    if filename.ends_with(".zip") {
        extract_zip_from_memory(cursor, &mod_dir)?;
    } else if filename.ends_with(".tar") {
        extract_tar_from_memory(cursor, &mod_dir)?;
    } else if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
        extract_tar_gz_from_memory(cursor, &mod_dir)?;
    } else {
        return Err(
            "Unsupported file format. Only ZIP, TAR, and TAR.GZ are supported.".to_string(),
        );
    }

    if let Ok(entries) = fs::read_dir(&mod_dir) {
        let dirs: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();

        if dirs.len() == 1 && fs::read_dir(&mod_dir).map(|e| e.count()).unwrap_or(0) == 1 {
            let nested_dir = dirs[0].path();

            for entry in fs::read_dir(&nested_dir)
                .map_err(|e| format!("Failed to read nested directory: {}", e))?
            {
                let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
                let target_path = mod_dir.join(entry.file_name());

                if entry
                    .file_type()
                    .map_err(|e| format!("Failed to get file type: {}", e))?
                    .is_dir()
                {
                    fs::rename(entry.path(), &target_path)
                        .map_err(|e| format!("Failed to move directory: {}", e))?;
                } else {
                    fs::rename(entry.path(), &target_path)
                        .map_err(|e| format!("Failed to move file: {}", e))?;
                }
            }

            fs::remove_dir_all(&nested_dir)
                .map_err(|e| format!("Failed to remove nested directory: {}", e))?;
        }
    }

    Ok(mod_dir.to_string_lossy().to_string())
}

// Helper function to check for .lua files
fn check_for_lua_files(dir: &PathBuf) -> Result<bool, String> {
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension == "lua" {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

// Helper function to extract ZIP files from disk
fn extract_zip(path: &str, target_dir: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {}", e))?;
    let file = fs::File::open(path).map_err(|e| format!("Failed to open ZIP file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to open ZIP archive: {}", e))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to access file in archive: {}", e))?;
        let file_path = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };
        let output_path = target_dir.join(&file_path);
        if file.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directory: {}", e))?;
            }
            let mut outfile = fs::File::create(&output_path)
                .map_err(|e| format!("Failed to create file {}: {}", output_path.display(), e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write file {}: {}", output_path.display(), e))?;
        }
    }
    Ok(())
}

// Helper function to extract ZIP files from memory
fn extract_zip_from_memory(cursor: Cursor<Vec<u8>>, target_dir: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {}", e))?;
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("Failed to open ZIP archive: {}", e))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to access file in archive: {}", e))?;
        let file_path = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };
        let output_path = target_dir.join(&file_path);
        if file.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directory: {}", e))?;
            }
            let mut outfile = fs::File::create(&output_path)
                .map_err(|e| format!("Failed to create file {}: {}", output_path.display(), e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write file {}: {}", output_path.display(), e))?;
        }
    }
    Ok(())
}

// Helper function to extract TAR files from disk
fn extract_tar(path: &str, target_dir: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {}", e))?;
    let file = fs::File::open(path).map_err(|e| format!("Failed to open TAR file: {}", e))?;
    let mut archive = Archive::new(file);
    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read TAR entries: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read TAR entry: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {}", e))?;
        let output_path = target_dir.join(path);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent directory: {}", e))?;
        }
        entry
            .unpack(&output_path)
            .map_err(|e| format!("Failed to unpack file {}: {}", output_path.display(), e))?;
    }
    Ok(())
}

// Helper function to extract TAR files from memory
fn extract_tar_from_memory(cursor: Cursor<Vec<u8>>, target_dir: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {}", e))?;
    let mut archive = Archive::new(cursor);
    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read TAR entries: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read TAR entry: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {}", e))?;
        let output_path = target_dir.join(path);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent directory: {}", e))?;
        }
        entry
            .unpack(&output_path)
            .map_err(|e| format!("Failed to unpack file {}: {}", output_path.display(), e))?;
    }
    Ok(())
}

// Helper function to extract TAR.GZ files from disk
fn extract_tar_gz(path: &str, target_dir: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {}", e))?;
    let file = fs::File::open(path).map_err(|e| format!("Failed to open TAR.GZ file: {}", e))?;
    let gz = GzDecoder::new(file);
    let mut archive = Archive::new(gz);
    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read TAR entries: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read TAR entry: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {}", e))?;
        let output_path = target_dir.join(path);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent directory: {}", e))?;
        }
        entry
            .unpack(&output_path)
            .map_err(|e| format!("Failed to unpack file {}: {}", output_path.display(), e))?;
    }
    Ok(())
}

// Helper function to extract TAR.GZ files from memory
fn extract_tar_gz_from_memory(cursor: Cursor<Vec<u8>>, target_dir: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory: {}", e))?;
    let gz = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz);
    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read TAR entries: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read TAR entry: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {}", e))?;
        let output_path = target_dir.join(path);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent directory: {}", e))?;
        }
        entry
            .unpack(&output_path)
            .map_err(|e| format!("Failed to unpack file {}: {}", output_path.display(), e))?;
    }
    Ok(())
}

#[tauri::command]
async fn refresh_mods_folder(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    let installed_mods = db.get_installed_mods()?;

    #[cfg(not(target_os = "linux"))]
    let mods_dir = get_lovely_mods_dir(None);
    #[cfg(target_os = "linux")]
    let mods_dir = get_lovely_mods_dir(db.get_installation_path()?.as_ref());

    let entries = std::fs::read_dir(&mods_dir).map_err(|e| AppError::FileRead {
        path: mods_dir.clone(),
        source: e.to_string(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| AppError::FileRead {
            path: mods_dir.clone(),
            source: e.to_string(),
        })?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::InvalidState("Invalid filename".to_string()))?;

        if name.contains(".lovely") || name.contains("lovely") {
            continue;
        }

        let ft = entry.file_type().map_err(|e| AppError::FileRead {
            path: path.clone(),
            source: e.to_string(),
        })?;

        match (ft.is_dir(), ft.is_file()) {
            (true, _) => {
                if !installed_mods.iter().any(|m| m.path.contains(name)) {
                    std::fs::remove_dir_all(&path).map_err(|e| AppError::FileWrite {
                        path: path.clone(),
                        source: e.to_string(),
                    })?;
                }
            }
            (_, true) => {
                if !installed_mods.iter().any(|m| m.path.contains(name)) {
                    std::fs::remove_file(&path).map_err(|e| AppError::FileWrite {
                        path: path.clone(),
                        source: e.to_string(),
                    })?;
                }
            }
            _ => continue,
        }
    }
    Ok(())
}

#[tauri::command]
async fn get_discord_rpc_status(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    map_error(db.is_discord_rpc_enabled())
}

#[tauri::command]
async fn set_discord_rpc_status(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    map_error(db.set_discord_rpc_enabled(enabled))?;
    let discord_rpc = state
        .discord_rpc
        .lock()
        .map_err(|_| AppError::LockPoisoned("Discord RPC lock poisoned".to_string()))?;
    discord_rpc.set_enabled(enabled);
    Ok(())
}

#[tauri::command]
async fn launch_balatro(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let (path_str, lovely_console_enabled) = {
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
        (
            db.get_installation_path()?
                .ok_or_else(|| AppError::InvalidState("No installation path set".to_string()))?,
            db.is_lovely_console_enabled()?,
        )
    };
    let path = PathBuf::from(path_str);

    #[cfg(target_os = "macos")]
    {
        let lovely_path = map_error(lovely::ensure_lovely_exists().await)?;
        let balatro_executable = path.join("Balatro.app/Contents/MacOS/love");
        if lovely_console_enabled {
            let disable_arg = if !lovely_console_enabled {
                " --disable-console"
            } else {
                ""
            };
            let command_line = format!(
                "cd '{}' && DYLD_INSERT_LIBRARIES='{}' '{}'{}",
                path.display(),
                lovely_path.display(),
                balatro_executable.display(),
                disable_arg
            );
            let applescript = format!(
                "tell application \"Terminal\" to do script \"{}\"",
                command_line
            );
            Command::new("osascript")
                .arg("-e")
                .arg(applescript)
                .spawn()
                .map_err(|e| AppError::ProcessExecution(e.to_string()))?;
        } else {
            let mut command = Command::new(path.join("Balatro.app/Contents/MacOS/love"));
            command
                .env("DYLD_INSERT_LIBRARIES", lovely_path)
                .current_dir(&path);
            command
                .spawn()
                .map_err(|e| AppError::ProcessExecution(e.to_string()))?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        let exe_path = find_executable_in_directory(&path)
            .ok_or_else(|| format!("No executable found in {}", path.display()))?;
        let dll_path = path.join("version.dll");
        if !dll_path.exists() {
            lovely::ensure_version_dll_exists(&path)
                .await
                .inspect_err(|_| log::error!("Failed to install `lovely`"))?;
        }
        if lovely_console_enabled {
            Command::new(&exe_path)
                .current_dir(&path)
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {}", exe_path.display(), e))?;
        } else {
            Command::new(&exe_path)
                .current_dir(&path)
                .arg("--disable-console")
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {}", exe_path.display(), e))?;
        }
        log::debug!("Launched game from {}", exe_path.display());
    }

    #[cfg(target_os = "linux")]
    {
        let dll_path = path.join("version.dll");
        if !dll_path.exists() {
            lovely::ensure_version_dll_exists(&path)
                .await
                .inspect_err(|_| log::error!("Failed to install `lovely`"))?;
        }
        let app_id = "2379780";
        let url_handler = Command::new("xdg-mime")
            .arg("query")
            .arg("default")
            .arg("x-scheme-handler/steam")
            .output()
            .map_err(|e| format!("Failed to query `steam://` handler: {}", e))
            .and_then(|output| {
                let output = String::from_utf8_lossy(&output.stdout);
                if output.trim().is_empty() {
                    return Err("No default `steam://` handler found".to_string());
                }
                if output.trim() != "steam.desktop" {
                    log::warn!(
                        "The system's default `steam://` handler is {} instead of steam",
                        output
                    );
                }
                Ok(())
            });
        if url_handler.is_ok() {
            let _ = Command::new("xdg-open")
                .arg(format!("steam://run/{}", app_id))
                .spawn();
            log::debug!("Launched Balatro through Steam URL protocol");
            return Ok(());
        }
        let steam_result = which::which("steam").map(|steam_path| {
            let mut command = Command::new(steam_path);
            if !lovely_console_enabled {
                command.args(["-applaunch", app_id, "--", "--disable-console"]);
            } else {
                command.args(["-applaunch", app_id]);
            }
            command.spawn()
        });
        if let Ok(Ok(_)) = steam_result {
            log::debug!("Launched Balatro through Steam executable");
            return Ok(());
        }
        let exe_path = find_executable_in_directory(&path)
            .ok_or_else(|| format!("No executable found in {}", path.display()))?;
        let mut command = Command::new(&exe_path);
        command
            .current_dir(&path)
            .env("WINEDLLOVERRIDES", "version=n,b");
        if !lovely_console_enabled {
            command.arg("--disable-console");
        }
        command
            .spawn()
            .map_err(|e| format!("Failed to launch {}: {}", exe_path.display(), e))?;
        log::debug!("Launched Balatro directly with WINEDLLOVERRIDES");
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn find_executable_in_directory(dir: &PathBuf) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut executables: Vec<PathBuf> = Vec::new();
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = std::fs::metadata(&path) {
                    use std::os::unix::fs::PermissionsExt;
                    let permissions = metadata.permissions();
                    if permissions.mode() & 0o111 != 0 {
                        executables.push(path);
                    }
                }
            }
        }
        if executables.is_empty() {
            return None;
        }
        for exe in &executables {
            if let Some(file_name) = exe.file_name().and_then(|n| n.to_str()) {
                if file_name.to_lowercase().contains("balatro") {
                    return Some(exe.clone());
                }
            }
        }
        for exe in &executables {
            if let Some(file_name) = exe.file_name().and_then(|n| n.to_str()) {
                if file_name.to_lowercase() == "love" {
                    return Some(exe.clone());
                }
            }
        }
        return Some(executables[0].clone());
    }
    None
}

#[cfg(target_os = "windows")]
fn find_executable_in_directory(dir: &PathBuf) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut executables: Vec<PathBuf> = Vec::new();
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && path.extension() == Some(std::ffi::OsStr::new("exe")) {
                executables.push(path);
            }
        }
        if executables.is_empty() {
            return None;
        }
        for exe in &executables {
            if let Some(file_name) = exe.file_name().and_then(|n| n.to_str()) {
                if file_name.to_lowercase().contains("balatro") {
                    return Some(exe.clone());
                }
            }
        }
        return Some(executables[0].clone());
    }
    None
}

#[tauri::command]
async fn check_mod_installation(mod_type: String) -> Result<bool, String> {
    let db = map_error(Database::new())?;
    let installed_mods = map_error(db.get_installed_mods())?;
    let cached_mods = match cache::load_cache() {
        Ok(Some((mods, _))) => mods,
        _ => Vec::new(),
    };
    let detected_mods = local_mod_detection::detect_manual_mods(&db, &cached_mods)?;
    let mod_name = mod_type.as_str();
    match mod_name {
        "Steamodded" | "Talisman" => Ok(installed_mods.iter().any(|m| m.name == mod_name)
            || detected_mods.iter().any(|m| m.name == mod_name)),
        _ => Err(AppError::InvalidState("Invalid mod type".to_string()).to_string()),
    }
}

#[tauri::command]
async fn check_existing_installation(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    if let Some(path) = db.get_installation_path()? {
        let path_buf = PathBuf::from(&path);
        if bmm_lib::balamod::Balatro::from_custom_path(path_buf).is_some() {
            Ok(Some(path))
        } else {
            map_error(db.remove_installation_path())?;
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

#[allow(non_snake_case)]
#[tauri::command]
async fn install_mod(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
    url: String,
    folderName: String,
) -> Result<PathBuf, String> {
    let folderName = {
        if folderName.is_empty() {
            None
        } else {
            Some(folderName)
        }
    };
    #[cfg(not(target_os = "linux"))]
    return map_error(bmm_lib::installer::install_mod(None, url, folderName).await);
    #[cfg(target_os = "linux")]
    {
        let installation_path = state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?;
        return map_error(
            bmm_lib::installer::install_mod(installation_path.as_ref(), url, folderName).await,
        );
    };
}

#[tauri::command]
async fn get_installed_mods_from_db(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<InstalledMod>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    map_error(db.get_installed_mods())
}

#[tauri::command]
async fn add_installed_mod(
    state: tauri::State<'_, AppState>,
    name: String,
    path: String,
    dependencies: Vec<String>,
    current_version: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let current_version = {
        if current_version.is_empty() {
            None
        } else {
            Some(current_version)
        }
    };
    map_error(db.add_installed_mod(&name, &path, &dependencies, current_version))
}

#[tauri::command]
async fn force_remove_mod(
    state: tauri::State<'_, AppState>,
    name: String,
    path: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "linux"))]
    map_error(bmm_lib::installer::uninstall_mod(None, PathBuf::from(path)))?;
    #[cfg(target_os = "linux")]
    {
        let installation_path = db.get_installation_path()?;
        map_error(bmm_lib::installer::uninstall_mod(
            installation_path.as_ref(),
            PathBuf::from(path),
        ))?;
    };
    map_error(db.remove_installed_mod(&name))
}

#[tauri::command]
async fn reindex_mods(state: tauri::State<'_, AppState>) -> Result<(usize, usize), String> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::LockPoisoned(format!("Database lock poisoned: {}", e)))?;
    let installed_mods = db.get_installed_mods().map_err(|e| e.to_string())?;
    let mut to_remove = Vec::new();
    for (index, installed_mod) in installed_mods.iter().enumerate() {
        if !PathBuf::from(&installed_mod.path).exists() {
            to_remove.push(index);
        }
    }
    let cleaned_entries = to_remove.len();
    for &index in to_remove.iter().rev() {
        db.remove_installed_mod(&installed_mods[index].name)
            .map_err(|e| e.to_string())?;
    }
    Ok((0, cleaned_entries))
}

#[tauri::command]
async fn delete_manual_mod(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(format!(
            "Invalid path '{}': Path doesn't exist",
            path.display()
        ));
    }
    #[cfg(not(target_os = "linux"))]
    let mods_dir = get_lovely_mods_dir(None);
    #[cfg(target_os = "linux")]
    let mods_dir = get_lovely_mods_dir(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
    );
    let canonicalized_path = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return Err(format!(
                "Failed to canonicalize path {}: {}",
                path.display(),
                e
            ))
        }
    };
    let canonicalized_mods_dir = match mods_dir.canonicalize() {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to canonicalize mods directory: {}", e)),
    };
    if !canonicalized_path.starts_with(&canonicalized_mods_dir) {
        return Err(format!(
            "Path is outside of the mods directory: {}",
            path.display()
        ));
    }
    log::info!("Deleting manual mod at path: {}", path.display());
    if path.is_dir() {
        match std::fs::remove_dir_all(&path) {
            Ok(_) => log::info!("Successfully removed directory: {}", path.display()),
            Err(e) => return Err(format!("Failed to remove directory: {}", e)),
        }
    } else {
        match std::fs::remove_file(&path) {
            Ok(_) => log::info!("Successfully removed file: {}", path.display()),
            Err(e) => return Err(format!("Failed to remove file: {}", e)),
        }
    }
    Ok(())
}

#[tauri::command]
async fn get_detected_local_mods(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<local_mod_detection::DetectedMod>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let cached_mods = match cache::load_cache() {
        Ok(Some((mods, _))) => mods,
        _ => Vec::new(),
    };
    local_mod_detection::detect_manual_mods(&db, &cached_mods)
}

#[tauri::command]
async fn get_dependents(mod_name: String) -> Result<Vec<String>, String> {
    let db = Database::new().map_err(|e| e.to_string())?;
    let all_dependents = db.get_dependents(&mod_name).map_err(|e| e.to_string())?;
    let filtered_dependents: Vec<String> = all_dependents
        .into_iter()
        .filter(|dep| dep != &mod_name)
        .collect();
    Ok(filtered_dependents)
}

#[tauri::command]
async fn cascade_uninstall(
    state: tauri::State<'_, AppState>,
    root_mod: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut to_uninstall = vec![root_mod.clone()];
    let mut processed = HashSet::new();
    while let Some(current) = to_uninstall.pop() {
        if processed.contains(¤t) {
            continue;
        }
        processed.insert(current.clone());
        let mod_details = map_error(db.get_mod_details(¤t))?;
        let dependents = map_error(db.get_dependents(¤t))?;
        to_uninstall.extend(dependents);
        #[cfg(not(target_os = "linux"))]
        map_error(bmm_lib::installer::uninstall_mod(
            None,
            PathBuf::from(mod_details.path),
        ))?;
        #[cfg(target_os = "linux")]
        {
            let installation_path = db.get_installation_path()?;
            map_error(bmm_lib::installer::uninstall_mod(
                installation_path.as_ref(),
                PathBuf::from(mod_details.path),
            ))?;
        };
        map_error(db.remove_installed_mod(¤t))?;
    }
    Ok(())
}

#[tauri::command]
async fn remove_installed_mod(
    state: tauri::State<'_, AppState>,
    name: String,
    path: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let is_framework = name.to_lowercase() == "steamodded" || name.to_lowercase() == "talisman";
    if is_framework {
        let all_dependents = map_error(db.get_dependents(&name))?;
        let real_dependents: Vec<String> = all_dependents
            .into_iter()
            .filter(|dep| dep != &name)
            .collect();
        if !real_dependents.is_empty() {
            return Err(format!(
                "Use cascade_uninstall to remove {} with {} dependents",
                name,
                real_dependents.len()
            ));
        }
    }
    #[cfg(not(target_os = "linux"))]
    map_error(bmm_lib::installer::uninstall_mod(None, PathBuf::from(path)))?;
    #[cfg(target_os = "linux")]
    {
        let installation_path = db.get_installation_path()?;
        map_error(bmm_lib::installer::uninstall_mod(
            installation_path.as_ref(),
            PathBuf::from(path),
        ))?;
    };
    map_error(db.remove_installed_mod(&name))
}

#[tauri::command]
async fn get_balatro_path(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    map_error(db.get_installation_path())
}

#[tauri::command]
async fn set_balatro_path(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let db = match state.db.lock() {
        Ok(db) => db,
        Err(e) => return Err(e.to_string()),
    };
    map_error(db.set_installation_path(&path))
}

#[tauri::command]
async fn find_steam_balatro(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let balatros = find_balatros();
    if let Some(path) = balatros.first() {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        map_error(db.set_installation_path(&path.path.to_string_lossy()))?;
    }
    Ok(balatros
        .iter()
        .map(|b| b.path.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command]
async fn get_steamodded_versions(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    #[cfg(not(target_os = "linux"))]
    let installer = ModInstaller::new(None, ModType::Steamodded);
    #[cfg(target_os = "linux")]
    let installer = ModInstaller::new(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
        ModType::Steamodded,
    );
    installer
        .get_available_versions()
        .await
        .map(|versions| versions.into_iter().map(|v| v.to_string()).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_steamodded_version(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
    version: String,
) -> Result<String, String> {
    #[cfg(not(target_os = "linux"))]
    let installer = ModInstaller::new(None, ModType::Steamodded);
    #[cfg(target_os = "linux")]
    let installer = ModInstaller::new(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
        ModType::Steamodded,
    );
    installer
        .install_version(&version)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_talisman_versions(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    #[cfg(not(target_os = "linux"))]
    let installer = ModInstaller::new(None, ModType::Talisman);
    #[cfg(target_os = "linux")]
    let installer = ModInstaller::new(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
        ModType::Talisman,
    );
    installer
        .get_available_versions()
        .await
        .map(|versions| versions.into_iter().map(|v| v.to_string()).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_latest_steamodded_release(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    if let Ok(Some(versions)) = cache::load_versions_cache("steamodded") {
        if !versions.is_empty() {
            let version = &versions[0];
            return Ok(format!(
                "https://github.com/Steamodded/smods/archive/refs/tags/{}.zip",
                version
            ));
        }
    }
    #[cfg(not(target_os = "linux"))]
    let installer = ModInstaller::new(None, ModType::Steamodded);
    #[cfg(target_os = "linux")]
    let installer = ModInstaller::new(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
        ModType::Steamodded,
    );
    installer
        .get_latest_release()
        .await
        .map(|version| {
            match installer.mod_type {
                ModType::Steamodded => {
                    format!(
                        "https://github.com/Steamodded/smods/archive/refs/tags/{}.zip",
                        version
                    )
                }
                _ => format!(
                    "https://github.com/Steamodded/smods/archive/refs/tags/{}.zip",
                    version
                ),
            }
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_talisman_version(
    #[cfg(target_os = "linux")] state: tauri::State<'_, AppState>,
    version: String,
) -> Result<String, String> {
    #[cfg(not(target_os = "linux"))]
    let installer = ModInstaller::new(None, ModType::Talisman);
    #[cfg(target_os = "linux")]
    let installer = ModInstaller::new(
        state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?
            .get_installation_path()?
            .as_ref(),
        ModType::Talisman,
    );
    installer
        .install_version(&version)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn backup_local_mod(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(format!("Path doesn't exist: {}", path.display()));
    }
    let backup_dir = get_backup_dir()?;
    let backup_id = format!(
        "backup_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("Failed to get timestamp: {}", e))?
            .as_millis()
    );
    let backup_path = backup_dir.join(backup_id);
    std::fs::create_dir_all(&backup_path)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;
    if path.is_dir() {
        copy_dir_all(&path, &backup_path.join(path.file_name().unwrap()))
            .map_err(|e| format!("Failed to copy mod to backup: {}", e))?;
    } else {
        std::fs::copy(&path, backup_path.join(path.file_name().unwrap()))
            .map_err(|e| format!("Failed to copy mod file to backup: {}", e))?;
    }
    let metadata = json!({
        "original_path": path.to_string_lossy().to_string(),
        "backup_time": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    std::fs::write(
        backup_path.join("metadata.json"),
        serde_json::to_string_pretty(&metadata)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?,
    )
    .map_err(|e| format!("Failed to write metadata: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn restore_from_backup(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let backup_dir = get_backup_dir()?;
    let mut latest_backup = None;
    let mut latest_time = 0;
    for entry in std::fs::read_dir(&backup_dir)
        .map_err(|e| format!("Failed to read backup directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read backup entry: {}", e))?;
        let metadata_path = entry.path().join("metadata.json");
        if metadata_path.exists() {
            let metadata: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&metadata_path)
                    .map_err(|e| format!("Failed to read metadata file: {}", e))?,
            )
            .map_err(|e| format!("Failed to parse metadata: {}", e))?;
            if let Some(original_path) = metadata.get("original_path").and_then(|v| v.as_str()) {
                if original_path == path.to_string_lossy() {
                    if let Some(backup_time) = metadata.get("backup_time").and_then(|v| v.as_u64())
                    {
                        if backup_time > latest_time {
                            latest_time = backup_time;
                            latest_backup = Some(entry.path());
                        }
                    }
                }
            }
        }
    }
    let backup_path = latest_backup.ok_or_else(|| "No backup found for this path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {}", e))?;
    }
    for entry in std::fs::read_dir(&backup_path)
        .map_err(|e| format!("Failed to read backup directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read backup entry: {}", e))?;
        let file_name = entry.file_name();
        if file_name == "metadata.json" {
            continue;
        }
        let dest_path = path.parent().unwrap().join(&file_name);
        if entry.path().is_dir() {
            copy_dir_all(&entry.path(), &dest_path)
                .map_err(|e| format!("Failed to restore directory from backup: {}", e))?;
        } else {
            std::fs::copy(entry.path(), &dest_path)
                .map_err(|e| format!("Failed to restore file from backup: {}", e))?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn remove_backup(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let backup_dir = get_backup_dir()?;
    for entry in std::fs::read_dir(&backup_dir)
        .map_err(|e| format!("Failed to read backup directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read backup entry: {}", e))?;
        let metadata_path = entry.path().join("metadata.json");
        if metadata_path.exists() {
            let metadata: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&metadata_path)
                    .map_err(|e| format!("Failed to read metadata file: {}", e))?,
            )
            .map_err(|e| format!("Failed to parse metadata: {}", e))?;
            if let Some(original_path) = metadata.get("original_path").and_then(|v| v.as_str()) {
                if original_path == path.to_string_lossy() {
                    std::fs::remove_dir_all(entry.path())
                        .map_err(|e| format!("Failed to remove backup: {}", e))?;
                }
            }
        }
    }
    Ok(())
}

fn get_backup_dir() -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir().join("balatro_mod_manager_backups");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;
    Ok(temp_dir)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let path = entry.path();
        if ty.is_dir() {
            copy_dir_all(&path, &dst.join(path.file_name().unwrap()))?;
        } else {
            std::fs::copy(&path, dst.join(path.file_name().unwrap()))?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn get_background_state(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    map_error(db.get_background_enabled())
}

#[tauri::command]
async fn set_background_state(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    map_error(db.set_background_enabled(enabled))
}

#[tauri::command]
async fn verify_path_exists(path: String) -> bool {
    PathBuf::from(path).exists()
}

#[tauri::command]
async fn path_exists(path: String) -> Result<bool, String> {
    let path = PathBuf::from(path);
    Ok(path.exists())
}

#[tauri::command]
async fn check_custom_balatro(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<bool, String> {
    let path = PathBuf::from(&path);
    let path_to_check = if path.is_file() {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or(path.clone())
    } else {
        path.clone()
    };
    let is_valid = bmm_lib::balamod::Balatro::from_custom_path(path_to_check.clone()).is_some();
    if is_valid {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        map_error(db.set_installation_path(&path_to_check.to_string_lossy()))?;
    }
    Ok(is_valid)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            app.emit("single-instance", Payload { args: argv, cwd })
                .unwrap();
        }))
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_prevent_default::init())
        .setup(|app| {
            let db = map_error(Database::new())?;
            let discord_rpc = DiscordRpcManager::new();
            let discord_rpc_enabled = db.is_discord_rpc_enabled().unwrap_or(true);
            discord_rpc.set_enabled(discord_rpc_enabled);
            app.manage(AppState {
                db: Mutex::new(db),
                discord_rpc: Mutex::new(discord_rpc),
            });
            let app_dir = app
                .path()
                .app_data_dir()
                .map_err(|_| AppError::DirNotFound(PathBuf::from("app data directory")))?;
            std::fs::create_dir_all(&app_dir).map_err(|e| AppError::DirCreate {
                path: app_dir.clone(),
                source: e.to_string(),
            })?;
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            find_steam_balatro,
            check_custom_balatro,
            check_existing_installation,
            get_balatro_path,
            set_balatro_path,
            launch_balatro,
            check_steam_running,
            check_balatro_running,
            get_installed_mods_from_db,
            install_mod,
            add_installed_mod,
            remove_installed_mod,
            get_steamodded_versions,
            install_steamodded_version,
            install_talisman_version,
            get_talisman_versions,
            verify_path_exists,
            path_exists,
            check_mod_installation,
            refresh_mods_folder,
            save_mods_cache,
            load_mods_cache,
            save_versions_cache,
            load_versions_cache,
            set_lovely_console_status,
            get_lovely_console_status,
            check_untracked_mods,
            clear_cache,
            cascade_uninstall,
            force_remove_mod,
            get_dependents,
            reindex_mods,
            get_background_state,
            set_background_state,
            get_last_fetched,
            update_last_fetched,
            get_repo_path,
            clone_repo,
            pull_repo,
            list_directories,
            read_json_file,
            read_text_file,
            get_mod_thumbnail,
            get_discord_rpc_status,
            set_discord_rpc_status,
            get_latest_steamodded_release,
            set_discord_rpc_status,
            mod_update_available,
            get_detected_local_mods,
            delete_manual_mod,
            backup_local_mod,
            restore_from_backup,
            remove_backup,
            open_directory,
            get_mods_folder,
            process_dropped_file,
            process_mod_archive,
            is_mod_enabled,
            toggle_mod_enabled,
            is_mod_enabled_by_path,
            toggle_mod_enabled_by_path,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        log::error!("Failed to run application: {}", e);
        log::logger().flush();
        std::process::exit(1);
    }
}
