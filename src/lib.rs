//! Implements `SmallString`, a `String`-like container for small strings
//!
//! ## `no_std` support
//!
//! By default, `smallstr` does not depend on `std`. The `std` feature may be enabled
//! to add the `std` dependency. The `ffi` feature also implies `std`.
//!
//! ## `ffi` feature
//!
//! The `ffi` feature will add the following trait implementations to `SmallString`:
//!
//! * `PartialEq<OsStr>`
//! * `PartialEq<&'_ OsStr>`
//! * `PartialEq<OsString>`
//! * `PartialEq<Cow<'_, OsString>>`
//!
//! This feature also adds `std` as a dependency.
//!
//! ## `serde` support
//!
//! When the `serde` feature is enabled, the traits `serde::Deserialize` and
//! `serde::Serialize` are implemented for `SmallString`.
//!
//! This feature is disabled by default.
//!
//! By default, the `serde` dependency is compiled with `no_std`.
//! If the `std` feature is enabled, `std` is added as a dependency in `serde`, as well.
//!
//! ## `union` feature
//!
//! This feature will enable the `union` feature in `smallvec`, which reduces the size of
//! a `SmallString` instance. This feature requires a nightly compiler.

#![cfg_attr(not(any(feature = "ffi", feature = "std")), no_std)]
#![deny(missing_docs)]

extern crate alloc;

pub use string::*;

mod string;
