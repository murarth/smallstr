//! Implements `SmallString`, a `String`-like container for small strings
//!
//! ## `no_std` support
//!
//! By default, `smallstr` does not depend on `std`. However, the `ffi` feature will add
//! `std` as a dependency.
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
//! ## `union` feature
//!
//! This feature will enable the `union` feature in `smallvec`, which reduces the size of
//! a `SmallString` instance. This feature requires a nightly compiler.

#![no_std]
#![deny(missing_docs)]

extern crate alloc;

#[cfg(any(feature = "error", feature = "ffi",))]
extern crate std;

pub use string::*;

mod string;
