#[cfg(feature = "desktop")]
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let compiler = cc::Build::new().cpp(true).get_compiler();
        println!("cargo:rustc-env=PROBE_CXX={}", compiler.path().display());
        let includes = compiler
            .env()
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("INCLUDE"));
        let sdk = includes
            .map(|(_, value)| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        let sdk = sdk
            .split(';')
            .find(|path| path.to_ascii_lowercase().contains("windows kits"))
            .unwrap_or("unavailable");
        println!("cargo:rustc-env=PROBE_WINDOWS_SDK_INCLUDE={sdk}");
    } else {
        println!("cargo:rustc-env=PROBE_CXX=unavailable");
        println!("cargo:rustc-env=PROBE_WINDOWS_SDK_INCLUDE=unavailable");
    }
    #[cfg(feature = "desktop")]
    build_desktop();
}

#[cfg(feature = "desktop")]
fn build_desktop() {
    // An owned procedural test icon avoids importing either product's branding/resources.
    let icon = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("probe.ico");
    std::fs::write(&icon, test_icon()).expect("write generated probe icon");
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new().window_icon_path(&icon)),
    )
    .expect("Tauri probe resources");
    // The context macro also loads an icon. Its supported config overlay points at the same
    // generated resource, keeping binary assets out of the source tree.
    println!(
        "cargo:rustc-env=TAURI_CONFIG={}",
        serde_json::json!({"bundle": {"icon": [icon]}})
    );
}

#[cfg(feature = "desktop")]
fn test_icon() -> Vec<u8> {
    let mut bytes = vec![0, 0, 1, 0, 1, 0, 16, 16, 0, 0, 1, 0, 32, 0];
    bytes.extend_from_slice(&1128_u32.to_le_bytes());
    bytes.extend_from_slice(&22_u32.to_le_bytes());
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&16_i32.to_le_bytes());
    bytes.extend_from_slice(&32_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&[0_u8; 24]);
    for y in 0..16 {
        for x in 0..16 {
            bytes.extend_from_slice(if x < 8 && y < 8 {
                &[220, 180, 20, 255]
            } else {
                &[40, 40, 40, 255]
            });
        }
    }
    bytes.extend_from_slice(&[0_u8; 64]);
    bytes
}
