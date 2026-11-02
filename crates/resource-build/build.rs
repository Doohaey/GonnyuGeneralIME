use sha2::{Digest, Sha256};
use std::path::Path;

const WORKSPACE_PACKAGES: &[&str] = &[
    "gannyu-input-cli",
    "gannyu-input-core",
    "gannyu-input-ffi",
    "gannyu-sanitize-binary",
    "gonnyu-resource-build",
];
const CACHE_BASELINE_VERSION: &str = "1.2.4-pre.1";
const CACHE_BASELINE_BUILD_SCRIPT: &[u8] = include_bytes!("build.rs.cache-baseline");

fn collect(path: &Path, root: &Path, digest: &mut Sha256) {
    let mut paths = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(&path, root, digest);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            add(&path, root, digest);
        }
    }
}

fn add_bytes(path: &Path, root: &Path, digest: &mut Sha256, contents: &[u8]) {
    println!("cargo:rerun-if-changed={}", path.display());
    digest.update(
        path.strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
            .as_bytes(),
    );
    digest.update(contents);
}

fn add(path: &Path, root: &Path, digest: &mut Sha256) {
    add_bytes(path, root, digest, &std::fs::read(path).unwrap());
}

fn normalized_lock(lock: &str) -> String {
    let mut package = "";
    let mut output = String::with_capacity(lock.len());
    for line in lock.lines() {
        if line == "[[package]]" {
            package = "";
        } else if let Some(name) = line
            .strip_prefix("name = \"")
            .and_then(|v| v.strip_suffix('\"'))
        {
            package = name;
        }
        if WORKSPACE_PACKAGES.contains(&package) && line.starts_with("version = ") {
            output.push_str(&format!("version = \"{CACHE_BASELINE_VERSION}\""));
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    output
}

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().unwrap().parent().unwrap();
    let mut digest = Sha256::new();
    for relative in ["crates/core/src", "crates/resource-build/src"] {
        collect(&root.join(relative), root, &mut digest);
    }

    let lock_path = root.join("Cargo.lock");
    let lock = std::fs::read_to_string(&lock_path).unwrap();
    add_bytes(
        &lock_path,
        root,
        &mut digest,
        normalized_lock(&lock).as_bytes(),
    );
    for relative in ["crates/core/Cargo.toml", "crates/resource-build/Cargo.toml"] {
        add(&root.join(relative), root, &mut digest);
    }

    // Keep the fingerprint input stable so existing compiled regions stay valid.
    let build_script = root.join("crates/resource-build/build.rs");
    add_bytes(
        &build_script,
        root,
        &mut digest,
        CACHE_BASELINE_BUILD_SCRIPT,
    );
    println!("cargo:rerun-if-changed=crates/resource-build/build.rs.cache-baseline");

    let compiler = std::process::Command::new(std::env::var_os("RUSTC").unwrap())
        .arg("-vV")
        .output()
        .unwrap();
    assert!(compiler.status.success());
    digest.update(compiler.stdout);
    digest.update(std::env::var("TARGET").unwrap().as_bytes());
    println!(
        "cargo:rustc-env=GANNYU_RESOURCE_COMPILER_KEY={:x}",
        digest.finalize()
    );
}
