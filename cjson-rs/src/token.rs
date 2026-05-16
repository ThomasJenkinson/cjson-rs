//! JSON tokens per RFC 8259.

use crate::error::Position;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `{` — RFC 8259 §4 begin-object
    BeginObject,
    /// `}` — RFC 8259 §4 end-object
    EndObject,
    /// `[` — RFC 8259 §5 begin-array
    BeginArray,
    /// `]` — RFC 8259 §5 end-array
    EndArray,
    /// `:` — RFC 8259 §4 name-separator
    NameSeparator,
    /// `,` — RFC 8259 §4/§5 value-separator
    ValueSeparator,
    /// `null` — RFC 8259 §3
    Null,
    /// `true` — RFC 8259 §3
    True,
    /// `false` — RFC 8259 §3
    False,
    /// JSON number per RFC 8259 §6
    Number(f64),
    /// JSON string per RFC 8259 §7, with escape sequences already resolved
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PositionedToken {
    pub token: Token,
    pub pos: Position,
}
