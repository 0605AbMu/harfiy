//! Mobile FFI bindings for Harfiy Engine (iOS C-ABI & Android JNI).

pub mod c_api;
#[cfg(target_os = "android")]
pub mod jni_api;

pub use c_api::*;
#[cfg(target_os = "android")]
pub use jni_api::*;
