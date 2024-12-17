# `ThreadX example Rust application`

The application follows a structure defined at https://ferrous-systems.com/blog/test-embedded-app/. This organizes
the project into separately testable parts. Read the blog to learn how to run the tests.

## Dependencies

#### 1. `flip-link`:

```console
$ cargo install flip-link
```

#### 2. `probe-rs`:

``` console
$ # install libudev
$ sudo apt-get install libudev-dev
$ # make sure to install v0.2.0 or later
$ cargo install probe-rs --features cli
```

#### 3. `Rust target`:

This project is currently set up for the STM32F103.

``` console
$ rustup target add thumbv7m-none-eabi
$ rustup target add thumbv7em-none-eabihf
```

#### 4. Arm GCC

You will need to install the arm gcc tools.  I downloaded the latest release from https://developer.arm.com/downloads/-/arm-gnu-toolchain-downloads . Install manually and add the bin folder
to your PATH.

#### 5. Others

```console
$ sudo apt install ninja-build
$ sudo apt-get install libclang-dev
```

## Running

Go to the threadx-app/cross folder and run

```console
cargo run --bin hello --release
```

The code assumes that you will be using an ST-Link debugger. 

## Debbugging
Hardfault:
Set:
(gdb) set mem inaccessible-by-default off

https://interrupt.memfault.com/blog/cortex-m-hardfault-debug
Entire CFSR - print/x *(uint32_t *) 0xE000ED28
UsageFault Status Register (UFSR) - print/x *(uint16_t *)0xE000ED2A
BusFault Status Register (BFSR) - print/x *(uint8_t *)0xE000ED29
MemManage Status Register (MMFSR) - print/x *(uint8_t *)0xE000ED28


