System preperation:

- Rust embedded setup (probe-rs, arm target)

In netx-sys build.rs
- Adapt source path to point to netx 

- Adapt path to the default NX_USER_FILE 

- Adapt path to the wiced binary:
    println!("cargo:rustc-link-search=/Users/janjongen/Documents/workspace/threadx-rust/wiced-sys/src/");

Replace all local paths in the thread-sys/build.rs and the wiced-sys/build.rs

For minimq checkout ... and place it locally since it needs to reference embedded NAL 0.9 and the official version only references 0.8

Build and run example app 


Goto threadx-app/cross/app and run: 

cargo run --release --target thumbv7em-none-eabihf --bin network
