use std::{env, fs, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is required"));
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required"));
    let icon_dir = manifest_dir.join("icons");
    let ico_path = icon_dir.join("icon.ico");
    let png_path = icon_dir.join("icon.png");

    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");

    let ico = fs::read(&ico_path).unwrap_or_else(|error| {
        panic!(
            "unable to read Windows ICO resource {}: {error}",
            ico_path.display()
        )
    });
    fs::read(&png_path).unwrap_or_else(|error| {
        panic!(
            "unable to read PNG resource {}: {error}",
            png_path.display()
        )
    });

    let windows_icon_path = out_dir.join("open-reader.ico");
    fs::write(&windows_icon_path, &ico).expect("unable to write the build icon");

    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new().window_icon_path(windows_icon_path),
    ))
    .expect("failed to run tauri build script");
}
