//! Functional programming utilities

use crate::consts::{ALPHABETS, ID_LENGTH};

/// The Applicative trait provides a pure behavior,
/// which can be used to create values of type f a from values of type a.
pub trait Applicative<R> {
    /// The Associative type acts as a (f a) wrapper for Self.
    type WrappedSelf<T>;

    /// Applicative::pure(_) is abstraction with lifts any arbitrary type to underlying higher
    /// order type
    fn pure(v: R) -> Self::WrappedSelf<R>;
}

impl<R> Applicative<R> for Option<R> {
    type WrappedSelf<T> = Option<T>;
    fn pure(v: R) -> Self::WrappedSelf<R> {
        Some(v)
    }
}

impl<R, E> Applicative<R> for Result<R, E> {
    type WrappedSelf<T> = Result<T, E>;
    fn pure(v: R) -> Self::WrappedSelf<R> {
        Ok(v)
    }
}

/// based on the condition provided into the `predicate`
pub fn when<W: Applicative<(), WrappedSelf<()> = W>, F>(predicate: bool, f: F) -> W
where
    F: FnOnce() -> W,
{
    if predicate {
        f()
    } else {
        W::pure(())
    }
}

#[inline]
#[cfg_attr(feature = "deja", track_caller)]
#[cfg_attr(
    feature = "deja",
    deja::id(
        component = "common_utils",
        operation = "generate_id_with_default_len",
        codec = SerdeCodec,
    )
)]
pub fn generate_id_with_default_len(prefix: &str) -> String {
    let len: usize = ID_LENGTH;
    format!("{}_{}", prefix, nanoid::nanoid!(len, &ALPHABETS))
}

#[inline]
#[cfg_attr(feature = "deja", track_caller)]
#[cfg_attr(
    feature = "deja",
    deja::id(component = "common_utils", operation = "generate_id", codec = SerdeCodec,)
)]
pub fn generate_id(length: usize, prefix: &str) -> String {
    format!("{}_{}", prefix, nanoid::nanoid!(length, &ALPHABETS))
}

#[inline]
#[cfg_attr(feature = "deja", track_caller)]
#[cfg_attr(
    feature = "deja",
    deja::id(component = "common_utils", operation = "generate_uuid_v7", codec = SerdeCodec,)
)]
pub fn generate_uuid_v7() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Generate a random (v4) UUID string. Connector transformers that need a random
/// idempotency key / client-request-id should call this seamed helper rather than
/// `uuid::Uuid::new_v4()` directly, so the value is captured and replayed deterministically.
#[inline]
#[cfg_attr(feature = "deja", track_caller)]
#[cfg_attr(
    feature = "deja",
    deja::id(component = "common_utils", operation = "generate_uuid_v4", codec = SerdeCodec,)
)]
pub fn generate_uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
}
