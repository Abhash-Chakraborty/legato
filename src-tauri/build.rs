fn main() {
    generate_tray_translations();

    // Stage transcribe-cpp's runtime DLLs (and the dynamically loaded ggml
    // backend modules) for the installer. Self-gates on the dynamic-backends
    // posture used on x86_64; it's a no-op for the static ARM64 build, where
    // there is nothing to ship.
    stage_transcribe_runtime_libs();

    // Must run after transcribe staging because that helper recreates transcribe-libs/.
    stage_vc_runtime_dlls();

    tauri_build::build()
}

/// Stage the MSVC runtime DLLs into `transcribe-libs/` for app-local deployment.
///
/// Handy's native stack links the VC++ runtime dynamically (/MD). Shipping the
/// DLLs beside `handy.exe` covers machines with no redistributable installed and
/// machines whose system redist is older than the CI toolset (issue #1527).
///
/// Driven by `HANDY_VC_REDIST_DIRS`, set by CI to the redist dirs from the same
/// Visual Studio install that compiled the native code. Copies only the runtime
/// DLL families Handy imports and no-ops when the env var is unset.
fn stage_vc_runtime_dlls() {
    use std::path::PathBuf;

    println!("cargo:rerun-if-env-changed=HANDY_VC_REDIST_DIRS");

    let Some(redist_dirs) = std::env::var_os("HANDY_VC_REDIST_DIRS") else {
        return;
    };

    let dest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("transcribe-libs");
    std::fs::create_dir_all(&dest).expect("create transcribe-libs staging dir");

    let mut copied: Vec<String> = Vec::new();
    for dir in std::env::split_paths(&redist_dirs) {
        for entry in std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("HANDY_VC_REDIST_DIRS: read {}: {e}", dir.display()))
            .flatten()
        {
            let src = entry.path();
            let name = src
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let lower = name.to_lowercase();
            let wanted = lower.ends_with(".dll")
                && (lower.starts_with("msvcp140")
                    || lower.starts_with("vcruntime140")
                    || lower.starts_with("vcomp140"));
            if wanted {
                std::fs::copy(&src, dest.join(&name))
                    .unwrap_or_else(|e| panic!("copy {}: {e}", src.display()));
                copied.push(lower);
            }
        }
    }

    // Fail the build rather than ship an installer that regresses issue #1527.
    for required in ["msvcp140.dll", "vcruntime140.dll"] {
        if !copied.iter().any(|n| n == required) {
            panic!(
                "HANDY_VC_REDIST_DIRS is set but {required} was not found in it; \
                 the app-local VC++ runtime would be incomplete and Handy would \
                 crash on machines without a current redist (issue #1527)"
            );
        }
    }
    println!(
        "cargo:warning=Staged {} VC++ runtime DLL(s) for app-local deployment",
        copied.len()
    );
}

/// Stage transcribe-cpp's runtime DLLs into `transcribe-libs/` so the installer
/// can ship them beside `handy.exe` (DLLs resolve from the exe dir).
///
/// Source dirs arrive as `DEP_TRANSCRIBE_CPP_*`: the sys crate (`links =
/// "transcribe"`) emits its install dirs and the wrapper (`links =
/// "transcribe_cpp"`) forwards them one hop to us — the only way that metadata
/// crosses cargo's one-hop `links` boundary. The keys exist only in a shared /
/// dynamic-backends build; the static ARM64 build leaves them unset, so this
/// is a no-op there. `RUNTIME_DIR` (core libs) and `MODULE_DIR` (dynamically
/// loaded ggml modules) may be the same dir — the `BTreeSet` below dedups them.
fn stage_transcribe_runtime_libs() {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    println!("cargo:rerun-if-env-changed=DEP_TRANSCRIBE_CPP_RUNTIME_DIR");
    println!("cargo:rerun-if-env-changed=DEP_TRANSCRIBE_CPP_MODULE_DIR");

    // Present only in a shared posture. A static build has nothing to ship.
    let Some(runtime_dir) = std::env::var_os("DEP_TRANSCRIBE_CPP_RUNTIME_DIR") else {
        return;
    };

    // transcribe-cpp publishes its runtime layout in up to two directories:
    //   RUNTIME_DIR : the shared libs to load (transcribe + core ggml / ggml-base)
    //   MODULE_DIR  : the dynamically loaded ggml backend modules (the per-ISA
    //                 ggml-cpu-* and ggml-vulkan), dynamic-backends only. Often
    //                 — but not always — the SAME directory as RUNTIME_DIR.
    // BOTH must sit next to the executable, or init_backends_default() finds the
    // core libs but zero loadable compute backends and registers no devices.
    let mut dirs = BTreeSet::new();
    dirs.insert(PathBuf::from(runtime_dir));
    if let Some(module_dir) = std::env::var_os("DEP_TRANSCRIBE_CPP_MODULE_DIR") {
        dirs.insert(PathBuf::from(module_dir));
    }

    let dest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("transcribe-libs");
    // Recreate clean so a renamed or dropped ggml module can never linger in the
    // package from a previous build.
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::create_dir_all(&dest).expect("create transcribe-libs staging dir");

    // Collect by name across both dirs, so a DLL present in both is staged once.
    let mut libs: std::collections::BTreeMap<String, PathBuf> = Default::default();
    for dir in &dirs {
        println!("cargo:rerun-if-changed={}", dir.display());
        for entry in std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            .flatten()
        {
            let src = entry.path();
            let name = src.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.ends_with(".dll") {
                libs.insert(name.to_string(), src);
            }
        }
    }

    let mut copied = 0usize;
    for (name, src) in &libs {
        std::fs::copy(src, dest.join(name))
            .unwrap_or_else(|e| panic!("copy {}: {e}", src.display()));
        copied += 1;
    }
    if copied == 0 {
        panic!(
            "no transcribe-cpp runtime libraries found under {dirs:?}; a shared / \
             dynamic-backends build must ship them or the app registers zero \
             compute devices"
        );
    }
    println!("cargo:warning=Staged {copied} transcribe-cpp runtime library file(s)");
}

/// Generate the tray menu strings from the frontend's English locale file.
///
/// Source of truth: the "tray" section of src/i18n/locales/en/translation.json.
/// Each key becomes a snake_case field of `TrayStrings`.
fn generate_tray_translations() {
    use std::fs;
    use std::path::Path;

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let json_path = Path::new("../src/i18n/locales/en/translation.json");

    println!("cargo:rerun-if-changed={}", json_path.display());

    let content = fs::read_to_string(json_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let tray = parsed.get("tray").unwrap().as_object().unwrap();
    let fields: Vec<_> = tray
        .iter()
        .map(|(key, value)| (camel_to_snake(key), value.as_str().unwrap_or("")))
        .collect();

    // Generate code
    let mut out = String::from(
        "// Auto-generated from src/i18n/locales/en/translation.json - do not edit\n\n",
    );

    // Struct
    out.push_str("#[derive(Debug, Clone)]\npub struct TrayStrings {\n");
    for (rust_field, _) in &fields {
        out.push_str(&format!("    pub {rust_field}: String,\n"));
    }
    out.push_str("}\n\n");

    // Constructor
    out.push_str("pub fn tray_strings() -> TrayStrings {\n    TrayStrings {\n");
    for (rust_field, val) in &fields {
        out.push_str(&format!(
            "        {rust_field}: \"{}\".to_string(),\n",
            escape_string(val)
        ));
    }
    out.push_str("    }\n}\n");

    fs::write(Path::new(&out_dir).join("tray_translations.rs"), out).unwrap();

    println!(
        "cargo:warning=Generated tray strings: {} fields",
        fields.len()
    );
}

fn camel_to_snake(s: &str) -> String {
    s.chars()
        .enumerate()
        .fold(String::new(), |mut acc, (i, c)| {
            if c.is_uppercase() && i > 0 {
                acc.push('_');
            }
            acc.push(c.to_lowercase().next().unwrap());
            acc
        })
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
