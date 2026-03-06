# FreeMDU Protocol [![crates.io](https://img.shields.io/crates/v/freemdu?logo=rust)](https://crates.io/crates/freemdu) [![docs.rs](https://img.shields.io/docsrs/freemdu?logo=rust)](https://docs.rs/freemdu) [![MSRV](https://img.shields.io/crates/msrv/freemdu?logo=rust)](https://crates.io/crates/freemdu)

The FreeMDU protocol crate implements the proprietary Miele diagnostic protocol. It offers an asynchronous, platform-agnostic API for communicating with Miele appliances via the diagnostic interface.

More details about the interface and the FreeMDU project can be found [here](https://github.com/medusalix/FreeMDU).

## Compatibility

This crate can be used in `no_std` environments and embedded projects, but an allocator is required due to the use of `Box`.

## Optional features

When adding this crate as a dependency, the following optional features can be specified (all disabled by default):

- **`native-serial`**: enables a serial port implementation based on the [`serial2-tokio`](https://crates.io/crates/serial2-tokio) crate (requires `std`)

## Binaries

Several binaries are provided to work with your devices:

- **`find_keys`**: finds the diagnostic keys of a device using a brute-force search
- **`dump_memory`**: reads RAM and ROM data from a supported device and writes them to a file
- **`dump_eeprom`**: reads the EEPROM contents from a supported device and writes them to a file

A binary can be executed with the following command, replacing `<BIN>` with the desired binary name:

```shell
cargo run --all-features --bin <BIN>
```

All binaries accept the flag `-p` to define the serial port of your ESP32, the default is:

```shell
cargo run --all-features --bin <BIN> -- -p /dev/ttyACM0
```

For `dump_eeprom`, you can also use these flags (they are the default values):
- specify start and/or end addresses: `-s 0x0000 -e 0x07ff`
- specify output filename: `-o eeprom_dump.bin`
- use byte instead of word addressing for newer devices: `-b`

For `dump_memory`:
- start/end 32-bit addresses: `-s 0x0000_0000 -e 0x0000_ffff`
- output file: `-o memory_dump.bin`