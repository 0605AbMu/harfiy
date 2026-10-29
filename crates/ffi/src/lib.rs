//! Mobile FFI bindings for Harfiy Engine (iOS C-ABI & Android JNI).
//!
//! Each target compiles only the binding it ships: Android loads the JNI symbols, while the
//! C-ABI serves iOS and host-side consumers (examples, tests).

#[cfg(not(target_os = "android"))]
pub mod c_api;
pub mod edits;
#[cfg(target_os = "android")]
pub mod jni_api;

#[cfg(not(target_os = "android"))]
pub use c_api::*;
#[cfg(target_os = "android")]
pub use jni_api::*;
