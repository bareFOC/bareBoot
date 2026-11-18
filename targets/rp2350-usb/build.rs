use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR must be set by cargo");
    let src = Path::new("memory.x");
    let dst = Path::new(&out_dir).join("memory.x");

    fs::copy(src, &dst).expect("failed to copy memory.x into OUT_DIR");

    // cortex-m-rt's link.x does `INCLUDE memory.x`; point the linker at OUT_DIR.
    println!("cargo:rustc-link-search={}", out_dir);

    println!("cargo:rerun-if-changed=memory.x");
}
