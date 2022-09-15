use std::io::Write;
use std::io::Result;

#[derive(PartialEq)]
enum State {
    Cr,
    Text,
}

pub struct CrStripper<'a, W> {
    inner: &'a mut W,
    state: State,
}

impl<'a, W: Write> CrStripper<'a, W> {
    pub fn new(inner: &'a mut W) -> CrStripper<'a, W> {
        CrStripper{ inner, state: State::Text }
    }
}

impl<'a, W: Write> Write for CrStripper<'a, W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut write_from: usize = 0;
        for (i, c) in buf.iter().enumerate() {
            self.state = match self.state {
                State::Text => {
                    if *c == b'\r' {
                        if i > write_from {
                            self.inner.write(&buf[write_from..i])?;
                        }
                        write_from = i + 1;
                        State::Cr
                    } else {
                        State::Text
                    }
                },
                State::Cr => {
                    match c {
                        b'\n' => {
                            State::Text
                        },
                        b'\r' => {
                            self.inner.write(b"\r")?;
                            write_from = i + 1;
                            State::Cr
                        }
                        _ => {
                            self.inner.write(b"\r")?;
                            write_from = i;
                            State::Text
                        }
                    }
                },
            }
        }
        if write_from < buf.len() {
            self.inner.write(&buf[write_from..])?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_a() {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(&mut buf);
        crstripper.write(b"foo\r\nbar").unwrap();
        assert_eq!(buf.as_slice(), b"foo\nbar");
    }

    #[test]
    pub fn test_b() {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(&mut buf);
        crstripper.write(b"foo\r").unwrap();
        crstripper.write(b"\nbar").unwrap();
        assert_eq!(buf.as_slice(), b"foo\nbar");
    }
}
