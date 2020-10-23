use bindgen::{Builder, CargoCallbacks};
use cc::Build;
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=setup/setup.h");
    println!("cargo:rerun-if-changed=setup/setup.c");

    Build::new().file("setup/setup.c").compile("setup");

    let bindings = Builder::default()
        .header("setup/setup.h")
        .parse_callbacks(Box::new(CargoCallbacks))
        .generate()
        .unwrap();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("setup.rs")).unwrap();
}
