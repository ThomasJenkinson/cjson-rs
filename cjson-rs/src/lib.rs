#![forbid(unsafe_code)]

//! cjson-rs — memory-safe JSON parser.
//!
//! Implements RFC 8259. Provides a value model compatible with cJSON's
//! semantics so that the companion `cjson-rs-ffi` crate can expose a
//! drop-in C ABI.

pub mod error;
pub mod parser;
pub mod serialiser;
pub mod token;
pub mod tokeniser;
pub mod value;

pub use error::{Error, Position};
pub use parser::{parse, parse_with_options, ParserOptions};
pub use serialiser::{serialise, serialise_pretty};
pub use token::{PositionedToken, Token};
pub use tokeniser::Tokeniser;
pub use value::Value;
