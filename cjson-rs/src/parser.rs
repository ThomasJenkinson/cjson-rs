//! RFC 8259 parser.
//!
//! Consumes a UTF-8 byte slice and produces a `Value` tree. Enforces a
//! configurable maximum nesting depth to prevent stack overflow on
//! adversarial input (matches cJSON's `CJSON_NESTING_LIMIT` default of
//! 1000).

use crate::error::{Error, Position};
use crate::token::{PositionedToken, Token};
use crate::tokeniser::Tokeniser;
use crate::value::Value;

/// Parser configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParserOptions {
    /// Maximum depth of nested arrays and objects before parsing is
    /// rejected. Default 1000, matching cJSON's `CJSON_NESTING_LIMIT`.
    pub nesting_limit: usize,
}

impl Default for ParserOptions {
    fn default() -> Self {
        Self {
            nesting_limit: 1000,
        }
    }
}

/// Parse a complete JSON document with default options.
pub fn parse(input: &[u8]) -> Result<Value, Error> {
    parse_with_options(input, &ParserOptions::default())
}

/// Parse a complete JSON document with caller-supplied options.
pub fn parse_with_options(input: &[u8], opts: &ParserOptions) -> Result<Value, Error> {
      let tokens = Tokeniser::tokenise(input)?;
      let mut parser = Parser {
          tokens,
          cursor: 0,
          depth: 0,
          limit: opts.nesting_limit,
      };
      let value = parser.parse_value()?;
      if parser.cursor < parser.tokens.len() {
          return Err(Error::TrailingData {
              pos: parser.tokens[parser.cursor].pos,
          });
      }
      Ok(value)
}

/// Parse a single JSON value from the prefix of `input` and return both the
/// value and the byte offset immediately after its last consumed byte (i.e.
/// not counting any trailing whitespace or unparsed bytes). Trailing
/// whitespace, garbage, or extra values are allowed and ignored.
///
/// Backs cJSON's `cJSON_ParseWithOpts(..., require_null_terminated = false)`
/// — see `cjson-rs-ffi/src/lib.rs::cJSON_ParseWithOpts`.
pub fn parse_prefix(input: &[u8]) -> Result<(Value, usize), Error> {
    let (tokens, _) = Tokeniser::tokenise_prefix(input);
    let mut parser = Parser {
        tokens,
        cursor: 0,
        depth: 0,
        limit: ParserOptions::default().nesting_limit,
    };
    let value = parser.parse_value()?;
    // parse_value consumed at least one token, so cursor >= 1 and the
    // expression below is safe.
    let parse_end = parser.tokens[parser.cursor - 1].end.offset;
    Ok((value, parse_end))
}

struct Parser {
    tokens: Vec<PositionedToken>,
    cursor: usize,
    depth: usize,
    limit: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor).map(|pt| &pt.token)
    }

    fn peek_pos(&self) -> Position {
        self.tokens
            .get(self.cursor)
            .map(|pt| pt.pos)
            .or_else(|| self.tokens.last().map(|pt| pt.pos))
            .unwrap_or(Position::START)
    }

    fn advance(&mut self) {
        self.cursor += 1;
    }

    fn enter(&mut self, pos: Position) -> Result<(), Error> {
        if self.depth >= self.limit {
            return Err(Error::NestingLimitExceeded { pos });
        }
        self.depth += 1;
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    /// RFC 8259 §3: value = false / null / true / object / array / number / string
    fn parse_value(&mut self) -> Result<Value, Error> {
        let pos = self.peek_pos();
        let tok = self.peek().ok_or(Error::UnexpectedEof { pos })?;
        match tok {
            Token::Null => {
                self.advance();
                Ok(Value::Null)
            }
            Token::True => {
                self.advance();
                Ok(Value::Bool(true))
            }
            Token::False => {
                self.advance();
                Ok(Value::Bool(false))
            }
            Token::Number(n) => {
                let n = *n;
                self.advance();
                Ok(Value::Number(n))
            }
            Token::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(Value::String(s))
            }
            Token::BeginArray => self.parse_array(),
            Token::BeginObject => self.parse_object(),
            _ => Err(Error::UnexpectedToken { pos }),
        }
    }

    /// RFC 8259 §5: array = begin-array [ value *( value-separator value ) ] end-array
    fn parse_array(&mut self) -> Result<Value, Error> {
        let open_pos = self.peek_pos();
        self.enter(open_pos)?;
        self.advance(); // consume [

        let mut items = Vec::new();

        if matches!(self.peek(), Some(Token::EndArray)) {
            self.advance();
            self.leave();
            return Ok(Value::Array(items));
        }

        loop {
            items.push(self.parse_value()?);
            match self.peek() {
                Some(Token::ValueSeparator) => {
                    let comma_pos = self.peek_pos();
                    self.advance();
                    // Reject trailing comma: next token must start a value, not close.
                    if matches!(self.peek(), Some(Token::EndArray)) {
                        return Err(Error::UnexpectedToken { pos: comma_pos });
                    }
                }
                Some(Token::EndArray) => {
                    self.advance();
                    self.leave();
                    return Ok(Value::Array(items));
                }
                Some(_) => {
                    return Err(Error::UnexpectedToken {
                        pos: self.peek_pos(),
                    });
                }
                None => {
                    return Err(Error::UnexpectedEof {
                        pos: self.peek_pos(),
                    });
                }
            }
        }
    }

    /// RFC 8259 §4: object = begin-object [ member *( value-separator member ) ] end-object
    ///              member = string name-separator value
    fn parse_object(&mut self) -> Result<Value, Error> {
        let open_pos = self.peek_pos();
        self.enter(open_pos)?;
        self.advance(); // consume {

        let mut members = Vec::new();

        if matches!(self.peek(), Some(Token::EndObject)) {
            self.advance();
            self.leave();
            return Ok(Value::Object(members));
        }

        loop {
            // Expect string key.
            let key_pos = self.peek_pos();
            let key = match self.peek() {
                Some(Token::String(s)) => {
                    let s = s.clone();
                    self.advance();
                    s
                }
                Some(_) => return Err(Error::UnexpectedToken { pos: key_pos }),
                None => return Err(Error::UnexpectedEof { pos: key_pos }),
            };

            // Expect name-separator (:).
            match self.peek() {
                Some(Token::NameSeparator) => self.advance(),
                Some(_) => {
                    return Err(Error::UnexpectedToken {
                        pos: self.peek_pos(),
                    })
                }
                None => {
                    return Err(Error::UnexpectedEof {
                        pos: self.peek_pos(),
                    })
                }
            }

            // Parse value.
            let value = self.parse_value()?;
            members.push((key, value));

            // Expect value-separator (,) or end-object (}).
            match self.peek() {
                Some(Token::ValueSeparator) => {
                    let comma_pos = self.peek_pos();
                    self.advance();
                    if matches!(self.peek(), Some(Token::EndObject)) {
                        return Err(Error::UnexpectedToken { pos: comma_pos });
                    }
                }
                Some(Token::EndObject) => {
                    self.advance();
                    self.leave();
                    return Ok(Value::Object(members));
                }
                Some(_) => {
                    return Err(Error::UnexpectedToken {
                        pos: self.peek_pos(),
                    })
                }
                None => {
                    return Err(Error::UnexpectedEof {
                        pos: self.peek_pos(),
                    })
                }
            }
        }
    }
}
