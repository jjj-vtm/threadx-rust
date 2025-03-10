## NetX rust integration

This takes the build process and most of the structures from https://github.com/sabaton-systems/threadx-rust/ and builds a ThreadX + NetX variant.
Compared to the original we did:

- Fix some UB
- Generate NetX bindings
- Implement simple async executor based on https://github.com/zesterer/pollster
- Implement embedded-nal interface for NetX/Wiced Wifi

## Quickstart

In the `network.rs` example adapt the SSID, WLAN-Passwort and the MQTT settings accordingly.  

Goto `threadx-app/cross/app` and run:

`cargo run --release --target thumbv7em-none-eabihf --bin network`

# Things to be adressed

Only supports the MXAZ3166 board!

## Shortcomings

- Error handling must be implemented

### embedded-nal

- Only a single socket can be used

### Async executor

- 32 parallel async tasks are supported
- Simple executor which blocks the thread it runs on 

## Control structures

Control structures should be checked if they are moveable ie. can be copied via a simple memcopy. Often this is not explicitely documented within the
ThreadX documentation hence we should assume that they cannot be moved. There are at least 2 obvious solutions:

- Make the control structures static and limit to a fixed number of for example mutexes
- Use the "std library" approach ie. pin box the control structure

# Further ideas

## Static tasks / threads

Veecle and embassy use statically allocated tasks via the type-impl-in-trait nightly feature. Maybe we should do the same to avoid dynamic allocation and the global allocator. 