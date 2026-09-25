# NetX rust integration

This takes the build process and most of the structures from https://github.com/sabaton-systems/threadx-rust/ and builds a ThreadX + NetX variant.
Compared to the original we did:

- Fix some UB
- Generate NetX bindings
- Implement simple async executor based on https://github.com/zesterer/pollster
- Implement embedded-nal interface for NetX/Wiced Wifi
- Async interface for buttons

## Quickstart

### Executor example

The `executor.rs`example switches between 2 screens via tha A button and shows either a welcome screen or the actual temperature. It is a demonstration of the async executor based on ThreadX EventFlags.

Goto `threadx-app/cross/app` and run:

`cargo run --release --target thumbv7em-none-eabihf --bin executor`

### Network example

This examples connects to a WiFi Network and to an MQTT5 broker and regularly publishes the current temperature as uMessage (see: Link to uProtocol). 

The WiFi credentials are read at build time from the `WIFI_SSID` and `WIFI_PASSWORD` environment variables, so they never end up in the source. Adapt the MQTT settings in the `network.rs` example accordingly.

Goto `threadx-app/cross/app` and run:

`WIFI_SSID=<ssid> WIFI_PASSWORD=<password> cargo run --release --target thumbv7em-none-eabihf --bin network`

Because `cargo build` builds all binaries, the variables also need to be set when building the whole workspace.

## Shortcomings

- Only supports the MXAZ3166 board!
- No production ready error handling 
- General structure needs to be improved 
- Some more abstractions see embassy

### embedded-nal

- Only a single socket can be used
- As for now the implementation is unsound as there are no checks to assure that buffers are big enough to hold incoming packets 

### Async executor

- 32 parallel async tasks are supported
- Simple executor which blocks the thread it runs on 

## Control structures

Control structures should be checked if they are moveable ie. can be copied via a simple memcopy. Often this is not explicitely documented within the
ThreadX documentation hence we should assume that they cannot be moved. There are at least 2 obvious solutions:

- Make the control structures static and limit to a fixed number of for example mutexes
- Use the "std library" approach ie. pin box the control structure

## Further ideas

### Static tasks / threads

Veecle and embassy use statically allocated tasks via the type-impl-in-trait nightly feature. Maybe we should do the same to avoid dynamic allocation and the global allocator. 
