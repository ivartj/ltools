use std::io::{ Write, Result, Error, ErrorKind };
use std::default::Default;

#[derive(Clone, Copy)]
enum State {
    B0, // 0 bits filled
    B6, // 6 bits filled
    B4, // 4 bits filled
    B2, // 2 bits filled
    P0, // expecting zero more padding
    P1, // expecting one more padding
}

#[derive(Clone, Copy)]
pub struct DecodeState {
    state: State,
    octet: u8,
}

impl Default for DecodeState {
    fn default() -> Self {
        DecodeState{ state: State::B0, octet: 0u8 }
    }
}

pub struct DecodeWriter<W: Write> {
    inner: W,
    state: State,
    octet: u8,
}

impl<W: Write> DecodeWriter<W> {
    pub fn new(inner: W) -> DecodeWriter<W> {
        DecodeWriter{ inner, state: State::B0, octet: 0u8 }
    }

    pub fn new_with_state(inner: W, state: DecodeState) -> DecodeWriter<W> {
        DecodeWriter{
            inner,
            state: state.state,
            octet: state.octet,
        }
    }

    pub fn get_ptr(&self) -> &W {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    pub fn get_state(&self) -> DecodeState {
        DecodeState {
            state: self.state,
            octet: self.octet,
        }
    }
}

fn decode_value_of(digit: u8) -> Option<u8> {
    match digit {
        b'A'..=b'Z' => Some(digit - b'A'),
        b'a'..=b'z' => Some(digit - b'a' + 26),
        b'0'..=b'9' => Some(digit - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

impl<W: Write> Write for DecodeWriter<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        for c in buf.iter().copied() {
            self.state = match (self.state, c, decode_value_of(c)) {
                (State::B0, _, Some(value)) => {
                    self.octet = value << 2;
                    State::B6
                },
                (State::B6, _, Some(value)) => {
                    self.octet |= value >> 4;
                    self.inner.write_all(&[self.octet])?;
                    self.octet = value << 4;
                    State::B4
                },
                (State::B4, _, Some(value)) => {
                    self.octet |= value >> 2;
                    self.inner.write_all(&[self.octet])?;
                    self.octet = value << 6;
                    State::B2
                },
                (State::B4, b'=', _) => State::P1,
                (State::B2, _, Some(value)) => {
                    self.octet |= value;
                    self.inner.write_all(&[self.octet])?;
                    State::B0
                },
                (State::B2 | State::P1, b'=', _) => State::P0,
                (_, _, _) => return Err(Error::new(ErrorKind::InvalidData, format!("unexpected character 0x{:02X}", c))),
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        match self.state {
            // valid final states
            State::P0 | State::B0 => Ok(()),
            // other states
            _ => Err(Error::new(ErrorKind::Other, "base64 decoder flushed on invalid end state")),
        }
    }
}

enum EncodeState {
    B0, // 0 bits filled
    B2, // 2 bits filled
    B4, // 4 bits filled
}

fn encode_value_of(u6: u8) -> u8 {
    match u6 {
        0..=25 => b'A' + u6,
        26..=51 => u6 - 26 + b'a',
        52..=61 => u6 - 52 + b'0',
        62 => b'+',
        63 => b'/',
        _ => panic!("encode_value_of is not defined for {u6}"),
    }
}

pub struct EncodeWriter<W: Write> {
    inner: W,
    state: EncodeState,
    u6: u8,
}

impl<W: Write> EncodeWriter<W> {
    pub fn new(inner: W) -> EncodeWriter<W> {
        EncodeWriter{
            inner,
            state: EncodeState::B0,
            u6: 0,
        }
    }

    fn emit(&mut self) -> Result<()> {
        self.inner.write_all(&[encode_value_of(self.u6)])?;
        Ok(())
    }
}

impl<W: Write> Write for EncodeWriter<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        for c in buf.iter().copied() {
            self.state = match self.state {
                EncodeState::B0 => {
                    self.u6 = c >> 2;
                    self.emit()?;
                    self.u6 = (c << 4) & 0x3F;
                    EncodeState::B2
                },
                EncodeState::B2 => {
                    self.u6 |= c >> 4;
                    self.emit()?;
                    self.u6 = (c << 2) & 0x3F;
                    EncodeState::B4
                }
                EncodeState::B4 => {
                    self.u6 |= c >> 6;
                    self.emit()?;
                    self.u6 = c & 0x3F;
                    self.emit()?;
                    EncodeState::B0
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        match self.state {
            EncodeState::B0 => {},
            EncodeState::B2 => {
                self.emit()?;
                self.inner.write_all(b"==")?;
            },
            EncodeState::B4 => {
                self.emit()?;
                self.inner.write_all(b"=")?;
            },
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    pub use super::*;

    #[test]
    fn test1() -> Result<()> {
        let mut decoder = DecodeWriter::new(Vec::new());
        decoder.write(b"SGVsbG8gd29ybGQ=")?;
        decoder.flush()?;
        assert_eq!(std::str::from_utf8(decoder.get_ptr()), Ok("Hello world"));
        Ok(())
    }

    #[test]
    fn test2() -> Result<()> {
        let mut decoder = DecodeWriter::new(Vec::new());
        decoder.write(b"SGVsbG8gd29ybGQh")?;
        decoder.flush()?;
        assert_eq!(std::str::from_utf8(decoder.get_ptr()), Ok("Hello world!"));
        Ok(())
    }

    #[test]
    fn test3() -> Result<()> {
        let mut decoder = DecodeWriter::new(Vec::new());
        decoder.write(b"SGVsbG93b3JsZA==")?;
        decoder.flush()?;
        assert_eq!(std::str::from_utf8(decoder.get_ptr()), Ok("Helloworld"));
        Ok(())
    }

    #[test]
    fn test_invalid_data() {
        let mut buf = Vec::new();
        let mut decoder = DecodeWriter::new(&mut buf);
        let result = decoder.write(b"\r");
        if let Err(error) = result {
            assert_eq!(error.kind(), ErrorKind::InvalidData);
            assert_eq!(error.to_string(), "unexpected character 0x0D");
        } else {
            panic!("expected error");
        }
    }

    #[test]
    fn test_encode_a() -> Result<()> {
        let mut buf: Vec<u8> = Vec::new();
        let mut encoder = EncodeWriter::new(&mut buf);
        encoder.write(b"abcd")?;
        encoder.flush()?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "YWJjZA==");
        Ok(())
    }

    #[test]
    fn test_encode_b() -> Result<()> {
        let mut buf: Vec<u8> = Vec::new();
        let mut encoder = EncodeWriter::new(&mut buf);
        encoder.write(b"12345678")?;
        encoder.flush()?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "MTIzNDU2Nzg=");
        Ok(())
    }
}

