# qrtr-sys

Raw FFI bindings for **libqrtr-glib**, the GLib-based API for the Qualcomm Router (QRTR) IPC bus.

This crate provides low-level, unsafe Rust bindings to the native C library. It is intended to be used as the foundation for higher-level, safe Rust abstractions.

## Status

This crate is automatically generated from the upstream GObject Introspection metadata (`Qrtr-1.0.gir`) using the **gtk-rs GIR** code generator.

Manual modifications to the generated source files are discouraged, as they will be overwritten when the bindings are regenerated.

## Requirements

To build this crate, the development files for **libqrtr-glib** must be installed on your system, including the corresponding `pkg-config` metadata.

On Debian-based distributions:

```sh
sudo apt install libqrtr-glib-dev
```

## Safety

This crate exposes the native C API directly and performs no safety checks. Calling its functions is inherently `unsafe`.

Most applications should prefer a higher-level wrapper crate instead of using `qrtr-sys` directly.

## License

Licensed under either:

* Apache License, Version 2.0
* MIT License

at your option.
