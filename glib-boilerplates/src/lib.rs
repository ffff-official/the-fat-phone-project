//! Procedural macros for generating conversion boilerplate between safe Rust
//! types and GLib FFI types generated from GIR.
//!
//! `glib-boilerplates` provides the [`Boilerplates`] derive macro,
//! which automatically generates common `From` implementations between low-level
//! GLib FFI structures and their safe Rust counterparts.
//!
//! The source FFI type is inferred from the Rust structure name using the
//! configured prefix and module path.
//!
//! # Example
//!
//! The following declaration:
//!
//! ```ignore
//! use glib_boilerplates::Boilerplates;
//!
//! #[derive(Debug, Clone, PartialEq, Eq, Hash, Boilerplates)]
//! #[boilerplates(prefix = "Qmi", source = "ffi")]
//! pub struct MessageUimGetCardStatusOutputCardStatusCardsElement {
//!     pub card_state: UimCardState,
//!     pub upin_state: UimPinState,
//!     pub upin_retries: u8,
//!     pub upuk_retries: u8,
//!     pub error_code: UimCardError,
//!     pub applications:
//!         Vec<MessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2>,
//! }
//! ```
//!
//! generates conversion code equivalent to: (The reverse From implementation is generated as well.)
//!
//! ```ignore
//! #[derive(Debug, Clone, PartialEq, Eq, Hash)]
//! pub struct MessageUimGetCardStatusOutputCardStatusCardsElement {
//!     pub card_state: UimCardState,
//!     pub upin_state: UimPinState,
//!     pub upin_retries: u8,
//!     pub upuk_retries: u8,
//!     pub error_code: UimCardError,
//!     pub applications:
//!         Vec<MessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2>,
//! }
//!
//! impl From<ffi::QmiMessageUimGetCardStatusOutputCardStatusCardsElement>
//!     for MessageUimGetCardStatusOutputCardStatusCardsElement
//! {
//!     fn from(
//!         value: ffi::QmiMessageUimGetCardStatusOutputCardStatusCardsElement,
//!     ) -> Self {
//!         unsafe {
//!             Self {
//!                 card_state: from_glib(value.card_state),
//!                 upin_state: from_glib(value.upin_state),
//!                 upin_retries: value.upin_retries,
//!                 upuk_retries: value.upuk_retries,
//!                 error_code: from_glib(value.error_code),
//!                 applications: {
//!                     let garray = value.applications;
//!
//!                     if garray.is_null()
//!                         || (*garray).data.is_null()
//!                         || !((*garray).data as *const ffi::QmiMessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2)
//!                             .is_aligned()
//!                     {
//!                         Vec::new()
//!                     } else {
//!                         std::slice::from_raw_parts(
//!                             (*garray).data
//!                                 as *const ffi::QmiMessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2,
//!                             (*garray).len as usize,
//!                         )
//!                         .iter()
//!                         .copied()
//!                         .map(Into::into)
//!                         .collect()
//!                     }
//!                 },
//!             }
//!         }
//!     }
//! }
//! ```
//!
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericArgument, LitStr, Path, Type, parse_macro_input};

#[proc_macro_derive(Boilerplates, attributes(boilerplates))]
pub fn derive_boilerplates(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_boilerplates(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_boilerplates(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let destination_name = input.ident;

    let mut prefix = String::from("Qmi");
    let mut source: Path = syn::parse_str("crate::ffi")?;

    // #[boilerplates(
    //     prefix = "Qmi",
    //     source = "ffi"
    // )]

    for attribute in &input.attrs {
        if !attribute.path().is_ident("boilerplates") {
            continue;
        }

        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                let value: LitStr = meta.value()?.parse()?;
                prefix = value.value();
                return Ok(());
            }

            if meta.path.is_ident("source") {
                let value: LitStr = meta.value()?.parse()?;
                source = value.parse()?;
                return Ok(());
            }

            Err(meta.error("Unknown option"))
        })?;
    }

    // MessageUimFoo -> ffi::QmiMessageUimFoo
    let source_name = format_ident!(
        "{}{}",
        prefix,
        destination_name,
        span = destination_name.span(),
    );

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    destination_name,
                    "Each fields must be named",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                destination_name,
                "Boilerplates works only on structs",
            ));
        }
    };

    let assignments_from_ffi = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().expect("Why are not identifiants ?");

        let expression = conversion_expression_from_ffi(&field.ty, field_name, &prefix, &source);

        quote! {
            #field_name: #expression
        }
    });

    let assignments_from_rust = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().expect("Why are not identifiants ?");

        let expression = conversion_expression_from_rust(&field.ty, field_name, &prefix, &source);

        quote! {
            #field_name: #expression
        }
    });

    Ok(quote! {
        impl From<#source::#source_name> for #destination_name {
            fn from(value: #source::#source_name) -> Self {
                unsafe {
                    Self {
                        #(#assignments_from_ffi,)*
                    }
                }
            }
        }

        impl From<#destination_name> for #source::#source_name {
            fn from(value: #destination_name) -> Self {
                use glib::translate::IntoGlib;

                Self {
                    #(#assignments_from_rust,)*
                }
            }
        }
    })
}

fn conversion_expression_from_ffi(
    destination_type: &Type,
    field_name: &syn::Ident,
    prefix: &str,
    source: &Path,
) -> proc_macro2::TokenStream {
    if is_bool(destination_type) {
        return quote! {
            value.#field_name != 0
        };
    }

    if is_direct_copy_type(destination_type) {
        return quote! {
            value.#field_name
        };
    }

    if let Some(inner_type) = vec_inner_type(destination_type) {
        let Type::Path(type_path) = inner_type else {
            panic!("sa mere");
        };

        let ident = &type_path.path.segments.last().unwrap().ident;

        let ffi_element_type = if is_direct_copy_type(inner_type) {
            quote! { #ident }
        } else {
            let ffi_ident = format_ident!("{}{}", prefix, ident);
            quote! { #source::#ffi_ident }
        };
        return quote! {
            {
                let garray = value.#field_name;
                if garray.is_null() || (*garray).data.is_null() || !((*garray).data as *const #ffi_element_type).is_aligned() {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(
                        (*garray).data as *const #ffi_element_type,
                        (*garray).len as usize,
                    )
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                }
            }
        };
    }

    quote! {
        ::glib::translate::from_glib(value.#field_name)
    }
}

fn conversion_expression_from_rust(
    destination_type: &Type,
    field_name: &syn::Ident,
    prefix: &str,
    source: &Path,
) -> proc_macro2::TokenStream {
    if is_direct_copy_type(destination_type) {
        return quote! {
            value.#field_name
        };
    }

    if let Some(inner_type) = vec_inner_type(destination_type) {
        let Type::Path(type_path) = inner_type else {
            panic!("sa mere");
        };
        let ident = &type_path.path.segments.last().unwrap().ident;

        let ffi_element_type = if is_direct_copy_type(inner_type) {
            quote! { #ident }
        } else {
            let ffi_ident = format_ident!("{}{}", prefix, ident);
            quote! { #source::#ffi_ident }
        };

        return quote! {
            unsafe {
                let values = value.#field_name;
                let len = values.len();

                let array = glib_sys::g_array_sized_new(
                    0,
                    0,
                    std::mem::size_of::<#ffi_element_type>() as u32,
                    len as u32,
                );

                glib_sys::g_array_set_size(array, len as u32);

                if len != 0 {
                    let destination = (*array).data as *mut #ffi_element_type;

                    for (index, value) in values.into_iter().enumerate() {
                        destination.add(index).write(value.into());
                    }
                }
                array
            }
        };
    }

    quote! {
        value.#field_name.into_glib()
    }
}

fn is_bool(ty: &Type) -> bool {
    type_name(ty).is_some_and(|name| name == "bool")
}

fn vec_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    let segment = type_path.path.segments.last()?;

    if segment.ident != "Vec" {
        return None;
    }

    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };

    match args.args.first()? {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

fn is_direct_copy_type(ty: &Type) -> bool {
    type_name(ty).is_some_and(|name| {
        matches!(
            name.as_str(),
            "u8" | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "f32"
                | "f64"
                | "char"
        )
    })
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    if type_path.qself.is_some() {
        return None;
    }

    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}
