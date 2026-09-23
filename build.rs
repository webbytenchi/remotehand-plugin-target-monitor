use std::{env, fs, path::PathBuf};

fn replace_once(source: String, needle: &str, replacement: &str, label: &str) -> String {
    if !source.contains(needle) {
        panic!("expected {label} marker was not found");
    }
    source.replacen(needle, replacement, 1)
}

fn main() {
    println!("cargo:rerun-if-changed=src/main.rs");

    let source = fs::read_to_string("src/main.rs")
        .expect("failed to read src/main.rs")
        .replace("\r\n", "\n");

    const STORE_MARKER: &str = "    let store = Arc::new(Mutex::new(MonitorStore::new()));\n";
    const REQUEST_ID_MARKER: &str = "    let mut host_request_id = 1u64;\n";
    const REPLACE_CONFIG_EXISTING_MARKER: &str = "                    let mut monitor = existing.clone();\n                    monitor.target.enabled = target.enabled;\n                    next.insert(target.id.clone(), monitor);\n                    continue;\n";

    let source_without_request_id = source.replacen(REQUEST_ID_MARKER, "", 1);
    let mut patched = replace_once(
        source_without_request_id,
        STORE_MARKER,
        &format!(
            "{STORE_MARKER}    let mut host_request_id = 1u64;\n    ensure_config_loaded(&store, &mut input, &output_tx, &mut host_request_id)?;\n"
        ),
        "MonitorStore startup marker",
    );

    patched = replace_once(
        patched,
        REPLACE_CONFIG_EXISTING_MARKER,
        "                    let target_id = target.id.clone();\n                    let mut monitor = existing.clone();\n                    monitor.target = target;\n                    next.insert(target_id, monitor);\n                    continue;\n",
        "replace_config existing-target marker",
    );

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is missing"));
    fs::write(out_dir.join("main_patched.rs"), patched)
        .expect("failed to write generated main_patched.rs");
}
