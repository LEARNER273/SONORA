use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Android: link the system AAudio + logging libraries (available since API 26).
    if target_os == "android" {
        println!("cargo:rustc-link-lib=dylib=aaudio");
        println!("cargo:rustc-link-lib=dylib=log");
    }
}
