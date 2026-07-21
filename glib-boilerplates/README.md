# glib-boilerplates

**Procedural macros for generating conversion boilerplate between safe Rust types and GLib FFI types generated from GIR.**

`glib-boilerplates` provides the `Boilerplates` derive macro, which automatically generates common `From` implementations between low-level GLib FFI structures and their safe Rust counterparts.

The source FFI type is inferred from the Rust type name using the configured prefix and module path, allowing bindings generated from GIR to remain concise while avoiding repetitive conversion code.

**Alpha status**

This crate was created to cover my immediate needs and is currently in a very early stage of development. It has only been tested against my own use cases and should be considered experimental.

If you plan to use it in your own project, I strongly recommend reviewing the generated code and adapting it to your requirements. Contributions and feedback are welcome.

## Example

Writing:

```rust
use glib_boilerplates::Boilerplates;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Boilerplates)]
#[boilerplates(prefix = "Qmi", source = "ffi")]
pub struct MessageUimGetCardStatusOutputCardStatusCardsElement {
    pub card_state: UimCardState,
    pub upin_state: UimPinState,
    pub upin_retries: u8,
    pub upuk_retries: u8,
    pub error_code: UimCardError,
    pub applications: Vec<MessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2>,
}
```

automatically generates the corresponding `From` implementations between:

```rust
MessageUimGetCardStatusOutputCardStatusCardsElement
```

and:

```rust
ffi::QmiMessageUimGetCardStatusOutputCardStatusCardsElement
```

in both directions.

## Features

The derive macro currently supports automatic conversion of:

* Primitive integer types
* `bool`
* GLib enumeration types through `from_glib`
* Nested structures implementing `From`
* `Vec<T>` backed by `GArray`
* Bidirectional `From` implementations between safe and FFI types

The generated code performs the necessary conversions while keeping the user-facing types entirely safe and idiomatic.

## Motivation

Projects based on GIR-generated bindings typically require hundreds of repetitive conversion implementations between raw GLib structures and safe Rust types.

`glib-boilerplates` eliminates most of this boilerplate by generating the conversion code automatically, allowing the bindings to remain concise, maintainable, and focused on their public API.
