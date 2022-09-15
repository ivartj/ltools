use std::io::Write;
use std::io::Result;
use std::str::from_utf8_unchecked;

enum State {
    LineStart,
    AttributeType,
    ValueColon,
    SafeStringValue,
    Base64Value,
    WhitespaceBefore(&'static State),
}

#[derive(PartialEq, Debug)]
pub enum Event<'a> {
    TypeChar(char),
    TypeFinish,
    ValueText(&'a str),
    ValueBase64(&'a str),
    ValueFinish,
}

pub trait ReceiveEvent {
    fn receive_event<'a>(&mut self, event: Event<'a>);
}

pub struct Lexer<R> {
    state: State,
    event_receiver: R,
}

impl<R: ReceiveEvent> Lexer<R> {
    pub fn new(event_receiver: R) -> Lexer<R> {
        Lexer{
            state: State::LineStart,
            event_receiver,
        }
    }

    fn emit(&mut self, event: Event) {
        self.event_receiver.receive_event(event);
    }

    pub fn get_ref(&self) -> &R {
        &self.event_receiver
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.event_receiver
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

impl<R: ReceiveEvent> Write for Lexer<R> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut value_start = 0;
        for (i, c) in buf.iter().enumerate() {
            self.state = match self.state {
                State::LineStart => match c {
                    b'\n' => State::LineStart,
                    ALPHA!() => {
                        self.emit(Event::TypeChar((*c).into()));
                        State::AttributeType
                    },
                    DIGIT!() => todo!(), // OIDs
                    _ => todo!(),
                },
                State::AttributeType => match c {
                    b';' => todo!(), // attribute options
                    ALPHA!() | DIGIT!() | b'-' => {
                        self.emit(Event::TypeChar((*c).into()));
                        State::AttributeType
                    },
                    b':' => {
                        self.emit(Event::TypeFinish);
                        State::ValueColon
                    },
                    _ => todo!(),
                },
                State::ValueColon => match c {
                    SAFE_INIT_CHAR!() => {
                        value_start = i;
                        State::SafeStringValue
                    },
                    b' ' => State::WhitespaceBefore(&State::SafeStringValue),
                    b':' => State::WhitespaceBefore(&State::Base64Value),
                    b'\n' => State::LineStart,
                    _ => todo!(),
                },
                State::SafeStringValue => match c {
                    SAFE_CHAR!() => State::SafeStringValue,
                    b'\n' => {
                        self.emit(Event::ValueText(unsafe { from_utf8_unchecked(&buf[value_start..i]) }));
                        self.emit(Event::ValueFinish);
                        State::LineStart
                    },
                    _ => todo!(),
                },
                State::Base64Value => match c {
                    BASE64_CHAR!() => State::Base64Value,
                    b'\n' => {
                        self.emit(Event::ValueBase64(unsafe { from_utf8_unchecked(&buf[value_start..i]) }));
                        self.emit(Event::ValueFinish);
                        State::LineStart
                    },
                    _ => todo!(),
                },
                State::WhitespaceBefore(next_state) => match (next_state, c) {
                    (_, b' ') => State::WhitespaceBefore(next_state),
                    (State::SafeStringValue, SAFE_INIT_CHAR!()) => {
                        value_start = i;
                        State::SafeStringValue
                    },
                    (State::Base64Value, BASE64_CHAR!()) => {
                        value_start = i;
                        State::Base64Value
                    },
                    (_, _) => todo!(),
                },
            };
        }

        match self.state {
            State::SafeStringValue => self.emit(Event::ValueText(unsafe { from_utf8_unchecked(&buf[value_start..]) })),
            State::Base64Value => self.emit(Event::ValueBase64(unsafe { from_utf8_unchecked(&buf[value_start..]) })),
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

    impl<'z> ReceiveEvent for Vec<String> {
        fn receive_event(&mut self, event: Event) {
            self.push(format!("{:?}", event));
        }
    }

    #[test]
    fn it_works() {
        let vec = Vec::new();
        let mut lexer = Lexer::new(vec);
        lexer.write(b"\
                    dn:cn=admin,ou=sa,o=system\n\
                    cn: admin\n\
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

