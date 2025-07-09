use std::{env, fs, path::Path};

const BUNDLED_DATA_URL: &str =
    "https://drive.google.com/uc?export=download&id=1OVsIizFF1zZWNfoLiX5gzkzjNaaUbQET";

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let bundled_data_zip_path = Path::new(&out_dir).join("bundled_data.zip");

    // Download the zip file
    let mut resp = reqwest::blocking::get(BUNDLED_DATA_URL).expect("Failed to download zip");
    let mut out = fs::File::create(&bundled_data_zip_path).expect("Failed to create zip file");
    resp.copy_to(&mut out).expect("Failed to copy content");

    // Extract it
    let mut zip_file = fs::File::open(&bundled_data_zip_path).expect("Cannot open zip");
    let mut archive = zip::ZipArchive::new(&mut zip_file).expect("Failed to read zip archive");

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = Path::new(".").join(file.name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).unwrap();
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p).unwrap();
                }
            }
            let mut outfile = fs::File::create(&outpath).unwrap();
            std::io::copy(&mut file, &mut outfile).unwrap();
        }
    }

    tauri_build::build()
}
