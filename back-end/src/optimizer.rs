use crate::utils::io_error;
use include_dir::{include_dir, Dir};
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::config::{AdvancedOption, OptimizerConfig, SSBU_TITLE_ID};
use crate::profile::UserProfile;

static BUNDLED_ARC_CONFIG: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundled_data/arc_config");
static BUNDLED_ARC_MODS: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundled_data/arc_mods");
static BUNDLED_SKYLINE: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundled_data/skyline");
static BUNDLED_SAVE_DATA: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundled_data/save");
static BUNDLED_SSBU_SETTINGS: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundled_data/ssbu_settings");

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

pub fn optimize_settings(config: &OptimizerConfig) -> io::Result<()> {
    let ssbu_settings_path = config
        .local_data
        .yuzu_folder
        .as_ref()
        .ok_or(io_error!(NotFound, "yuzu folder not found"))?
        .join("config")
        .join("custom");
    load_bundled_dir(&BUNDLED_SSBU_SETTINGS, ssbu_settings_path)?;
    Ok(())
}

pub fn optimize_mods(
    config: &OptimizerConfig,
    user_profile: &UserProfile,
    advanced_options: Vec<AdvancedOption>,
) -> io::Result<()> {
    let skyline_path = config
        .local_data
        .yuzu_folder
        .as_ref()
        .ok_or(io_error!(NotFound, "yuzu folder not found"))?
        .join("sdmc")
        .join("atmosphere")
        .join("contents")
        .join(SSBU_TITLE_ID);

    if advanced_options.contains(&AdvancedOption::CleanSkyline) {
        if skyline_path.is_dir() {
            log::info!("Removing skyline files...");
            fs::remove_dir_all(skyline_path.as_path())?;
        }
    }

    let arc_config_path = config.get_arc_config_folder(&user_profile)?;
    let arc_mods_path = config
        .local_data
        .yuzu_folder
        .as_ref()
        .ok_or(io_error!(NotFound, "yuzu folder not found"))?
        .join("sdmc")
        .join("ultimate");

    if advanced_options.contains(&AdvancedOption::CleanArc) {
        if arc_mods_path.is_dir() {
            log::info!("Removing arcropolis files...");
            fs::remove_dir_all(arc_mods_path.as_path())?;
        }
    }

    let arc_mods_path = arc_mods_path.join("mods");

    load_bundled_dir(&BUNDLED_SKYLINE, skyline_path)?;

    load_bundled_dir(&BUNDLED_ARC_CONFIG, arc_config_path)?;

    load_bundled_dir(&BUNDLED_ARC_MODS, arc_mods_path)?;

    Ok(())
}

pub fn optimize_save(config: &OptimizerConfig, user_profile: &UserProfile) -> io::Result<()> {
    let save_file_path = config.get_save_folder(&user_profile)?;
    load_bundled_dir(&BUNDLED_SAVE_DATA, save_file_path)?;
    Ok(())
}
