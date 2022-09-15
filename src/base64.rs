use std::io::{ Write, Result };

enum State {
    B0, // 0 bits filled
    B6, // 6 bits filled
    B4, // 4 bits filled
    B2, // 2 bits filled
    P0, // expecting zero more padding
    P1, // expecting one more padding
}

pub struct Decoder<W: Write> {
    inner: W,
    state: State,
    octet: u8,
}

impl<W: Write> Decoder<W> {
    pub fn new(inner: W) -> Decoder<W> {
        Decoder{ inner, state: State::B0, octet: 0u8 }
    }

    pub fn get_ptr(&self) -> &W {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }
}

fn value_of(digit: u8) -> Option<u8> {
    match digit {
        b'A'..=b'Z' => Some(digit - b'A'),
        b'a'..=b'z' => Some(digit - b'a' + 26),
        b'0'..=b'9' => Some(digit - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

impl<W: Write> Write for Decoder<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        for c in buf.into_iter().copied() {
            self.state = match (c, value_of(c)) {
                (_, Some(value)) => match self.state {
                    State::B0 => {
                        self.octet = value << 2;
                        State::B6
                    },
                    State::B6 => {
                        self.octet |= value >> 4;
                        self.inner.write(&[self.octet])?;
                        self.octet = value << 4;
                        State::B4
                    },
                    State::B4 => {
                        self.octet |= value >> 2;
                        self.inner.write(&[self.octet])?;
                        self.octet = value << 6;
                        State::B2
                    },
                    State::B2 => {
                        self.octet |= value;
                        self.inner.write(&[self.octet])?;
                        State::B0
                    },
                    _ => todo!(),
                },
                (b'=', _) => {
                    match self.state {
                        State::B4 => State::P1,
                        State::B2 => State::P0,
                        State::P1 => State::P0,
                        _ => todo!(),
                    }
                },
                _ => todo!(),
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    pub use super::*;

    #[test]
    fn test1() -> Result<()> {
        let mut decoder = Decoder::new(Vec::new());
        decoder.write(b"SGVsbG8gd29ybGQ=")?;
        assert_eq!(std::str::from_utf8(decoder.get_ptr()), Ok("Hello world"));
        Ok(())
    }

    #[test]
    fn test2() -> Result<()> {
        let mut decoder = Decoder::new(Vec::new());
        decoder.write(b"SGVsbG8gd29ybGQh")?;
        assert_eq!(std::str::from_utf8(decoder.get_ptr()), Ok("Hello world!"));
        Ok(())
    }

    #[test]
    fn test3() -> Result<()> {
        let mut decoder = Decoder::new(Vec::new());
        decoder.write(b"SGVsbG93b3JsZA==")?;
        assert_eq!(std::str::from_utf8(decoder.get_ptr()), Ok("Helloworld"));
        Ok(())
    }
}

