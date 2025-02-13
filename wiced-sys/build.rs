use std::env;
use std::path::PathBuf;
use std::path::Path;

fn main() {
    // Tell cargo to look for shared libraries in the specified directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let wiced_src_path = Path::new(&manifest_dir).join("src");
    println!("cargo:rustc-link-search={}", wiced_src_path.display());


    println!("cargo:rustc-link-lib=static=libwiced_sdk_bin");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for
        .header(wiced_src_path.join("wiced_sdk.h").to_str().unwrap())
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}