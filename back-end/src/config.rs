use crate::profile::{self, UserProfile};
use crate::utils::io_error;
use ini::Ini;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use tauri::path::PathResolver;

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

impl std::fmt::Display for Optimization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Optimization::Settings => write!(f, "Settings"),
            Optimization::Mods => write!(f, "Mods"),
            Optimization::Save => write!(f, "Save"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    pub local_data: LocalPersistantData,
    pub user_profiles: Vec<UserProfile>,
    pub emu_filesystem: EmuFileSystem,
}

impl OptimizerConfig {
    pub fn load<R: tauri::Runtime>(
        path_resolver: &PathResolver<R>,
        emu_folder: Option<PathBuf>,
    ) -> Self {
        let data_dir = path_resolver
            .data_dir()
            .expect("Unable to get data directory");
        let app_data_dir = path_resolver
            .app_data_dir()
            .expect("Unable to get app data directory");
        let default_emu_folder = data_dir.join(DEFAULT_EMU);
        let mut local_data = LocalPersistantData::load(app_data_dir.as_path());
        let emu_folder =
            emu_folder.unwrap_or(local_data.emu_folder.clone().unwrap_or(default_emu_folder));

        let emu_filesystem = EmuFileSystem::load(emu_folder.as_path(), path_resolver);

        let user_profiles = emu_filesystem
            .nand_folder
            .as_ref()
            .and_then(|nf| profile::parse_user_profiles_save_file(nf.as_path()).ok());

        match user_profiles {
            Some(user_profiles) => {
                local_data.emu_folder = Some(emu_folder.to_path_buf());
                if let Some(selected_user) = local_data.selected_user_profile.as_ref() {
                    if !user_profiles.contains(selected_user) {
                        if let Some(default_user) = user_profiles.first() {
                            local_data.selected_user_profile = Some(default_user.clone());
                        } else {
                            local_data.selected_user_profile = None;
                        }
                    }
                }
                OptimizerConfig {
                    local_data,
                    user_profiles,
                    emu_filesystem,
                }
            }
            _ => {
                local_data.emu_folder = None;
                local_data.selected_user_profile = None;
                OptimizerConfig {
                    local_data,
                    user_profiles: vec![],
                    emu_filesystem,
                }
            }
        }
    }

    pub fn get_emulator_name(&self) -> String {
        self.emu_filesystem
            .emu_name
            .as_ref()
            .unwrap_or(&DEFAULT_EMU.to_string())
            .to_string()
    }

    pub fn get_save_folder(&self, user_profile: &UserProfile) -> io::Result<PathBuf> {
        Ok(self
            .emu_filesystem
            .nand_folder
            .as_ref()
            .ok_or(io_error!(NotFound, "Unable to find nand folder"))?
            .join("user")
            .join("save")
            .join("0000000000000000")
            .join(user_profile.get_uuid_emu_storage_string())
            .join(SSBU_TITLE_ID))
    }

    pub fn get_arc_config_folder(&self, user_profile: &UserProfile) -> io::Result<PathBuf> {
        let uuids = user_profile.get_uuid_arc_storage_strings();
        Ok(self
            .emu_filesystem
            .sdmc_folder
            .as_ref()
            .ok_or(io_error!(NotFound, "Unable to find nand folder"))?
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
pub struct LocalPersistantData {
    pub emu_folder: Option<PathBuf>,
    pub selected_user_profile: Option<UserProfile>,
    #[serde(with = "vectorize")]
    pub user_statuses: HashMap<UserProfile, UserStatus>,
}

impl LocalPersistantData {
    pub fn save(&self, app_data_dir: &Path) {
        let local_writer = File::create(app_data_dir.join("optimizer_data.json"))
            .expect("Unable to create local meta data file");
        serde_json::to_writer(local_writer, self).expect("Error writing local meta data to file");
    }

    pub fn load(app_data_dir: &Path) -> LocalPersistantData {
        let local_writer = File::open(app_data_dir.join("optimizer_data.json"));
        match local_writer {
            Ok(f) => serde_json::from_reader(f).unwrap_or(LocalPersistantData::default()),
            _ => LocalPersistantData::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EmuFileSystem {
    pub emu_name: Option<String>,
    pub config_folder: Option<PathBuf>,
    pub nand_folder: Option<PathBuf>,
    pub sdmc_folder: Option<PathBuf>,
}

impl EmuFileSystem {
    pub fn load<R: tauri::Runtime>(emu_folder: &Path, path_resolver: &PathResolver<R>) -> Self {
        let mut is_local_user_emu_data_folder = false;
        let emu_name = emu_folder
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .and_then(|f| {
                if f.to_lowercase() == "user" {
                    is_local_user_emu_data_folder = true;
                    log::info!("Local 'user' emulator data folder found. Trying to find infer emulator name...");
                    let exe_folder = emu_folder.parent()?;                    
                    return std::fs::read_dir(exe_folder).ok()?.filter_map(|entry| {
                        let entry = entry.ok()?;
                        let path = entry.path();
                        if (cfg!(windows) && path.extension()?.to_ascii_lowercase() == "exe")
                            || (cfg!(not(windows)) && path.extension().is_none()) {
                            let metadata = entry.metadata().ok()?;
                            Some((metadata.len(), path.file_stem()?.to_string_lossy().into_owned()))
                        } else {
                            None
                        }
                    })
                    .max_by_key(|&(size, _)| size)
                    .map(|(_, stem)| stem);
                }
                Some(f)
            });
        if emu_name.is_none() {
            return EmuFileSystem::default();
        }
        let emu_name = emu_name.unwrap();

        let config_dir = if cfg!(windows) || is_local_user_emu_data_folder {
            emu_folder.join("config")
        } else {
            path_resolver
                .config_dir()
                .map(|f| f.join(emu_name.as_str()))
                .expect("Unable to find config directory")
        };

        let main_config_settings_path = config_dir.join("qt-config.ini");
        let emu_config = Ini::load_from_file_noescape(main_config_settings_path.as_path());
        if emu_config.is_err() {
            return EmuFileSystem::default();
        }
        let emu_config = emu_config.unwrap();

        let default_nand_dir = emu_folder.join("nand");
        let default_nand_dir = default_nand_dir.to_str();
        let nand_dir = Self::get_emu_config_path(
            &emu_config,
            "Data%20Storage",
            "nand_directory",
            default_nand_dir,
        );

        let default_sdmc_dir = emu_folder.join("sdmc");
        let default_sdmc_dir = default_sdmc_dir.to_str();
        let sdmc_dir = Self::get_emu_config_path(
            &emu_config,
            "Data%20Storage",
            "sdmc_directory",
            default_sdmc_dir,
        );

        EmuFileSystem {
            emu_name: Some(emu_name),
            config_folder: Some(config_dir),
            nand_folder: nand_dir,
            sdmc_folder: sdmc_dir,
        }
    }

    fn get_emu_config_path(
        ini: &ini::Ini,
        section: &str,
        key: &str,
        default: Option<&str>,
    ) -> Option<PathBuf> {
        let section = ini.section(Some(section))?;
        let use_default = section
            .get(format!("{}\\default", key))
            .map(|b| b.parse().unwrap_or(false))
            .unwrap_or(false);
        let value = match use_default {
            true => default,
            false => section.get(key),
        };
        value.map(PathBuf::from)
    }
}
