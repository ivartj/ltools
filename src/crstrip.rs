use std::io::Write;
use std::io::Result;

#[derive(PartialEq)]
enum State {
    Cr,
    Text,
}

pub struct CrStripper<W> {
    inner: W,
    state: State,
}

impl<W: Write> CrStripper<W> {
    pub fn new(inner: W) -> CrStripper<W> {
        CrStripper{ inner, state: State::Text }
    }
}

impl<W: Write> Write for CrStripper<W> {
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
    pub fn test_a() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(&mut buf);
        crstripper.write(b"foo\r\nbar")?;
        assert_eq!(buf.as_slice(), b"foo\nbar");
        Ok(())
    }

    #[test]
    pub fn test_b() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(&mut buf);
        crstripper.write(b"foo\r")?;
        crstripper.write(b"\nbar")?;
        assert_eq!(buf.as_slice(), b"foo\nbar");
        Ok(())
    }
}
