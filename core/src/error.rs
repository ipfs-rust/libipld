//! `Ipld` error definitions.
use crate::cid::Cid;
use crate::ipld::{Ipld, IpldIndex};
pub use anyhow::{Error, Result};
use thiserror::Error;

/// Block exceeds 1MiB.
#[derive(Clone, Copy, Debug, Error)]
#[error("Block size {0} exceeds 1MiB.")]
pub struct BlockTooLarge(pub usize);

/// The codec is unsupported.
#[derive(Clone, Copy, Debug, Error)]
#[error("Unsupported codec {0:?}.")]
pub struct UnsupportedCodec(pub u64);

/// The multihash is unsupported.
#[derive(Clone, Copy, Debug, Error)]
#[error("Unsupported multihash {0:?}.")]
pub struct UnsupportedMultihash(pub u64);

/// Hash does not match the CID.
#[derive(Clone, Debug, Error)]
#[error("Hash of data does not match the CID.")]
pub struct InvalidMultihash(pub Vec<u8>);

/// The block wasn't found. The supplied string is a CID.
#[derive(Clone, Copy, Debug, Error)]
#[error("Failed to retrieve block {0}.")]
pub struct BlockNotFound(pub Cid);

/// Type error.
#[derive(Clone, Debug, Error)]
#[error("Expected {expected:?} but found {found:?}")]
pub struct TypeError {
    /// The expected type.
    pub expected: TypeErrorType,
    /// The actual type.
    pub found: TypeErrorType,
}

impl TypeError {
    /// Creates a new type error.
    pub fn new<A: Into<TypeErrorType>, B: Into<TypeErrorType>>(expected: A, found: B) -> Self {
        Self {
            expected: expected.into(),
            found: found.into(),
        }
    }
}

/// Type error type.
#[derive(Clone, Debug)]
pub enum TypeErrorType {
    /// Null type.
    Null,
    /// Boolean type.
    Bool,
    /// Integer type.
    Integer,
    /// Float type.
    Float,
    /// String type.
    String,
    /// Bytes type.
    Bytes,
    /// List type.
    List,
    /// StringMap type.
    StringMap,
    /// IntegerMap type.
    IntegerMap,
    /// Link type.
    Link,
    /// Tag type.
    Tag,
    /// Key type.
    Key(String),
    /// Index type.
    Index(usize),
}

impl From<Ipld> for TypeErrorType {
    fn from(ipld: Ipld) -> Self {
        Self::from(&ipld)
    }
}

impl From<&Ipld> for TypeErrorType {
    fn from(ipld: &Ipld) -> Self {
        match ipld {
            Ipld::Null => Self::Null,
            Ipld::Bool(_) => Self::Bool,
            Ipld::Integer(_) => Self::Integer,
            Ipld::Float(_) => Self::Float,
            Ipld::String(_) => Self::String,
            Ipld::Bytes(_) => Self::Bytes,
            Ipld::List(_) => Self::List,
            Ipld::StringMap(_) => Self::StringMap,
            #[cfg(feature = "unleashed")]
            Ipld::IntegerMap(_) => Self::IntegerMap,
            Ipld::Link(_) => Self::Link,
            #[cfg(feature = "unleashed")]
            Ipld::Tag(_, _) => Self::Tag,
        }
    }
}

impl From<IpldIndex<'_>> for TypeErrorType {
    fn from(index: IpldIndex<'_>) -> Self {
        match index {
            IpldIndex::List(i) => Self::Index(i),
            IpldIndex::Map(s) => Self::Key(s),
            IpldIndex::MapRef(s) => Self::Key(s.into()),
        }
    }
}
