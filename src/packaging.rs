//! CPU-only application packaging. Does not open a window or start the game.
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn write(path: &Path, data: &[u8], permissions: Option<fs::Permissions>) -> Result<()> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temp = path.with_extension(format!("{}-{stamp}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(data)?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub fn macos_app(executable: &Path, cache: &Path, output: &Path) -> Result<()> {
    ensure!(
        output.extension().is_some_and(|ext| ext == "app"),
        "Application output must end in .app"
    );
    let icon = crate::assets::icons::read(cache, "minecraft.icns")?;
    ensure!(
        icon.starts_with(b"icns") && icon.len() >= 8,
        "Invalid official ICNS icon"
    );
    let bytes = fs::read(executable).context("Reading application executable")?;
    let contents = output.join("Contents");
    fs::create_dir_all(contents.join("MacOS"))?;
    fs::create_dir_all(contents.join("Resources"))?;
    write(&contents.join("Resources/OptiMC.icns"), &icon, None)?;
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>OptiMC</string>
<key>CFBundleIdentifier</key><string>dev.optimc.client</string>
<key>CFBundleName</key><string>OptiMC</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>{}</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleIconFile</key><string>OptiMC.icns</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
"#,
        env!("CARGO_PKG_VERSION")
    );
    write(&contents.join("Info.plist"), plist.as_bytes(), None)?;
    let legal = contents.join("Resources/Legal");
    fs::create_dir_all(&legal)?;
    for (name, text) in [
        (
            "THIRD_PARTY_NOTICES.md",
            include_str!("../THIRD_PARTY_NOTICES.md"),
        ),
        (
            "Zoomify-LGPL-3.0.txt",
            include_str!("../LICENSES/Zoomify-LGPL-3.0.txt"),
        ),
        ("GPL-3.0.txt", include_str!("../LICENSES/GPL-3.0.txt")),
        ("Sodium.txt", include_str!("../LICENSES/Sodium.txt")),
        ("SkinLayers.txt", include_str!("../LICENSES/SkinLayers.txt")),
    ] {
        write(&legal.join(name), text.as_bytes(), None)?;
    }

    write(
        &contents.join("MacOS/OptiMC"),
        &bytes,
        Some(fs::metadata(executable)?.permissions()),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_copies_verified_icon_and_binary_without_running_either() {
        use sha1::{Digest, Sha1};
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("optimc-package-{}-{stamp}", std::process::id()));
        let root = crate::assets::root(&dir);
        fs::create_dir_all(root.join("resources/assets/icons")).unwrap();
        let icon = b"icns\0\0\0\x08";
        let hash = format!("{:x}", Sha1::digest(icon));
        let index = serde_json::json!({"objects":{"icons/minecraft.icns":{"size":8,"hash":hash}}});
        fs::write(root.join("asset-index.json"), index.to_string()).unwrap();
        fs::write(root.join("resources/assets/icons/minecraft.icns"), icon).unwrap();
        let executable = dir.join("source");
        fs::write(&executable, b"test executable, never run").unwrap();
        let app = dir.join("OptiMC.app");
        macos_app(&executable, &dir, &app).unwrap();
        assert_eq!(
            fs::read(app.join("Contents/Resources/OptiMC.icns")).unwrap(),
            icon
        );
        assert_eq!(
            fs::read(app.join("Contents/MacOS/OptiMC")).unwrap(),
            fs::read(&executable).unwrap()
        );
        let plist = fs::read_to_string(app.join("Contents/Info.plist")).unwrap();
        assert!(plist.contains("<key>CFBundleIconFile</key><string>OptiMC.icns</string>"));
        fs::write(
            root.join("resources/assets/icons/minecraft.icns"),
            b"corrupt!",
        )
        .unwrap();
        assert!(macos_app(&executable, &dir, &app).is_err());
        assert_eq!(
            fs::read(app.join("Contents/Resources/OptiMC.icns")).unwrap(),
            icon
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
