# Serial Async Multithread Library

`serial-thread` is a Rust library designed to facilitate serial port communication in a multithreaded environment. 
It leverages asynchronous/mpsc channels for efficient handling of serial data, suitable for applications that require 
concurrent processing of serial port input/output.

This lib is in early development stage, API not yet clearly defined and breaking changes will happen in upcoming releases.

## Features

- **Asynchronous Communication**: Utilizes std::sync::mpsc::chanels or async-channels (feature flag) for 
non-blocking serial port communication.
- **Multithreaded Support**: Designed to work efficiently in a multithreaded context.
- **Custom Serial Configurations**: Supports various configurations such as baud rate, character size, and flow control.
- **Error Handling**: Comprehensive error handling for robust application development.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
serial-thread = "0.0.1"
```
or 
``` 
cargo add serial_thread
```

## Example

A usage example can be found in /src/example