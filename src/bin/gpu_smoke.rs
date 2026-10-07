//! Dual-GPU-context smoke test (README step 4b).
//!
//! cymatrox creates its own wgpu device (`GpuContext`), independent of the
//! device Bevy's renderer creates. This binary brings both up in one process,
//! headless, to prove they coexist before layering any rendering on top.
//!
//! Run with: `cargo run --bin gpu_smoke`
//! Exits non-zero when either side fails to initialize.

use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::settings::RenderCreation;
use bevy::render::RenderPlugin;

fn main() {
    // 1. cymatrox owns its own wgpu device (wgpu 30).
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    match runtime.block_on(cymatrox::GpuContext::new()) {
        Ok(_) => println!("[gpu-smoke] cymatrox GpuContext initialized"),
        Err(e) => {
            eprintln!("[gpu-smoke] FAILED: cymatrox GpuContext: {e}");
            std::process::exit(1);
        }
    }

    // 2. Bevy owns a separate wgpu device (wgpu 29). Run headless: no winit,
    //    no window, no surface — just device initialization.
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<bevy::winit::WinitPlugin>()
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::default()),
                ..default()
            }),
    );
    app.finish();
    app.update();

    if app.world().contains_resource::<RenderDevice>() {
        println!("[gpu-smoke] Bevy render device initialized");
        println!("[gpu-smoke] OK: two independent wgpu devices coexist in one process");
    } else {
        eprintln!("[gpu-smoke] FAILED: Bevy render device was not created");
        std::process::exit(1);
    }
}
