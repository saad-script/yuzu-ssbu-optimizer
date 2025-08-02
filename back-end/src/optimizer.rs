use crate::utils::io_error;
use include_dir::{include_dir, Dir};
use ini::Ini;
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

pub fn optimize_settings(config: &OptimizerConfig, user_profile: &UserProfile) -> io::Result<()> {
    let ssbu_settings_path = config
        .local_data
        .emu_folder
        .as_ref()
        .ok_or(io_error!(NotFound, "Emulator data folder not found"))?
        .join("config")
        .join("custom");
    load_bundled_dir(&BUNDLED_SSBU_SETTINGS, ssbu_settings_path)?;
    let main_config_settings_path = config
        .local_data
        .emu_folder
        .as_ref()
        .ok_or(io_error!(NotFound, "Emulator data folder not found"))?
        .join("config")
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
    let emu_name = config.get_emulator_name();
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
    Ok(())
}

pub fn optimize_mods(
    config: &OptimizerConfig,
    user_profile: &UserProfile,
    advanced_options: Vec<AdvancedOption>,
) -> io::Result<()> {
    let sdmc_path = config.emu_filesystem
        .sdmc_folder
        .as_ref()
        .ok_or(io_error!(NotFound, "Unable to find sdmc directory"))?;
    let skyline_path = sdmc_path
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
    let arc_mods_path = sdmc_path
        .join("ultimate");

    if advanced_options.contains(&AdvancedOption::CleanArc) {
        if arc_mods_path.is_dir() {
            log::info!("Removing arcropolis files...");
            fs::remove_dir_all(arc_config_path.as_path())?;
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
