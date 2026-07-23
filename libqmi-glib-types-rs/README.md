[![crates.io](https://img.shields.io/crates/v/libqmi-glib-types-rs.svg)](https://crates.io/crates/libqmi-glib-types-rs)
[![Docs.rs](https://docs.rs/libqmi-glib-types-rs/badge.svg)](https://docs.rs/libqmi-glib-types-rs)
[![License](https://img.shields.io/crates/l/libqmi-glib-types-rs.svg)](https://crates.io/crates/libqmi-glib-types-rs)
# libqmi-glib-types-rs

Rust definitions of the **enums** and **flags** from **libqmi-glib**.

This crate provides safe and idiomatic Rust representations of the enumeration and bitflag types exposed by the companion **`libqmi-glib-sys`** crate.

It is intended to be used as a building block for higher-level bindings, avoiding direct use of the raw FFI types.

## Features

* Safe Rust enums generated from `libqmi-glib`
* Rust bitflags generated from `libqmi-glib`
* Seamless conversion to and from the corresponding FFI types
* Designed to be used alongside `libqmi-glib-sys`

## Relationship with `libqmi-glib-sys`

* **`libqmi-glib-sys`**: raw FFI bindings to the C library.
* **`libqmi-glib-types-rs`**: safe Rust enums and bitflags corresponding to the FFI types.

This crate does not expose the object-oriented API of libqmi-glib; it only provides the generated Rust types.
