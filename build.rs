//! Build script.
//!
//! On Windows, embed the application icon into the executable so Explorer, the
//! taskbar and the Start menu show it (the icon is not something
//! cargo-packager can inject into an already-built binary).

fn main() {
    #[cfg(windows)]
    {
        winresource::WindowsResource::new()
            .set_icon("build/icons/icon.ico")
            .compile()
            .expect("failed to embed the Windows application icon");
    }
}
