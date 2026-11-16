//! Compiles GLSL in `shaders/` to SPIR-V with `glslc` (GLSLC env var overrides the path).
//! With `QSP_SPIRV_DIR` set, copies prebuilt `<name>.spv` files from there instead (SPIR-V is
//! platform independent, so release CI compiles the shaders once).
//!
//! Also selects the backend: cfg `qvk` (Vulkan) unless building for wasm32 or
//! with the `wgpu` feature, in which case the WGSL wgpu backend is used and no
//! GLSL is compiled.
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(qvk)");
    let wasm = std::env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|a| a == "wasm32");
    let wgpu = std::env::var("CARGO_FEATURE_WGPU").is_ok();
    println!("cargo:rerun-if-changed=shaders");
    if wasm || wgpu {
        return;
    }
    println!("cargo:rustc-cfg=qvk");
    let out = std::env::var("OUT_DIR").unwrap();
    let glslc = std::env::var("GLSLC").unwrap_or_else(|_| "glslc".into());
    println!("cargo:rerun-if-env-changed=GLSLC");
    println!("cargo:rerun-if-env-changed=QSP_SPIRV_DIR");
    let prebuilt = std::env::var("QSP_SPIRV_DIR").ok();
    for e in std::fs::read_dir("shaders").unwrap() {
        let p = e.unwrap().path();
        println!("cargo:rerun-if-changed={}", p.display());
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
        if !matches!(ext, "vert" | "frag" | "comp") {
            continue;
        }
        let name = p.file_name().unwrap().to_str().unwrap();
        let dst = Path::new(&out).join(format!("{name}.spv"));
        if let Some(dir) = &prebuilt {
            let src = Path::new(dir).join(format!("{name}.spv"));
            std::fs::copy(&src, &dst).unwrap_or_else(|e| panic!("copying {}: {e}", src.display()));
            continue;
        }
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
