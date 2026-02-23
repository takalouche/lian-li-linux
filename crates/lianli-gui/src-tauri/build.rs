use std::path::Path;
use std::process::Command;

fn main() {
    let gui_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let src_dir = gui_root.join("src");

    // Tell cargo to re-run build.rs when any frontend source file changes
    println!("cargo:rerun-if-changed={}", src_dir.display());
    println!(
        "cargo:rerun-if-changed={}",
        gui_root.join("index.html").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        gui_root.join("package.json").display()
    );

    // Always rebuild frontend when build.rs is triggered
    println!("cargo:warning=Building frontend (bun run build)...");
    let status = Command::new("bun")
        .args(["run", "build"])
        .current_dir(gui_root)
        .status()
        .expect("Failed to run `bun run build`. Is bun installed?");
    assert!(status.success(), "Frontend build failed");

    tauri_build::build()
}
