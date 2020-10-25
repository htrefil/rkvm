use bindgen::{Builder, CargoCallbacks};
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=glue/glue.h");

    let bindings = Builder::default()
        .header("glue/glue.h")
        .parse_callbacks(Box::new(CargoCallbacks))
        .generate()
        .unwrap();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("glue.rs")).unwrap();
}
