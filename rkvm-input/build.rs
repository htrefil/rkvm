use bindgen::{Builder, CargoCallbacks};
use cc::Build;
use pkg_config::Config;
use std::env;
use std::path::PathBuf;

const RKVM_HAVE_ABS_PROFILE: &[u8] = b"RKVM_HAVE_ABS_PROFILE";

fn main() {
    match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
        "windows" => return,
        "linux" => {}
        _ => panic!("Unsupported target OS"),
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
        .parse_callbacks(Box::new(CargoCallbacks))
        .generate()
        .unwrap();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("glue.rs")).unwrap();

    // Check for RKVM_ABS_PROFILE, which was added in v6.1-rc1.
    let expanded = Build::new()
        .file("glue/check.h")
        .includes(library.include_paths)
        .expand();

    if expanded
        .windows(RKVM_HAVE_ABS_PROFILE.len())
        .any(|window| window == RKVM_HAVE_ABS_PROFILE)
    {
        println!("cargo:rustc-cfg=feature=\"abs_profile\"");
    }
}
