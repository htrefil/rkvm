use bindgen::Builder;
use cc::Build;
use pkg_config::Config;
use std::env;
use std::path::PathBuf;

const CHECKS: &[(&str, &str)] = &[
    // Added in v6.1-rc1.
    ("RKVM_HAVE_ABS_PROFILE", "have_abs_profile"),
    // Only present in newer kernels.
    (
        "RKVM_HAVE_INPUT_PROP_PRESSUREPAD",
        "have_input_prop_pressurepad",
    ),
];

fn main() {
    match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
        "windows" => return,
        "linux" => {}
        _ => panic!("Unsupported target OS"),
    }

    for (_, cfg) in CHECKS {
        println!("cargo:rustc-check-cfg=cfg({})", cfg);
    }

    println!("cargo:rerun-if-changed=glue/glue.h");
    println!("cargo:rerun-if-changed=glue/check.h");

    let library = Config::new()
        .atleast_version("1.9.0")
        .probe("libevdev")
        .unwrap();

    let args = library
        .include_paths
        .iter()
        .map(|path| format!("-I{}", path.as_os_str().to_str().unwrap()));

    let bindings = Builder::default()
        .header("glue/glue.h")
        .clang_args(args)
        // Importing `CargoCallbacks` would also pull in the deprecated constant of the same name.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .unwrap();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("glue.rs")).unwrap();

    // Check for definitions that are missing in older kernel headers.
    let expanded = Build::new()
        .file("glue/check.h")
        .includes(library.include_paths)
        .expand();

    for (marker, cfg) in CHECKS {
        let marker = marker.as_bytes();

        if expanded
            .windows(marker.len())
            .any(|window| window == marker)
        {
            println!("cargo:rustc-cfg={}", cfg);
        }
    }
}
