use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    generate_builtin_resource_index();
    tauri_build::build();
}

fn generate_builtin_resource_index() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));
    let index_path = out_dir.join("builtin_resource_index.rs");

    let mut resources = Vec::new();
    collect_resource_files(
        &manifest_dir,
        &manifest_dir.join("builtin-packs"),
        &mut resources,
    );
    collect_resource_files(
        &manifest_dir,
        &manifest_dir.join("prompt-templates"),
        &mut resources,
    );
    resources.sort_by(|left, right| left.0.cmp(&right.0));

    let mut output = String::from(
        "pub struct BuiltinResource {\n    pub path: &'static str,\n    pub contents: &'static str,\n}\n\npub const BUILTIN_RESOURCES: &[BuiltinResource] = &[\n",
    );
    for (relative_path, absolute_path) in resources {
        output.push_str("    BuiltinResource {\n");
        output.push_str(&format!(
            "        path: {:?},\n",
            relative_path.replace('\\', "/")
        ));
        output.push_str(&format!(
            "        contents: include_str!({:?}),\n",
            absolute_path.display().to_string()
        ));
        output.push_str("    },\n");
    }
    output.push_str("];\n");

    fs::write(index_path, output).expect("write builtin resource index");
}

fn collect_resource_files(
    manifest_dir: &Path,
    root: &Path,
    resources: &mut Vec<(String, PathBuf)>,
) {
    println!("cargo:rerun-if-changed={}", root.display());
    if !root.exists() {
        return;
    }

    for entry in fs::read_dir(root).expect("read resource dir") {
        let entry = entry.expect("resource dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_resource_files(manifest_dir, &path, resources);
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(manifest_dir)
            .expect("resource under manifest dir")
            .to_string_lossy()
            .to_string();
        resources.push((relative, path));
    }
}
