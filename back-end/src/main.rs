#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod config;
mod optimizer;
mod profile;
mod utils;

use config::{AdvancedOption, LocalMetaData, Optimization, OptimizerConfig};
use profile::UserProfile;
use std::sync::Mutex;
use sysinfo::System;
use tauri::api::dialog::blocking::FileDialogBuilder;
use tauri::api::dialog::MessageDialogBuilder;
use tauri::{AppHandle, Manager};
use tauri_plugin_log::LogTarget;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use crate::config::UserStatus;

struct AppState(Mutex<Option<OptimizerConfig>>);

static BUNDLED_WEBVIEW2_INSTALLER_DATA: &[u8; 1636808] =
    include_bytes!("../bundled_data/MicrosoftEdgeWebview2Setup.exe");

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .targets([LogTarget::Stdout, LogTarget::LogDir, LogTarget::Webview])
                .build(),
        )
        .manage(AppState(Mutex::new(None)))
        .setup(|app| {
            let app_data_dir = app
                .path_resolver()
                .app_data_dir()
                .expect("Unable to get app data directory");
            std::fs::create_dir_all(app_data_dir.as_path())
                .expect("Unable to create app data directory");
            let state: tauri::State<AppState> = app.state();
            let loaded_config = OptimizerConfig::load(app_data_dir.as_path());
            log::info!("Loaded Config: {:#?}", loaded_config);
            *state.0.lock().expect("Unable to aquire state lock") = Some(loaded_config);
            check_webview_status(app.app_handle());
            check_yuzu_not_running();
            Ok(())
        })
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { .. } => {
                let app_data_dir = event
                    .window()
                    .app_handle()
                    .path_resolver()
                    .app_data_dir()
                    .expect("Unable to get app data directory");
                let state: tauri::State<AppState> = event.window().state();
                let state = state.0.lock().expect("Unable to acquire state lock");
                let config = state.as_ref().expect("Config should be loaded by now");
                log::info!("Saving local data: {:#?}", config.local_data);
                config.local_data.save(app_data_dir.as_path());
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            select_yuzu_data_folder,
            update_selected_user,
            apply_optimization,
            get_user_status,
            query_metadata,
            query_config,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running tauri application");
}

#[tauri::command]
fn apply_optimization(
    state: tauri::State<AppState>,
    user_profile: UserProfile,
    optimization: Optimization,
    advanced_options: Vec<AdvancedOption>,
) -> Result<(), String> {
    let mut state = state.0.lock().expect("Unable to acquire state lock");
    let config = state.as_mut().expect("Config should be loaded by now");
    log::info!(
        "Applying Optimization for user {}: {}",
        user_profile.name,
        optimization.to_string()
    );

    let optimization_result = match optimization {
        Optimization::Settings => optimizer::optimize_settings(config, &user_profile),
        Optimization::Mods => optimizer::optimize_mods(config, &user_profile, advanced_options),
        Optimization::Save => optimizer::optimize_save(config, &user_profile),
    };
    if optimization_result.is_err() {
        log::error!("Error applying optimization");
        return Err(optimization_result.unwrap_err().to_string());
    }

    let metadata = &mut config.local_data;
    match (optimization, metadata.user_statuses.get_mut(&user_profile)) {
        (Optimization::Settings, Some(u)) => {
            u.settings_optimized = true;
        }
        (Optimization::Mods, Some(u)) => {
            u.mods_optimized = true;
        }
        (Optimization::Save, Some(u)) => {
            u.save_optimized = true;
        }
        (o, None) => {
            metadata.user_statuses.insert(
                user_profile,
                UserStatus {
                    settings_optimized: o == Optimization::Settings,
                    mods_optimized: o == Optimization::Mods,
                    save_optimized: o == Optimization::Save,
                },
            );
        }
    }
    Ok(())
}

// should be called by the front-end only once, and then cached to avoid cloning too much
#[tauri::command]
fn query_config(state: tauri::State<AppState>) -> OptimizerConfig {
    state
        .0
        .lock()
        .expect("Unable to acquire state lock")
        .as_ref()
        .expect("Config should be loaded by now")
        .clone()
}

// should be called by the front-end only once, and then cached to avoid cloning too much
#[tauri::command]
fn query_metadata(state: tauri::State<AppState>) -> LocalMetaData {
    state
        .0
        .lock()
        .expect("Unable to acquire state lock")
        .as_ref()
        .expect("Config should be loaded by now")
        .local_data
        .clone()
}

#[tauri::command]
async fn select_yuzu_data_folder(app_handle: tauri::AppHandle) -> Result<OptimizerConfig, String> {
    let app_data_dir = app_handle
        .path_resolver()
        .app_data_dir()
        .expect("Unable to get app data directory");
    let state: tauri::State<AppState> = app_handle.state();
    let mut state = state.0.lock().expect("Unable to acquire state lock");
    let config = state.as_mut().expect("Config should be loaded by now");
    let yuzu_folder = config.local_data.yuzu_folder.as_ref();
    let default_directory = tauri::api::path::data_dir().expect("Unable to find data directory");
    let dialog_directory = yuzu_folder.unwrap_or(&default_directory);
    let dialog_result = FileDialogBuilder::new()
        .set_title("Select yuzu folder")
        .set_directory(dialog_directory)
        .pick_folder();
    if let Some(f) = dialog_result {
        let new_config = OptimizerConfig::from_data_folder(app_data_dir.as_path(), f);
        if new_config.local_data.yuzu_folder.is_none() {
            return Err(String::from("Incorrect yuzu folder specified"));
        }
        *state = Some(new_config.clone());
        new_config.local_data.save(app_data_dir.as_path());
        return Ok(new_config);
    }
    return Err(String::from("No yuzu folder specified"));
}

#[tauri::command]
fn update_selected_user(state: tauri::State<AppState>, user_profile: Option<UserProfile>) {
    state
        .0
        .lock()
        .expect("Unable to acquire state lock")
        .as_mut()
        .expect("Config should be loaded by now")
        .local_data
        .selected_user_profile = user_profile;
}

#[tauri::command]
fn get_user_status(state: tauri::State<AppState>, user_profile: UserProfile) -> UserStatus {
    if let Some(status) = state
        .0
        .lock()
        .expect("Unable to acquire state lock")
        .as_ref()
        .expect("Config should be loaded by now")
        .local_data
        .user_statuses
        .get(&user_profile)
    {
        return status.clone();
    }
    UserStatus::default()
}

fn check_webview_status(app_handle: AppHandle) {
    if RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}").is_err() // System-wide 64bit
      && RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}").is_err() // System-wide 32bit
      && RegKey::predef(HKEY_CURRENT_USER).open_subkey("SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}").is_err() // User-wide 64bit&32bit
      {
        log::info!("Webview 2 not found on system! Prompting install message...");
        MessageDialogBuilder::new("Install Microsoft Webview2 Runtime", "This app requires Microsoft Webview2 Runtime. Install?")
            .kind(tauri::api::dialog::MessageDialogKind::Warning)
            .buttons(tauri::api::dialog::MessageDialogButtons::YesNo)
            .show(move |install| {
                if install {
                    log::info!("Starting Webview2 install process...");
                    std::fs::write("MicrosoftEdgeWebview2Setup.exe", BUNDLED_WEBVIEW2_INSTALLER_DATA)
                        .expect("Unable to write Webview2 installer to disk");
                    std::process::Command::new("MicrosoftEdgeWebview2Setup.exe")
                        .arg("/install")
                        .spawn()
                        .expect("Unable to start Webview2 installer process")
                        .wait()
                        .expect("Error running Webview2 installer"); 
                    if std::fs::remove_file("MicrosoftEdgeWebview2Setup.exe").is_err() {
                        log::warn!("Unable to clean up webview2 install artifacts");
                    }
                    log::info!("Restarting app...");
                    app_handle.restart();
                } else {
                    log::info!("Webview2 not found. Exiting app...");
                    app_handle.exit(0);
                }
            });
      }
}

fn check_yuzu_not_running() {
    let system = System::new_all();
    if system
        .processes_by_exact_name("yuzu.exe")
        .peekable()
        .peek()
        .is_some()
    {
        log::info!(
            "Detected yuzu at least one yuzu instance running. Prompting warning message..."
        );
        MessageDialogBuilder::new(
            "Close yuzu Instances", 
            "At least one yuzu instance is detected running on your system. The optimizer works best if yuzu is closed. Close all yuzu instances?")
            .kind(tauri::api::dialog::MessageDialogKind::Warning)
            .buttons(tauri::api::dialog::MessageDialogButtons::YesNo)
            .show(move |terminate_yuzu| {
                if terminate_yuzu {
                    for process in system.processes_by_name("yuzu.exe") {
                        log::info!("Killing yuzu instance: {} ({})", process.name(), process.pid());
                        process.kill();
                    }
                }
            });
    }
}
