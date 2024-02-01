use std::path::PathBuf;

#[macro_export]
macro_rules! io_error {
    ($kind:ident, $($arg:tt)*) => {{
        std::io::Error::new(std::io::ErrorKind::$kind, format!($($arg)*))
    }};
}
pub use io_error;

pub fn get_default_yuzu_folder() -> Option<PathBuf> {
    let roaming_app_data = tauri::api::path::data_dir()?;
    let yuzu_folder = roaming_app_data.join("yuzu");
    return Some(yuzu_folder);
}
