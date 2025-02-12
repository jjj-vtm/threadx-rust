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

# Things to be adressed

## Build

Build should be self contained!

## Shortcomings

- Error handling must be implemented

### embedded-nal

- Only a single socket can be used

### Async executor

- block_on can only work with one thread since the mutex can only be instantiated once
- Implementation also does use only a single event flag (0x1). Using more we could support up to 32 threads executing tasks 

## Control structures

Control structures should be checked if they are moveable ie. can be copied via a simple memcopy. Often this is not explicitely documented within the
ThreadX documentation hence we should assume that they cannot be moved. There are at least 2 obvious solutions:

- Make the control structures static and limit to a fixed number of for example mutexes
- Use the "std library" approach ie. pin box the control structure

# Further ideas

## Static tasks / threads

Veecle and embassy use statically allocated tasks via the type-impl-in-trait nightly feature. Maybe we should do the same to avoid dynamic allocation and the global allocator. 