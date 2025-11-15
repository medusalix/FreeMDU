# FreeMDU Protocol ![crates.io version](https://img.shields.io/crates/v/freemdu?logo=rust) ![docs.rs](https://img.shields.io/docsrs/freemdu?logo=rust) ![crates.io MSRV](https://img.shields.io/crates/msrv/freemdu?logo=rust)

The FreeMDU protocol crate implements the proprietary Miele diagnostic protocol. It offers an asynchronous, platform-agnostic API for communicating with Miele appliances via the diagnostic interface.

More details about the interface and the FreeMDU project can be found [here](https://github.com/medusalix/FreeMDU).

## Compatibility

This crate can be used in `no_std` environments and embedded projects, but an allocator is required due to the use of `Box`.

## Optional features

When adding this crate as a dependency, the following optional features can be specified (all disabled by default):

- **`native-serial`**: enables a serial port implementation based on the [`serial2-tokio`](https://crates.io/crates/serial2-tokio) crate (requires `std`)

## Examples

Several examples are provided to demonstrate the crate's functionality:

- **`find_keys`**: finds the diagnostic keys of a device using a brute-force search
- **`dump_memory`**: reads RAM and ROM data from a supported device and writes them to a file
- **`dump_eeprom`**: reads the EEPROM contents from a supported device and writes them to a file

An example can be executed with the following command, replacing `<EXAMPLE>` with the desired example name:

```shell
cargo run --all-features --example <EXAMPLE>
```
