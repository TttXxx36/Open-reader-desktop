use std::{env, fs, path::PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is required"));
    let icon_path = out_dir.join("open-reader.ico");
    let icon = STANDARD
        .decode(include_str!("icons/icon.ico.b64").trim())
        .expect("embedded icon should be valid base64");
    fs::write(&icon_path, icon).expect("unable to write the build icon");

    tauri_build::try_build(
        tauri_build::Attributes::new().windows_attributes(
            tauri_build::WindowsAttributes::new().window_icon_path(icon_path),
        ),
    )
    .expect("failed to run tauri build script");
}
