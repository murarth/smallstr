//! Implements `SmallString`, a `String`-like container for small strings
//!
//! # `no_std` support
//!
//! Like `smallvec`, `smallstr` has a default-enabled feature named `std` which
//! controls the dependency on Rust `libstd`.
//!
//! To depend on `smallstr` without `libstd`, add `default-features = false`
//! to the `smallstr` dependency.
//!
//! # `serde` support
//!
//! When the `serde` feature is enabled, the traits `serde::Deserialize` and
//! `serde::Serialize` are implemented for `SmallString`.
//!
//! This feature is disabled by default.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc))]

#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "serde")]
extern crate serde;

#[cfg(test)]
#[cfg(feature = "serde")]
extern crate bincode;

extern crate smallvec;

#[cfg(not(feature = "std"))]
mod std {
    pub use core::*;
}

pub use string::*;

mod string;
