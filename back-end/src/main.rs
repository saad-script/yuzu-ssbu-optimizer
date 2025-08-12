#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod config;
mod optimizer;
mod profile;
mod utils;

use config::{AdvancedOption, LocalPersistantData, Optimization, OptimizerConfig};
use profile::UserProfile;
use std::sync::{Arc, RwLock};
use sysinfo::System;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_log::{Target, TargetKind, WEBVIEW_TARGET};

use crate::config::UserStatus;

#[cfg(target_os = "windows")]
pub static BUNDLED_WEBVIEW2_INSTALLER_DATA: &[u8] =
    include_bytes!("../bundled_data/MicrosoftEdgeWebview2Setup.exe");

struct AppState {
    app_handle: AppHandle,
    config: Arc<RwLock<OptimizerConfig>>,
}

impl AppState {
    fn check_web_engine_status(&self) {
        #[cfg(target_os = "windows")]
        if tauri::webview_version().is_err() {
            log::info!("Webview 2 not found on system! Prompting install message...");
            let app_handle = self.app_handle.clone();
            self.app_handle
                .dialog()
                .message("This app requires Microsoft Webview2 Runtime. Install?")
                .title("Install Microsoft Webview2 Runtime")
                .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
                .buttons(tauri_plugin_dialog::MessageDialogButtons::YesNo)
                .show(move |install| {
                    if install {
                        log::info!("Starting Webview2 install process...");
                        std::fs::write(
                            "MicrosoftEdgeWebview2Setup.exe",
                            BUNDLED_WEBVIEW2_INSTALLER_DATA,
                        )
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

        #[cfg(not(target_os = "windows"))]
        if tauri::webview_version().is_err() {
            log::info!("Webkit not found on system! Prompting warning message...");
            let app_handle = self.app_handle.clone();
            self.app_handle
                .dialog()
                .message("This app requires webkit to be installed!")
                .title("Webkit Not Installed")
                .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCustom("Exit"))
                .show(move || app_handle.exit(0));
        }
    }

    fn check_emu_not_running(&self) {
        let emu_name = self.get_config().get_emulator_name();
        let process_name = if cfg!(windows) {
            format!("{}.exe", emu_name)
        } else {
            emu_name.clone()
        };

        let mut system = System::new_all();
        if system
            .processes_by_exact_name(&process_name)
            .peekable()
            .peek()
            .is_some()
        {
            log::info!(
                "Detected at least one {} instance running. Prompting warning message...",
                emu_name
            );
            let app_handle = self.app_handle.clone();
            self.app_handle.dialog()
                .message(format!("At least one {} instance is detected running on your system. The optimizer works best if {} is closed. Close all {} instances?", emu_name, emu_name, emu_name))
                .title(format!("Close {} Instances", emu_name))
                .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
                .buttons(tauri_plugin_dialog::MessageDialogButtons::YesNo)
                .show(move |terminate_emu| {
                    if terminate_emu {
                        system.refresh_all();
                        for process in system.processes_by_exact_name(&process_name) {
                            log::info!("Killing {} instance: {} ({})", emu_name, process.name(), process.pid());
                            process.kill();
                        }
                        app_handle.restart();
                    }
                });
        }
    }

    fn refresh_title(&self) {
        let config = self.get_config();
        let emu_name = config.get_emulator_name();
        self.app_handle
            .get_webview_window("main")
            .and_then(|w| {
                match w.set_title(format!("{} SSBU Optimizer", emu_name).as_str()) {
                    Ok(_) => Some(w),
                    Err(_) => None
                }
            });
    }

    pub fn get_config(&self) -> OptimizerConfig {
        let state: tauri::State<AppState> = self.app_handle.state();
        let config = state
            .config
            .read()
            .expect("Unable to acquire read lock on config")
            .clone();
        config
    }

    pub fn set_config(&self, config: OptimizerConfig) {
        let state: tauri::State<AppState> = self.app_handle.state();
        let mut config_guard = state
            .config
            .write()
            .expect("Unable to acquire write lock on config");
        *config_guard = config;
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .clear_targets()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::Webview),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("webview".into()),
                    })
                    .filter(|metadata| metadata.target().starts_with(WEBVIEW_TARGET)),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("rust".into()),
                    })
                    .filter(|metadata| !metadata.target().starts_with(WEBVIEW_TARGET)),
                ])
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Unable to get app data directory");
            std::fs::create_dir_all(app_data_dir.as_path())
                .expect("Unable to create app data directory");
            let loaded_config = OptimizerConfig::load(app.app_handle().path(), None);
            app.manage(AppState {
                app_handle: app.app_handle().clone(),
                config: Arc::new(RwLock::new(loaded_config)),
            });
            let state: tauri::State<AppState> = app.state();
            state.refresh_title();
            state.check_web_engine_status();
            state.check_emu_not_running();
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let app_data_dir = window
                    .app_handle()
                    .path()
                    .app_data_dir()
                    .expect("Unable to get app data directory");
                let state: tauri::State<AppState> = window.state();
                let config = state.get_config();
                log::info!("Saving local data: {:#?}", config.local_data);
                config.local_data.save(app_data_dir.as_path());
            }
        })
        .invoke_handler(tauri::generate_handler![
            select_emu_data_folder,
            update_selected_user,
            apply_optimization,
            get_user_status,
            query_local_persistant_data,
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
    let mut config = state.get_config();
    log::info!(
        "Applying Optimization for user {}: {}",
        user_profile.name,
        optimization
    );
    let optimization_result = match optimization {
        Optimization::Settings => optimizer::optimize_settings(&config, &user_profile),
        Optimization::Mods => optimizer::optimize_mods(&config, &user_profile, advanced_options),
        Optimization::Save => optimizer::optimize_save(&config, &user_profile),
    };
    if optimization_result.is_err() {
        log::error!("Error applying optimization");
        return Err(optimization_result.unwrap_err().to_string());
    }

    let local_data = &mut config.local_data;
    match (
        optimization,
        local_data.user_statuses.get_mut(&user_profile),
    ) {
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
            local_data.user_statuses.insert(
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
    state.get_config()
}

// should be called by the front-end only once, and then cached to avoid cloning too much
#[tauri::command]
fn query_local_persistant_data(state: tauri::State<AppState>) -> LocalPersistantData {
    state.get_config().local_data.clone()
}

#[tauri::command]
async fn select_emu_data_folder(app_handle: tauri::AppHandle) -> Result<OptimizerConfig, String> {
    let state: tauri::State<AppState> = app_handle.state();
    let config = state.get_config();
    let emu_folder = config.local_data.emu_folder.as_ref();
    let default_directory = app_handle
        .path()
        .app_data_dir()
        .expect("Unable to find data directory");
    let dialog_directory = emu_folder.unwrap_or(&default_directory);
    let dialog_result = app_handle
        .dialog()
        .file()
        .set_title("Select emulator data folder")
        .set_directory(dialog_directory)
        .blocking_pick_folder();
    if let Some(f) = dialog_result {
        let folder = f
            .into_path()
            .expect("Unable to read selection as folder path");
        let new_config = OptimizerConfig::load(app_handle.path(), Some(folder));
        if new_config.local_data.emu_folder.is_none() {
            return Err(String::from("Incorrect emulator data folder specified"));
        }
        state.set_config(new_config);
        state.refresh_title();
        state.check_emu_not_running();
        let new_config = state.get_config();
        return Ok(new_config);
    }
    Err(String::from("No emulator data folder specified"))
}

#[tauri::command]
fn update_selected_user(state: tauri::State<AppState>, user_profile: Option<UserProfile>) {
    state.get_config().local_data.selected_user_profile = user_profile;
}

#[tauri::command]
fn get_user_status(state: tauri::State<AppState>, user_profile: UserProfile) -> UserStatus {
    if let Some(status) = state
        .get_config()
        .local_data
        .user_statuses
        .get(&user_profile)
    {
        return status.clone();
    }
    UserStatus::default()
}
