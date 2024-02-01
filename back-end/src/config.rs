use crate::profile::{self, UserProfile};
use crate::utils::{self, io_error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const SSBU_TITLE_ID: &str = "01006A800016E000";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Optimization {
    Settings,
    Mods,
    Save,
}

impl FromStr for Optimization {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Settings" => Ok(Optimization::Settings),
            "Mods" => Ok(Optimization::Mods),
            "Save" => Ok(Optimization::Save),
            _ => Err(()),
        }
    }
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
}

impl OptimizerConfig {
    pub fn load(app_data_dir: &Path) -> Self {
        let mut local_data = LocalMetaData::load(app_data_dir);
        let user_profiles = match &local_data.yuzu_folder {
            Some(f) => profile::parse_user_profiles_save_file(f),
            None => Err(io_error!(NotFound, "Unable to load user profiles")),
        };
        match user_profiles {
            Ok(users) => {
                if let Some(selected_user) = local_data.selected_user_profile.as_ref() {
                    if !users.contains(selected_user) {
                        if let Some(default_user) = users.get(0) {
                            local_data.selected_user_profile = Some(default_user.clone());
                        } else {
                            local_data.selected_user_profile = None;
                        }
                    }
                }
                OptimizerConfig {
                    local_data: local_data,
                    user_profiles: users,
                }
            }
            _ => {
                let default_yuzu_folder =
                    utils::get_default_yuzu_folder().expect("Unable to find default yuzu folder");
                Self::from_data_folder(app_data_dir, default_yuzu_folder)
            }
        }
    }
    pub fn from_data_folder(app_data_dir: &Path, yuzu_folder: PathBuf) -> Self {
        let mut local_data = LocalMetaData::load(app_data_dir);
        let user_profiles = profile::parse_user_profiles_save_file(&yuzu_folder);
        match user_profiles {
            Ok(u) => {
                local_data.yuzu_folder = Some(yuzu_folder);
                if let Some(su) = local_data.selected_user_profile.as_ref() {
                    if !u.contains(su) {
                        local_data.selected_user_profile = None;
                    }
                }
                OptimizerConfig {
                    local_data: local_data,
                    user_profiles: u,
                }
            }
            _ => {
                local_data.yuzu_folder = None;
                local_data.selected_user_profile = None;
                OptimizerConfig {
                    local_data: local_data,
                    user_profiles: vec![],
                }
            }
        }
    }
}

impl OptimizerConfig {
    pub fn get_save_folder(&self, user_profile: &UserProfile) -> io::Result<PathBuf> {
        Ok(self
            .local_data
            .yuzu_folder
            .as_ref()
            .ok_or(io_error!(NotFound, "yuzu folder not found"))?
            .join("nand")
            .join("user")
            .join("save")
            .join("0000000000000000")
            .join(user_profile.get_uuid_yuzu_storage_string())
            .join(SSBU_TITLE_ID))
    }

    pub fn get_arc_config_folder(&self, user_profile: &UserProfile) -> io::Result<PathBuf> {
        let uuids = user_profile.get_uuid_arc_storage_strings();
        Ok(self
            .local_data
            .yuzu_folder
            .as_ref()
            .ok_or(io_error!(NotFound, "yuzu folder not found"))?
            .join("sdmc")
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
    pub yuzu_folder: Option<PathBuf>,
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
