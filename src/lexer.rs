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

#[derive(Debug)]
pub enum TokenKind {
    AttributeType,
    ValueText,
    ValueBase64,
    ValueFinish,
}

#[derive(Debug)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub loc: Loc,
    pub segment: &'a str,
}

pub trait ReceiveToken {
    fn receive_token<'a>(&mut self, token: Token<'a>);
}

pub struct Lexer<R> {
    state: State,
    token_receiver: R,
    buf: Vec<u8>,
    token_start: Loc,
}

impl<R: ReceiveToken> Lexer<R> {
    pub fn new(token_receiver: R) -> Lexer<R> {
        Lexer{
            state: State::LineStart,
            token_receiver,
            buf: Vec::with_capacity(1028),
            token_start: Loc::new(),
        }
    }

    fn emit(&mut self, token_kind: TokenKind) {
        let segment = unsafe { std::str::from_utf8_unchecked(&self.buf[..]) };
        let token = Token{
            loc: self.token_start,
            kind: token_kind,
            segment,
        };
        self.token_receiver.receive_token(token);
        self.buf.clear();
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

impl<R: ReceiveToken> LocWrite for Lexer<R> {
    fn loc_write(&mut self, loc: Loc, buf: &[u8]) -> Result<usize> {
        let mut loc = loc;
        for c in buf.iter().copied() {
            if !c.is_ascii() {
                return Err(Error::new(ErrorKind::Other, format!("non-ASCII character at line {}, column {}", loc.line, loc.column)));
            }
            self.state = match self.state {
                State::LineStart => match c {
                    b'\n' => State::LineStart,
                    b'#' => State::CommentLine,
                    ALPHA!() => {
                        self.token_start = loc;
                        self.buf.push(c);
                        State::AttributeType
                    },
                    DIGIT!() => todo!(), // OIDs
                    _ => todo!(),
                },
                State::CommentLine => match c {
                    b'\n' => State::LineStart,
                    _ => State::CommentLine,
                },
                State::AttributeType => match c {
                    b';' => todo!(), // attribute options
                    ALPHA!() | DIGIT!() | b'-' => {
                        if self.buf.len() >= MAX_TYPE_LENGTH {
                            let msg = format!("maximum attribute type name length exceeded on line {}, column {}", loc.line, loc.column);
                            return Err(Error::new(ErrorKind::Other, msg));
                        }
                        self.buf.push(c);
                        State::AttributeType
                    },
                    b':' => {
                        self.emit(TokenKind::AttributeType);
                        State::ValueColon
                    },
                    _ => todo!(),
                },
                State::ValueColon => match c {
                    SAFE_INIT_CHAR!() => {
                        self.token_start = loc;
                        self.buf.push(c);
                        State::SafeStringValue
                    },
                    b' ' => State::WhitespaceBefore(&State::SafeStringValue),
                    b':' => State::WhitespaceBefore(&State::Base64Value),
                    b'\n' => State::LineStart,
                    _ => todo!(),
                },
                State::SafeStringValue => match c {
                    SAFE_CHAR!() => {
                        self.buf.push(c);
                        State::SafeStringValue
                    },
                    b'\n' => {
                        self.emit(TokenKind::ValueText);
                        self.emit(TokenKind::ValueFinish);
                        State::LineStart
                    },
                    _ => todo!(),
                },
                State::Base64Value => match c {
                    BASE64_CHAR!() => {
                        self.buf.push(c);
                        State::Base64Value
                    },
                    b'\n' => {
                        self.emit(TokenKind::ValueBase64);
                        self.emit(TokenKind::ValueFinish);
                        State::LineStart
                    },
                    _ => todo!(),
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
                    (_, _) => todo!(),
                },
            };
            loc = loc.after(c);
        }

        match self.state {
            State::SafeStringValue => self.emit(TokenKind::ValueText),
            State::Base64Value => self.emit(TokenKind::ValueBase64),
            _ => (),
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl<'z> ReceiveToken for Vec<String> {
        fn receive_token(&mut self, token: Token) {
            self.push(format!("{:?}", token));
        }
    }

    #[test]
    fn it_works() {
        let vec = Vec::new();
        let mut lexer = Lexer::new(vec);
        lexer.loc_write(Loc::new(),
                    b"\
                    # comment 1\n\
                    dn:cn=admin,ou=sa,o=system\n\
                    # comment 2\n\
                    # comment 3\n\
                    cn: admin\n\
                    # comment 4\n\
                    sn:: MO4Z2VzdMO4bA==\n\
                    ").expect("success");
        let mut iter = lexer.get_ref().iter();
        assert_eq!(iter.next(), Some(&String::from("TypeChar('d')")));
        assert_eq!(iter.next(), Some(&String::from("TypeChar('n')")));
        assert_eq!(iter.next(), Some(&String::from("TypeFinish")));
        assert_eq!(iter.next(), Some(&String::from("ValueText(\"cn=admin,ou=sa,o=system\")")));
        assert_eq!(iter.next(), Some(&String::from("ValueFinish")));

        assert_eq!(iter.next(), Some(&String::from("TypeChar('c')")));
        assert_eq!(iter.next(), Some(&String::from("TypeChar('n')")));
        assert_eq!(iter.next(), Some(&String::from("TypeFinish")));
        assert_eq!(iter.next(), Some(&String::from("ValueText(\"admin\")")));
        assert_eq!(iter.next(), Some(&String::from("ValueFinish")));

        assert_eq!(iter.next(), Some(&String::from("TypeChar('s')")));
        assert_eq!(iter.next(), Some(&String::from("TypeChar('n')")));
        assert_eq!(iter.next(), Some(&String::from("TypeFinish")));
        assert_eq!(iter.next(), Some(&String::from("ValueBase64(\"MO4Z2VzdMO4bA==\")")));
        assert_eq!(iter.next(), Some(&String::from("ValueFinish")));
    }
}

