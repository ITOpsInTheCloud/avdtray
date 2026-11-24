// Embeds avdtray.manifest (Common Controls v6, per-monitor DPI awareness, asInvoker)
// when building with the MSVC toolchain. No build dependencies required.
fn main() {
    println!("cargo:rerun-if-changed=avdtray.manifest");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") && target.contains("msvc") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let manifest = std::path::Path::new(&dir).join("avdtray.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
        // our manifest already contains the trustInfo (asInvoker) section
        println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:NO");
    }
}
