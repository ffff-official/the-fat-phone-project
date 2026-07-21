# libqmi-glib-sys

Raw FFI bindings for **libqmi-glib**, the GLib-based library for communicating with Qualcomm-based modems using the Qualcomm MSM Interface (QMI) protocol.

This crate provides low-level, unsafe Rust bindings to the native C library. It is intended to serve as the foundation for higher-level, safe Rust abstractions.

## Status

This crate is automatically generated from the upstream GObject Introspection metadata (`Qmi-1.0.gir`) using the **gtk-rs GIR** code generator.

Manual modifications to the generated source files are discouraged, as they will be overwritten when the bindings are regenerated.

## Requirements

To build this crate, the development files for **libqmi-glib** must be installed on your system, including the corresponding `pkg-config` metadata.

On Debian-based distributions:

```sh
sudo apt install libqmi-glib-dev
```

## Safety

This crate exposes the native C API directly and performs no safety checks. Calling its functions is inherently `unsafe`.

Most applications should prefer a higher-level wrapper crate instead of using `libqmi-glib-sys` directly.

## License

Licensed under either:

* Apache License, Version 2.0
* MIT License

at your option.
