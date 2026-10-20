//! Android link fixes for the C++ speech engine.
//!
//! transcribe-cpp's CMake build hands cargo the system libraries it wants on
//! a Linux-like target: `stdc++`, `pthread`, `m`, `dl`. Bionic folds pthread
//! into libc and the NDK ships no libpthread at all, so the link fails on
//! `-lpthread`. An empty archive by that name satisfies the linker; the real
//! C++ runtime is Android's libc++, linked here as well (cargo-ndk copies
//! libc++_shared.so next to the app's library).

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let stubs = out.join("android-stubs");
    fs::create_dir_all(&stubs).expect("stub dir");
    // The 8-byte magic alone is a valid, empty ar archive.
    for name in ["libpthread.a", "librt.a"] {
        fs::write(stubs.join(name), b"!<arch>\n").expect("stub archive");
    }
    println!("cargo:rustc-link-search=native={}", stubs.display());
    if cfg!(feature = "speech") {
        println!("cargo:rustc-link-lib=dylib=c++_shared");
    }
}
