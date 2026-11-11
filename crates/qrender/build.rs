//! Compiles GLSL in `shaders/` to SPIR-V with `glslc` (GLSLC env var overrides the path).
use std::path::Path;
use std::process::Command;

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    let glslc = std::env::var("GLSLC").unwrap_or_else(|_| "glslc".into());
    println!("cargo:rerun-if-env-changed=GLSLC");
    println!("cargo:rerun-if-changed=shaders");
    for e in std::fs::read_dir("shaders").unwrap() {
        let p = e.unwrap().path();
        println!("cargo:rerun-if-changed={}", p.display());
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
        if !matches!(ext, "vert" | "frag" | "comp") {
            continue;
        }
        let name = p.file_name().unwrap().to_str().unwrap();
        let dst = Path::new(&out).join(format!("{name}.spv"));
        let st = Command::new(&glslc)
            .args(["--target-env=vulkan1.3", "-O", "-Werror"])
            .arg(&p)
            .arg("-o")
            .arg(&dst)
            .status()
            .unwrap_or_else(|e| panic!("running {glslc}: {e}"));
        assert!(st.success(), "glslc failed on {name}");
    }
}
