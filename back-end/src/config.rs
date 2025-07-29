use crate::profile::{self, UserProfile};
use crate::utils::{io_error};
use ini::Ini;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_EMU: &str = "yuzu";
pub const SSBU_TITLE_ID: &str = "01006A800016E000";


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Optimization {
    Settings,
    Mods,
    Save,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdvancedOption {
    CleanSkyline,
    CleanArc,
}

impl ToString for Optimization {
    fn to_string(&self) -> String {
        match self {
            Optimization::Settings => String::from("Settings"),
            Optimization::Mods { .. } => String::from("Mods"),
            Optimization::Save => String::from("Save"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    pub local_data: LocalMetaData,
    pub user_profiles: Vec<UserProfile>,
    #[serde(skip)]
    pub emu_config: Option<Ini>
}

impl OptimizerConfig {
    pub fn load(app_handle: AppHandle, emu_folder: Option<PathBuf>) -> Self {
        let data_dir = app_handle
            .path()
            .data_dir()
            .expect("Unable to get data directory");
        let app_data_dir = app_handle
                .path()
                .app_data_dir()
                .expect("Unable to get app data directory");
        let default_emu_folder = data_dir.join(DEFAULT_EMU);
        let local_data = LocalMetaData::load(app_data_dir.as_path());
        let emu_folder = emu_folder
            .unwrap_or(local_data.emu_folder.clone().unwrap_or(default_emu_folder));
        log::info!("EMU FOLDER: {:?}", emu_folder);
        let main_config_settings_path = emu_folder
            .join("config")
            .join("qt-config.ini");
        let emu_config = Ini::load_from_file_noescape(main_config_settings_path.as_path()).ok();
        let mut optimizer_config = OptimizerConfig {
            local_data: local_data,
            user_profiles: vec![],
            emu_config
        };
        if optimizer_config.emu_config.is_none() {
            return optimizer_config;
        };

        let user_profiles = optimizer_config
            .get_nand_folder()
            .and_then(|nf| profile::parse_user_profiles_save_file(nf.as_path()));

        match user_profiles {
            Ok(u) => {
                log::info!("TEST");
                optimizer_config.local_data.emu_folder = Some(emu_folder.to_path_buf());
                if let Some(su) = optimizer_config.local_data.selected_user_profile.as_ref() {
                    if !u.contains(su) {
                        if let Some(default_user) = u.get(0) {
                            optimizer_config.local_data.selected_user_profile = Some(default_user.clone());
                        } else {
                            optimizer_config.local_data.selected_user_profile = None;
                        }
                    }
                }
                optimizer_config.user_profiles = u;
            }
            _ => {
                optimizer_config.local_data.emu_folder = None;
                optimizer_config.local_data.selected_user_profile = None;
            }
        }
        return optimizer_config;
    }

    pub fn get_emulator_name(&self) -> String {
        let file_name = self.local_data.emu_folder
            .as_ref()
            .and_then(|f| f.file_name())
            .map(|f| f.to_str())
            .and_then(|f| f)
            .unwrap_or(DEFAULT_EMU);
        return file_name.to_string();
    }

    pub fn get_emu_config(&self, section: &str, key: &str, default: Option<&str>) -> Option<String> {
        let section = self.emu_config.as_ref()?.section(Some(section))?;
        let use_default= section.get(format!("{}\\default", key))
            .map(|b| b.parse().unwrap_or(false)).unwrap_or(false);
        let value = match use_default {
            true => default,
            false => section.get(key)
        };
        return value.map(|v| v.to_string());
    }

    pub fn get_nand_folder(&self) -> io::Result<PathBuf> {
        let default_nand_dir = self.local_data.emu_folder.as_ref()
            .and_then(|f| Some(f.join("nand")));
        let default_nand_dir = default_nand_dir.as_ref()
            .map(|f| f.as_path().to_str())
            .and_then(|inner| inner);
        let nand_dir = self.get_emu_config("Data%20Storage", "nand_directory", default_nand_dir)
            .ok_or(io_error!(NotFound, "Emulator nand folder not found"))?;
        Ok(PathBuf::from(nand_dir))
    }
    
    pub fn get_sdmc_folder(&self) -> io::Result<PathBuf> {
        let default_sdmc_dir = self.local_data.emu_folder.as_ref()
            .and_then(|f| Some(f.join("sdmc")));
        let default_sdmc_dir = default_sdmc_dir.as_ref()
            .map(|f| f.as_path().to_str())
            .and_then(|inner| inner);
        let sdmc_dir = self.get_emu_config("Data%20Storage", "sdmc_directory", default_sdmc_dir)
            .ok_or(io_error!(NotFound, "Emulator sdmc folder not found"))?;
        Ok(PathBuf::from(sdmc_dir))
    }

    pub fn get_save_folder(&self, user_profile: &UserProfile) -> io::Result<PathBuf> {
        Ok(self.get_nand_folder()?
            .join("user")
            .join("save")
            .join("0000000000000000")
            .join(user_profile.get_uuid_emu_storage_string())
            .join(SSBU_TITLE_ID))
    }

    pub fn get_arc_config_folder(&self, user_profile: &UserProfile) -> io::Result<PathBuf> {
        let uuids = user_profile.get_uuid_arc_storage_strings();
        Ok(self.get_sdmc_folder()?
            .join("ultimate")
            .join("arcropolis")
            .join("config")
            .join(uuids.0)
            .join(uuids.1))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserStatus {
    pub settings_optimized: bool,
    pub mods_optimized: bool,
    pub save_optimized: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LocalMetaData {
    pub emu_folder: Option<PathBuf>,
    pub selected_user_profile: Option<UserProfile>,
    #[serde(with = "vectorize")]
    pub user_statuses: HashMap<UserProfile, UserStatus>,
}

impl LocalMetaData {
    pub fn save(&self, app_data_dir: &Path) {
        let local_writer = File::create(app_data_dir.join("optimizer_data.json"))
            .expect("Unable to create local meta data file");
        serde_json::to_writer(local_writer, self).expect("Error writing local meta data to file");
    }

    pub fn load(app_data_dir: &Path) -> LocalMetaData {
        let local_writer = File::open(app_data_dir.join("optimizer_data.json"));
        let local_data = match local_writer {
            Ok(f) => serde_json::from_reader(f).unwrap_or(LocalMetaData::default()),
            _ => LocalMetaData::default(),
        };
        local_data
    }
}
