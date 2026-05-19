//! RFC 8259 tokeniser.
//!
//! Consumes a UTF-8 byte slice and yields a stream of `PositionedToken`s.
//! Each grammar production from RFC 8259 §3-§7 maps to a `Token` variant.
//! Escape sequences and `\uXXXX` surrogate pairs are resolved here, so
//! `Token::String` already holds the decoded value.

use crate::error::{Error, Position};
use crate::token::{PositionedToken, Token};

pub struct Tokeniser<'a> {
    input: &'a [u8],
    pos: Position,
}

impl<'a> Tokeniser<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            pos: Position::START,
        }
    }

    /// Tokenise the entire input.
    pub fn tokenise(input: &'a [u8]) -> Result<Vec<PositionedToken>, Error> {
        let mut t = Self::new(input);
        let mut out = Vec::new();
        loop {
            t.skip_ws();
            if t.is_at_end() {
                break;
            }
            out.push(t.next_token()?);
        }
        Ok(out)
    }
    /// Tokenise as many tokens as possible from `input`, stopping silently
    /// on the first tokenise error or at end-of-input. Returns the tokens
    /// successfully read plus the byte offset immediately after the last
    /// consumed token (i.e. *not* including any trailing whitespace or
    /// unrecognised bytes). Used by `parse_prefix` to support cJSON's
    /// `require_null_terminated = false` mode.
    pub fn tokenise_prefix(input: &'a [u8]) -> (Vec<PositionedToken>, usize) {
        let mut t = Self::new(input);
        let mut out = Vec::new();
        let mut last_token_end: usize = 0;
        loop {
            t.skip_ws();
            if t.is_at_end() {
                break;
            }
            match t.next_token() {
                Ok(tk) => {
                    last_token_end = t.pos.offset;
                    out.push(tk);
                }
                Err(_) => break,
            }
        }
        (out, last_token_end)
    }
    
    fn is_at_end(&self) -> bool {
        self.pos.offset >= self.input.len()
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos.offset).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.input.get(self.pos.offset + n).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.peek()?;
        match b {
            b'\n' => {
                self.pos.line += 1;
                self.pos.column = 1;
            }
            _ => self.pos.column += 1,
        }
        self.pos.offset += 1;
        Some(b)
    }

    /// RFC 8259 §2: ws = *( %x20 / %x09 / %x0A / %x0D )
    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    fn next_token(&mut self) -> Result<PositionedToken, Error> {
        let start = self.pos;
        let b = self.peek().ok_or(Error::UnexpectedEof { pos: start })?;
        let token = match b {
            b'{' => {
                self.advance();
                Token::BeginObject
            }
            b'}' => {
                self.advance();
                Token::EndObject
            }
            b'[' => {
                self.advance();
                Token::BeginArray
            }
            b']' => {
                self.advance();
                Token::EndArray
            }
            b':' => {
                self.advance();
                Token::NameSeparator
            }
            b',' => {
                self.advance();
                Token::ValueSeparator
            }
            b'"' => self.read_string()?,
            b't' => self.read_literal(b"true", Token::True)?,
            b'f' => self.read_literal(b"false", Token::False)?,
            b'n' => self.read_literal(b"null", Token::Null)?,
            b'-' | b'0'..=b'9' => self.read_number()?,
            _ => {
                return Err(Error::UnexpectedChar {
                    ch: b as char,
                    pos: start,
                });
            }
        };
        Ok(PositionedToken { token, pos: start, end: self.pos })
    }

    fn read_literal(&mut self, expected: &[u8], tok: Token) -> Result<Token, Error> {
        let start = self.pos;
        for &exp in expected {
            match self.peek() {
                Some(b) if b == exp => {
                    self.advance();
                }
                _ => return Err(Error::InvalidLiteral { pos: start }),
            }
        }
        Ok(tok)
    }

    /// RFC 8259 §7: string parsing with escape processing.
    fn read_string(&mut self) -> Result<Token, Error> {
        let start = self.pos;
        // Opening quote
        self.advance();

        let mut out = String::new();
        loop {
            let b = self.peek().ok_or(Error::UnexpectedEof { pos: start })?;
            match b {
                b'"' => {
                    self.advance();
                    return Ok(Token::String(out));
                }
                b'\\' => {
                    let escape_pos = self.pos;
                    self.advance();
                    let esc = self
                        .peek()
                        .ok_or(Error::UnexpectedEof { pos: escape_pos })?;
                    self.advance();
                    let c = match esc {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{0008}',
                        b'f' => '\u{000C}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.read_unicode_escape(escape_pos)?,
                        _ => return Err(Error::InvalidEscape { pos: escape_pos }),
                    };
                    out.push(c);
                }
                // RFC 8259 §7: unescaped control characters U+0000..=U+001F are forbidden
                0x00..=0x1F => {
                    return Err(Error::UnexpectedChar {
                        ch: b as char,
                        pos: self.pos,
                    });
                }
                0x20..=0x7F => {
                    out.push(b as char);
                    self.advance();
                }
                _ => {
                    let ch = self.read_utf8_char()?;
                    out.push(ch);
                }
            }
        }
    }

    /// Read a 4-hex-digit Unicode escape; handle UTF-16 surrogate pairs per RFC 8259 §7.
    fn read_unicode_escape(&mut self, start: Position) -> Result<char, Error> {
        let high = self.read_hex4(start)?;
        // High surrogate range: D800-DBFF must be followed by a low surrogate.
        if (0xD800..=0xDBFF).contains(&high) {
            match (self.peek(), self.peek_at(1)) {
                (Some(b'\\'), Some(b'u')) => {
                    self.advance(); // \
                    self.advance(); // u
                    let low = self.read_hex4(start)?;
                    if !(0xDC00..=0xDFFF).contains(&low) {
                        return Err(Error::InvalidSurrogatePair { pos: start });
                    }
                    let code = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                    char::from_u32(code).ok_or(Error::InvalidSurrogatePair { pos: start })
                }
                _ => Err(Error::InvalidSurrogatePair { pos: start }),
            }
        } else if (0xDC00..=0xDFFF).contains(&high) {
            // Lone low surrogate is invalid.
            Err(Error::InvalidSurrogatePair { pos: start })
        } else {
            char::from_u32(high).ok_or(Error::InvalidUnicodeEscape { pos: start })
        }
    }

    fn read_hex4(&mut self, start: Position) -> Result<u32, Error> {
        let mut val: u32 = 0;
        for _ in 0..4 {
            let b = self.peek().ok_or(Error::UnexpectedEof { pos: start })?;
            let d = match b {
                b'0'..=b'9' => (b - b'0') as u32,
                b'a'..=b'f' => (b - b'a' + 10) as u32,
                b'A'..=b'F' => (b - b'A' + 10) as u32,
                _ => return Err(Error::InvalidUnicodeEscape { pos: start }),
            };
            self.advance();
            val = (val << 4) | d;
        }
        Ok(val)
    }

    /// Decode one UTF-8 codepoint at the current position; advance past it.
    fn read_utf8_char(&mut self) -> Result<char, Error> {
        let start = self.pos;
        let first = self.peek().ok_or(Error::UnexpectedEof { pos: start })?;
        let len = match first {
            0x00..=0x7F => 1,
            0xC2..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF4 => 4,
            _ => return Err(Error::InvalidUtf8 { pos: start }),
        };
        let end = self.pos.offset + len;
        if end > self.input.len() {
            return Err(Error::InvalidUtf8 { pos: start });
        }
        let slice = &self.input[self.pos.offset..end];
        let s = core::str::from_utf8(slice).map_err(|_| Error::InvalidUtf8 { pos: start })?;
        let ch = s.chars().next().ok_or(Error::InvalidUtf8 { pos: start })?;
        // One codepoint = one column, but `len` bytes of offset.
        self.pos.offset = end;
        self.pos.column += 1;
        Ok(ch)
    }

    /// RFC 8259 §6: number = [ minus ] int [ frac ] [ exp ]
    fn read_number(&mut self) -> Result<Token, Error> {
        let start = self.pos;
        let start_off = self.pos.offset;

        // Optional minus
        if self.peek() == Some(b'-') {
            self.advance();
        }

        // int: %x30 / ( %x31-39 *DIGIT )
        match self.peek() {
            Some(b'0') => {
                self.advance();
            }
            Some(b'1'..=b'9') => {
                self.advance();
                while let Some(b'0'..=b'9') = self.peek() {
                    self.advance();
                }
            }
            _ => return Err(Error::InvalidNumber { pos: start }),
        }

        // Optional frac
        if self.peek() == Some(b'.') {
            self.advance();
            match self.peek() {
                Some(b'0'..=b'9') => {}
                _ => return Err(Error::InvalidNumber { pos: start }),
            }
            while let Some(b'0'..=b'9') = self.peek() {
                self.advance();
            }
        }

        // Optional exp
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.advance();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.advance();
            }
            match self.peek() {
                Some(b'0'..=b'9') => {}
                _ => return Err(Error::InvalidNumber { pos: start }),
            }
            while let Some(b'0'..=b'9') = self.peek() {
                self.advance();
            }
        }

        let end_off = self.pos.offset;
        let slice = &self.input[start_off..end_off];
        // Safe: every byte consumed above is ASCII.
        let s = core::str::from_utf8(slice).map_err(|_| Error::InvalidNumber { pos: start })?;
        let n: f64 = s.parse().map_err(|_| Error::InvalidNumber { pos: start })?;
        // RFC 8259 §6 does not permit NaN or Infinity.
        if !n.is_finite() {
            return Err(Error::NumberOutOfRange { pos: start });
        }
        Ok(Token::Number(n))
    }
}
