//! Build script: compiles the Slint UI and embeds the Windows icons.

fn main() {
    // "fluent-dark" only affects the std widgets we use (the scroll bars).
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint build failed");

    // Icon resource 1 is the app icon (exe, window and tray). Resource 2 is
    // the power icon used by the shutdown shortcut.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon_with_id("assets/app.ico", "1");
        res.set_icon_with_id("assets/power.ico", "2");
        res.compile().expect("couldn't embed the icons");
    }
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets/power.ico");
}
