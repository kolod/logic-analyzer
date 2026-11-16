//! Puts the application icon into the Windows executable (spec 014, user story 6).
//!
//! On other systems, and when the resource compiler is missing, this does nothing (a missing
//! compiler is a warning, never a failed build).
#![allow(clippy::print_stdout)]

#[cfg(windows)]
fn embed_icon() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/icon/app.ico");
    if let Err(error) = resource.compile() {
        println!("cargo:warning=the icon was not put into the executable: {error}");
    }
}

#[cfg(not(windows))]
fn embed_icon() {}

fn main() {
    println!("cargo:rerun-if-changed=assets/icon/app.ico");
    println!("cargo:rerun-if-changed=build.rs");
    embed_icon();
}
