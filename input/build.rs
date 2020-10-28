use bindgen::{Builder, CargoCallbacks};
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=glue/glue.h");
    println!("cargo:rustc-link-lib=evdev");

    // TODO: pkg-config
    let bindings = Builder::default()
        .header("glue/glue.h")
        .clang_arg("-I/usr/include/libevdev-1.0/")
        .parse_callbacks(Box::new(CargoCallbacks))
        .generate()
        .unwrap();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("glue.rs")).unwrap();
}
