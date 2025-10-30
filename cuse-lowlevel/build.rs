// SPDX-License-Identifier: MIT
//
// Author: Richard Wiedenhöft <richard@wiedenhoeft.xyz>
// Author: Johannes Leupolz <dev@leupolz.eu>
//
// This library is heavily baased on https://github.com/richard-w/libfuse-sys
// but adopted to only provide the low-level modules of fuse and cuse. 

extern crate bindgen;
extern crate pkg_config;

use std::env;
use std::iter;
use std::path::PathBuf;

const FUSE_USE_VERSION: u32 = 314; //fuse version of ubuntu 24.04


fn fuse_binding_filter(builder: bindgen::Builder) -> bindgen::Builder {
    let builder = builder
        // Whitelist "fuse_*" symbols and blocklist everything else
        .allowlist_recursively(false)
        .allowlist_type("(?i)^fuse.*")
        .allowlist_function("(?i)^fuse.*")
        .allowlist_var("(?i)^fuse.*")
        .blocklist_type("fuse_log_func_t")
        .blocklist_function("fuse_set_log_func");
    builder
}

fn cuse_binding_filter(builder: bindgen::Builder) -> bindgen::Builder {
    builder
        // Whitelist "cuse_*" symbols and blocklist everything else
        .allowlist_recursively(false)
        .allowlist_type("(?i)^cuse.*")
        .allowlist_function("(?i)^cuse.*")
        .allowlist_var("(?i)^cuse.*")
}

fn generate_fuse_bindings(
    header: &str,
    fuse_lib: &pkg_config::Library,
    binding_filter: fn(bindgen::Builder) -> bindgen::Builder,
) {
    // Find header file
    let mut header_path: Option<PathBuf> = None;
    for include_path in fuse_lib.include_paths.iter() {
        let test_path = include_path.join(header);
        if test_path.exists() {
            header_path = Some(test_path);
            break;
        }
    }
    let header_path = header_path
        .unwrap_or_else(|| panic!("Cannot find {}", header))
        .to_str()
        .unwrap_or_else(|| panic!("Path to {} contains invalid unicode characters", header))
        .to_string();

    // Gather fuse defines
    let defines = fuse_lib.defines.iter().map(|(key, val)| match val {
        Some(val) => format!("-D{}={}", key, val),
        None => format!("-D{}", key),
    });
    // Gather include paths
    let includes = fuse_lib
        .include_paths
        .iter()
        .map(|dir| format!("-I{}", dir.display()));
    // API version definition
    let api_define = iter::once(format!("-DFUSE_USE_VERSION={}", FUSE_USE_VERSION));
    // Chain compile flags
    let compile_flags = defines.chain(includes).chain(api_define);

    // Create bindgen builder
    let mut builder = bindgen::builder()
        // Add clang flags
        .clang_args(compile_flags)
        // Derive Debug, Copy and Default
        .derive_default(true)
        .derive_copy(true)
        .derive_debug(true)
        // Add CargoCallbacks so build.rs is rerun on header changes
        .parse_callbacks(Box::new(bindgen::CargoCallbacks));

    builder = binding_filter(builder);

    // Generate bindings
    let bindings = builder
        .header(header_path)
        .generate()
        .unwrap_or_else(|_| panic!("Failed to generate {} bindings", header));

    // Write bindings to file
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let bindings_path = out_dir.join(&header.replace(".h", ".rs"));
    bindings
        .write_to_file(&bindings_path)
        .unwrap_or_else(|_| panic!("Failed to write {}", bindings_path.display()));
}

fn main() {
    let mut pkgcfg = pkg_config::Config::new();

    // Find libfuse
    let fuse3_lib = pkgcfg.cargo_metadata(true).probe("fuse3").expect("Failed to find pkg-config module fuse3");
 
    // Generate lowlevel bindings
    generate_fuse_bindings(
        "fuse_lowlevel.h",
        &fuse3_lib,
        fuse_binding_filter,
    );
    // Generate lowlevel cuse bindings
    generate_fuse_bindings(
        "cuse_lowlevel.h",
        &fuse3_lib,
        cuse_binding_filter,
    );
}