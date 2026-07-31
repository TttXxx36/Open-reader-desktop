use std::{env, fs, path::PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is required"));
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required"));
    let icon_dir = manifest_dir.join("icons");
    fs::create_dir_all(&icon_dir).expect("unable to create the icon directory");

    let ico_path = icon_dir.join("icon.ico");
    let ico = STANDARD
        .decode(include_str!("icons/icon.ico.b64").trim())
        .expect("embedded ICO should be valid base64");
    fs::write(&ico_path, &ico).expect("unable to write the ICO resource");

    let png_path = icon_dir.join("icon.png");
    let png = STANDARD
        .decode(include_str!("icons/icon.png.b64").trim())
        .expect("embedded PNG should be valid base64");
    fs::write(&png_path, png).expect("unable to write the PNG resource");

    let windows_icon_path = out_dir.join("open-reader.ico");
    fs::write(&windows_icon_path, ico).expect("unable to write the build icon");

    tauri_build::try_build(
        tauri_build::Attributes::new().windows_attributes(
            tauri_build::WindowsAttributes::new().window_icon_path(windows_icon_path),
        ),
    )
    .expect("failed to run tauri build script");
}
