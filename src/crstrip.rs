use std::io::Write;
use std::io::Result;

#[derive(PartialEq)]
enum State {
    Normal,
    Cr,
}

pub struct CrStripper<W> {
    inner: W,
    state: State,
}

impl<W: Write> CrStripper<W> {
    pub fn new(inner: W) -> CrStripper<W> {
        CrStripper{ inner, state: State::Normal }
    }
}

impl<W: Write> Write for CrStripper<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut write_from = 0;
        for (i, c) in buf.iter().copied().enumerate() {
            self.state = match self.state {
                State::Normal => match c {
                    b'\r' => State::Cr,
                    _ => State::Normal,
                },
                State::Cr => {
                    if c == b'\n' {
                        if i > 1 {
                            self.inner.write(&buf[write_from..i-1])?;
                        }
                        write_from = i;
                        State::Normal
                    } else {
                        if i == 0 {
                            self.inner.write(b"\r")?;
                        }
                        match c {
                            b'\r' => State::Cr,
                            _ => State::Normal,
                        }
                    }
                },
            };
        }
        let write_until = if self.state == State::Cr {
            buf.len() - 1
        } else {
            buf.len()
        };

        self.inner.write(&buf[write_from..write_until])?;

        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        todo!()
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
        assert_eq!(String::from_utf8_lossy(buf.as_slice()), "foo\nbar");
        Ok(())
    }

    #[test]
    pub fn test_b() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(&mut buf);
        crstripper.write(b"foo\r")?;
        crstripper.write(b"\nbar")?;
        assert_eq!(String::from_utf8_lossy(buf.as_slice()), "foo\nbar");
        Ok(())
    }

    #[test]
    pub fn test_c() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(&mut buf);
        crstripper.write(b"foo\r\r\nbar")?;
        assert_eq!(String::from_utf8_lossy(buf.as_slice()), "foo\r\nbar");
        Ok(())
    }

    #[test]
    pub fn test_d() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(&mut buf);
        crstripper.write(b"foo\r\rbar")?;
        assert_eq!(String::from_utf8_lossy(buf.as_slice()), "foo\r\rbar");
        Ok(())
    }
}

