fn main() {
    println!("cargo:rerun-if-env-changed=KEYBOARD_GOBLOON_VERSION");
    let version = std::env::var("KEYBOARD_GOBLOON_VERSION")
        .ok()
        .or_else(git_version)
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());
    // Read by `VERSION` in main.rs: the same as the app's (keyboard-gobloon build.rs).
    println!("cargo:rustc-env=KEYBOARD_GOBLOON_VERSION={version}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icon = "../../assets/icon/KeyboardGobloon.ico";
        println!("cargo:rerun-if-changed={icon}");
        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon)
            .set("ProductName", "Keyboard Gobloon")
            .set("FileDescription", "Keyboard Gobloon Settings")
            .set("InternalName", "KeyboardGobloonSettings")
            .set("OriginalFilename", "KeyboardGobloonSettings.exe")
            .set("CompanyName", "Denis Ibragimov")
            .set(
                "LegalCopyright",
                "Copyright (c) 2026 Denis Ibragimov. MIT License.",
            )
            .set("ProductVersion", &version);
        res.compile().expect("failed to embed Windows resources");
    }
}

/// `0.1.0-alpha.4+3.g728daf5` from `git describe`, like the app's build.rs.
fn git_version() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["describe", "--tags", "--long", "--match", "v[0-9]*"])
        .output()
        .ok()?;
    let described = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let mut parts = described.rsplitn(3, '-');
    let (hash, count, tag) = (parts.next()?, parts.next()?, parts.next()?);
    Some(format!("{}+{count}.{hash}", tag.trim_start_matches('v')))
}
