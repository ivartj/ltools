use std::io::{ Result, Error, ErrorKind };
use crate::loc::{ Loc, LocWrite };

const MAX_TYPE_LENGTH: usize = 1024;

enum State {
    LineStart,
    CommentLine,
    AttributeType,
    ValueColon,
    SafeStringValue,
    Base64Value,
    WhitespaceBefore(&'static State),
}

#[derive(Debug, Eq, PartialEq)]
pub enum TokenKind {
    AttributeType,
    ValueText,
    ValueBase64,
    ValueFinish,
    EmptyLine,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub loc: Loc,
    pub segment: &'a str,
}

pub trait WriteToken {
    fn write_token(&mut self, token: Token) -> Result<()>;
}

pub struct Lexer<R> {
    state: State,
    token_receiver: R,
    buf: Vec<u8>,
    token_start: Loc,
}

impl<R: WriteToken> Lexer<R> {
    pub fn new(token_receiver: R) -> Lexer<R> {
        Lexer{
            state: State::LineStart,
            token_receiver,
            buf: Vec::with_capacity(1028),
            token_start: Loc::default(),
        }
    }

    fn emit(&mut self, token_kind: TokenKind) -> Result<()> {
        let segment = unsafe { std::str::from_utf8_unchecked(&self.buf[..]) };
        let token = Token{
            loc: self.token_start,
            kind: token_kind,
            segment,
        };
        self.token_receiver.write_token(token)?;
        self.buf.clear();
        Ok(())
    }

    pub fn get_ref(&self) -> &R {
        &self.token_receiver
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.token_receiver
    }
}

macro_rules! SAFE_INIT_CHAR {
    () => { 0x01..=0x09 | 0x0b..=0x0c | 0x0e..=0x1f | 0x21..=0x39 | 0x3b | 0x3d..=0x7d };
}

macro_rules! SAFE_CHAR {
    () => { 0x01..=0x09 | 0x0b..=0x0c | 0x0e..=0x7f };
}

macro_rules! ALPHA {
    () => { b'A'..=b'Z' | b'a'..=b'z' };
}

macro_rules! DIGIT {
    () => { b'0'..=b'9' };
}

macro_rules! BASE64_CHAR {
    () => { b'+' | b'/' | b'=' | DIGIT!() | ALPHA!() };
}

impl<R: WriteToken> LocWrite for Lexer<R> {
    fn loc_write(&mut self, loc: Loc, buf: &[u8]) -> Result<usize> {
        let mut loc = loc;
        for c in buf.iter().copied() {
            if !c.is_ascii() {
                return Err(Error::new(ErrorKind::Other, format!("non-ASCII character at line {}, column {}", loc.line, loc.column)));
            }
            self.state = match self.state {
                State::LineStart => match c {
                    b'\n' => {
                        self.emit(TokenKind::EmptyLine)?;
                        State::LineStart
                    },
                    b'#' => State::CommentLine,
                    ALPHA!() => {
                        self.token_start = loc;
                        self.buf.push(c);
                        State::AttributeType
                    },
                    DIGIT!() => {
                        return Err(Error::new(ErrorKind::Other, format!("unexpected digit on line {}, column {} (OID attribute types are not yet supported)", loc.line, loc.column)));
                    }, 
                    _ => {
                        return Err(Error::new(ErrorKind::Other, format!("unexpected character on line {}, column {}", loc.line, loc.column)));
                    },
                },
                State::CommentLine => match c {
                    b'\n' => State::LineStart,
                    _ => State::CommentLine,
                },
                State::AttributeType => match c {
                    b';' => {
                        return Err(Error::new(ErrorKind::Other, format!("unexpected semicolon on line {}, column {} (attribute options are not yet supported)", loc.line, loc.column)));
                    },
                    ALPHA!() | DIGIT!() | b'-' => {
                        if self.buf.len() >= MAX_TYPE_LENGTH {
                            let msg = format!("maximum attribute type name length exceeded on line {}, column {}", loc.line, loc.column);
                            return Err(Error::new(ErrorKind::Other, msg));
                        }
                        self.buf.push(c);
                        State::AttributeType
                    },
                    b':' => {
                        self.emit(TokenKind::AttributeType)?;
                        State::ValueColon
                    },
                    _ => return Err(Error::new(ErrorKind::Other, format!("unexpected character in attribute type name on line {}, column {}", loc.line, loc.column))),
                },
                State::ValueColon => match c {
                    SAFE_INIT_CHAR!() => {
                        self.token_start = loc;
                        self.buf.push(c);
                        State::SafeStringValue
                    },
                    b' ' => State::WhitespaceBefore(&State::SafeStringValue),
                    b':' => State::WhitespaceBefore(&State::Base64Value),
                    b'\n' => {
                        self.emit(TokenKind::ValueText)?;
                        self.emit(TokenKind::ValueFinish)?;
                        State::LineStart
                    },
                    b'<' => return Err(Error::new(ErrorKind::Other, format!("unexpected '<' on line {}, column {} (URL values not implemented at this time)", loc.line, loc.column))),
                    _ => return Err(Error::new(ErrorKind::Other, format!("unexpected character on line {}, column {} (expecting attribute value)", loc.line, loc.column))),
                },
                State::SafeStringValue => match c {
                    SAFE_CHAR!() => {
                        self.buf.push(c);
                        State::SafeStringValue
                    },
                    b'\n' => {
                        self.emit(TokenKind::ValueText)?;
                        self.emit(TokenKind::ValueFinish)?;
                        State::LineStart
                    },
                    _ => return Err(Error::new(ErrorKind::Other, format!("illegal LDIF safe-string character on line {}, column {} (a work-around is to base64-encode the value)", loc.line, loc.column))),
                },
                State::Base64Value => match c {
                    BASE64_CHAR!() => {
                        self.buf.push(c);
                        State::Base64Value
                    },
                    b'\n' => {
                        self.emit(TokenKind::ValueBase64)?;
                        self.emit(TokenKind::ValueFinish)?;
                        State::LineStart
                    },
                    _ => return Err(Error::new(ErrorKind::Other, format!("unexpected character on line {}, column {} while expecting base64 code", loc.line, loc.column))),
                },
                State::WhitespaceBefore(next_state) => match (next_state, c) {
                    (_, b' ') => State::WhitespaceBefore(next_state),
                    (State::SafeStringValue, SAFE_INIT_CHAR!()) => {
                        self.token_start = loc;
                        self.buf.push(c);
                        State::SafeStringValue
                    },
                    (State::Base64Value, BASE64_CHAR!()) => {
                        self.token_start = loc;
                        self.buf.push(c);
                        State::Base64Value
                    },
                    (_, _) => return Err(Error::new(ErrorKind::Other, format!("unexpected character on line {}, column {} while expecting value after attribute type", loc.line, loc.column))),
                },
            };
            loc = loc.after(c);
        }

        match self.state {
            State::SafeStringValue => self.emit(TokenKind::ValueText)?,
            State::Base64Value => self.emit(TokenKind::ValueBase64)?,
            _ => (),
        }
        Ok(buf.len())
    }

    /// This method is used to indicate end-of-file.
    fn loc_flush(&mut self, loc: Loc) -> Result<()> {
        match self.state {
            State::LineStart => self.emit(TokenKind::EmptyLine)?,
            State::CommentLine => self.emit(TokenKind::EmptyLine)?,
            State::AttributeType => return Err(Error::new(ErrorKind::Other, format!("unexpected end of file on on line {}, column {} inside attribute type", loc.line, loc.column))),
            State::ValueColon | State::SafeStringValue | State::WhitespaceBefore(_) => {
                self.emit(TokenKind::ValueText)?;
                self.emit(TokenKind::ValueFinish)?;
                self.emit(TokenKind::EmptyLine)?;
            },
            State::Base64Value => {
                self.emit(TokenKind::ValueBase64)?;
                self.emit(TokenKind::ValueFinish)?;
                self.emit(TokenKind::EmptyLine)?;
            },
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct TokenCopy {
        kind: TokenKind,
        loc: Loc,
        segment: String,
    }

    impl TokenCopy {
        fn type_and_segment(self) -> (TokenKind, String) {
            (self.kind, self.segment)
        }
    }

    impl WriteToken for &mut Vec<TokenCopy> {
        fn write_token(&mut self, token: Token) -> Result<()> {
            self.push(TokenCopy{
                kind: token.kind,
                loc: token.loc,
                segment: token.segment.to_owned(),
            });
            Ok(())
        }
    }

    #[test]
    fn it_works() {
        let mut vec = Vec::new();
        let mut lexer = Lexer::new(&mut vec);
        lexer.loc_write(Loc::default(),
                    b"\
                    # comment 1\n\
                    dn:cn=admin,ou=sa,o=system\n\
                    # comment 2\n\
                    # comment 3\n\
                    cn: admin\n\
                    # comment 4\n\
                    sn:: MO4Z2VzdMO4bA==\n\
                    \n\
                    dn: cn=uaadmin,ou=sa,o=data\n\
                    ").expect("success");
        let tuples: Vec<(TokenKind, String)> = vec.into_iter().map(TokenCopy::type_and_segment).collect();
        assert_eq!(tuples[0], (TokenKind::AttributeType, String::from("dn")));
        assert_eq!(tuples[1], (TokenKind::ValueText, String::from("cn=admin,ou=sa,o=system")));
        assert_eq!(tuples[2], (TokenKind::ValueFinish, String::from("")));

        assert_eq!(tuples[3], (TokenKind::AttributeType, String::from("cn")));
        assert_eq!(tuples[4], (TokenKind::ValueText, String::from("admin")));
        assert_eq!(tuples[5], (TokenKind::ValueFinish, String::from("")));

        assert_eq!(tuples[6], (TokenKind::AttributeType, String::from("sn")));
        assert_eq!(tuples[7], (TokenKind::ValueBase64, String::from("MO4Z2VzdMO4bA==")));
        assert_eq!(tuples[8], (TokenKind::ValueFinish, String::from("")));

        assert_eq!(tuples[9], (TokenKind::EmptyLine, String::from("")));

        assert_eq!(tuples[10], (TokenKind::AttributeType, String::from("dn")));
        assert_eq!(tuples[11], (TokenKind::ValueText, String::from("cn=uaadmin,ou=sa,o=data")));
        assert_eq!(tuples[12], (TokenKind::ValueFinish, String::from("")));
        assert_eq!(tuples.len(), 13);
    }
}

