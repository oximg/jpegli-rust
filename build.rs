use std::{
    env,
    hash::{Hash, Hasher},
    path::PathBuf,
    process::Command,
};

fn run(cmd: &mut Command) {
    let status = cmd
        .status()
        .expect("could not start CMake; install cmake and a C++ compiler");
    assert!(status.success(), "native build failed: {cmd:?}");
}

fn main() {
    println!("cargo:rerun-if-changed=native/shim.cc");
    println!("cargo:rerun-if-changed=native/shim.h");
    println!("cargo:rerun-if-changed=native/CMakeLists.txt");
    println!("cargo:rerun-if-changed=native/sources.json");
    println!("cargo:rerun-if-changed=native/vendor/.oximg-sources");
    for var in [
        "CC",
        "CXX",
        "CMAKE_GENERATOR",
        "OXIMG_JPEGLI_SANITIZE",
        "MACOSX_DEPLOYMENT_TARGET",
        "JPEGLI_BENCH_ABI",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    let target = env::var("TARGET").unwrap();
    assert_eq!(
        env::var("HOST").unwrap(),
        target,
        "POC supports native builds only"
    );
    assert!(
        target.contains("apple-darwin") || target.contains("linux-gnu"),
        "POC supports macOS and GNU/Linux only"
    );
    assert!(
        std::path::Path::new("native/vendor/.oximg-sources").exists(),
        "Run `python3 scripts/fetch-native.py` before building (Cargo does not download native sources)"
    );
    let digest = Command::new("cmake")
        .args(["-E", "sha256sum", "native/sources.json"])
        .output()
        .expect("cmake is required");
    assert!(
        digest.status.success(),
        "could not hash native source manifest"
    );
    let digest = String::from_utf8(digest.stdout).unwrap();
    let stamp = std::fs::read_to_string("native/vendor/.oximg-sources").unwrap();
    assert_eq!(
        Some(stamp.trim()),
        digest.split_whitespace().next(),
        "native source pins changed; move native/vendor aside and rerun scripts/fetch-native.py"
    );
    // Cargo package verification can reuse OUT_DIR for a relocated source tree.
    // CMake caches the absolute source path, so isolate its cache by source root.
    let mut source_hash = std::collections::hash_map::DefaultHasher::new();
    env::var_os("CARGO_MANIFEST_DIR")
        .unwrap()
        .hash(&mut source_hash);
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap())
        .join(format!("source-{:x}", source_hash.finish()));
    let build = out.join("native");
    let install = out.join("install");
    let mut config = Command::new("cmake");
    config
        .args(["-S", "native", "-B"])
        .arg(&build)
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", install.display()));
    let sanitize = env::var_os("OXIMG_JPEGLI_SANITIZE").is_some();
    // Benchmark-only: match the legacy jpegli-sys structs, using this SAME
    // native library for both wrappers. Normal builds retain libjpeg62 ABI.
    let abi = env::var("JPEGLI_BENCH_ABI").unwrap_or_else(|_| "62".into());
    assert!(
        abi == "62" || abi == "8",
        "unsupported jpegli benchmark ABI"
    );
    config.arg(format!("-DJPEGLI_LIBJPEG_LIBRARY_SOVERSION={abi}"));
    if target.contains("apple") {
        config.arg(format!(
            "-DCMAKE_OSX_DEPLOYMENT_TARGET={}",
            env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "11.0".into())
        ));
    }
    if sanitize {
        config
            .arg("-DCMAKE_C_FLAGS=-fsanitize=address,undefined -fno-omit-frame-pointer")
            .arg("-DCMAKE_CXX_FLAGS=-fsanitize=address,undefined -fno-omit-frame-pointer");
    } else {
        config.args(["-DCMAKE_C_FLAGS=", "-DCMAKE_CXX_FLAGS="]);
    }
    run(&mut config);
    run(Command::new("cmake")
        .arg("--build")
        .arg(&build)
        .args(["--target", "oximg_jpegli_shim", "--parallel"])
        .arg(env::var("NUM_JOBS").unwrap_or_else(|_| "2".into())));
    // Install only the three archives, not upstream tools or headers.
    run(Command::new("cmake").arg("--install").arg(&build));
    println!("cargo:rustc-link-search=native={}/lib", install.display());
    for lib in ["oximg_jpegli_shim", "jpegli-static", "hwy"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    println!(
        "cargo:rustc-link-lib={}",
        if target.contains("apple") {
            "c++"
        } else {
            "stdc++"
        }
    );
    if target.contains("linux") {
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
    }
    if sanitize {
        // Caller must use the matching clang driver to link sanitizer runtimes.
        println!("cargo:rustc-link-arg=-fsanitize=address,undefined");
    }
}
