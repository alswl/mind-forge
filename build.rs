use std::process::Command;

fn main() {
    // Refresh the embedded git SHA when HEAD moves.
    println!("cargo:rerun-if-changed=.git/HEAD");

    let git_sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CARGO_GIT_SHA={git_sha}");

    let build_date = Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CARGO_BUILD_DATE={build_date}");

    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let rustc_ver = Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CARGO_RUSTC={}", rustc_ver.trim());

    // ── KenLM vendored C++ query-only library ──────────────────────────────
    compile_kenlm();
}

#[allow(dead_code)]
fn compile_kenlm() {
    let mut build = cc::Build::new();

    let kenlm_dir = "vendor/kenlm";
    let lm_dir = "vendor/kenlm/lm";
    let util_dir = "vendor/kenlm/util";
    let dc_dir = "vendor/kenlm/util/double-conversion";

    // Trigger rebuild when vendored sources change
    println!("cargo:rerun-if-changed=vendor/kenlm/mf_kenlm_shim.cc");
    println!("cargo:rerun-if-changed=vendor/kenlm/mf_kenlm_shim.h");
    println!("cargo:rerun-if-changed=vendor/kenlm/lm/bhiksha.cc");
    println!("cargo:rerun-if-changed=vendor/kenlm/util/ersatz_progress.cc");

    // shim (at root level)
    build.file(format!("{}/mf_kenlm_shim.cc", kenlm_dir));

    // lm/ sources (query-only subset)
    let lm_sources = [
        "bhiksha.cc",
        "binary_format.cc",
        "config.cc",
        "lm_exception.cc",
        "model.cc",
        "quantize.cc",
        "read_arpa.cc",
        "search_hashed.cc",
        "search_trie.cc",
        "sizes.cc",
        "trie.cc",
        "trie_sort.cc",
        "value_build.cc",
        "virtual_interface.cc",
        "vocab.cc",
    ];
    for src in &lm_sources {
        build.file(format!("{}/{}", lm_dir, src));
    }

    // util/ sources
    let util_sources = [
        "bit_packing.cc",
        "ersatz_progress.cc",
        "exception.cc",
        "file.cc",
        "file_piece.cc",
        "float_to_string.cc",
        "integer_to_string.cc",
        "mmap.cc",
        "murmur_hash.cc",
        "parallel_read.cc",
        "pool.cc",
        "read_compressed.cc",
        "scoped.cc",
        "spaces.cc",
        "string_piece.cc",
    ];
    for src in &util_sources {
        build.file(format!("{}/{}", util_dir, src));
    }

    // util/double-conversion/ sources
    let dc_sources = [
        "bignum.cc",
        "bignum-dtoa.cc",
        "cached-powers.cc",
        "double-to-string.cc",
        "fast-dtoa.cc",
        "fixed-dtoa.cc",
        "string-to-double.cc",
        "strtod.cc",
    ];
    for src in &dc_sources {
        build.file(format!("{}/{}", dc_dir, src));
    }

    // Include paths: ./vendor/kenlm, ./vendor/kenlm/util
    build.include(kenlm_dir);
    build.include(util_dir);
    // KenLM sources #include "util/..." and "lm/..." so root is vendor/kenlm's parent
    build.include("vendor");

    build.cpp(true).std("c++17").flag("-std=c++17");
    build.opt_level(2);

    // Define KENLM_MAX_ORDER (default: 6) for array sizing
    build.flag("-DKENLM_MAX_ORDER=6");

    // Suppress warnings from vendored code
    build.flag("-Wno-unused-parameter");
    build.flag("-Wno-unused-variable");
    build.flag("-Wno-sign-compare");
    build.flag("-Wno-unused-function");
    build.flag("-Wno-deprecated-declarations");
    if !build.get_compiler().is_like_msvc() {
        build.flag("-Wno-unused-but-set-variable");
    }

    build.compile("mf_kenlm");
}
