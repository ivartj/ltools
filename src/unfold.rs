use std::io::Write;
use std::io::Result;

#[derive(PartialEq)]
enum State {
    LineStart,
    Text,
    NewlineAfterText,
}

pub struct Unfolder<W> {
    inner: W,
    state: State,
}

impl<W: Write> Unfolder<W> {
    pub fn new(inner: W) -> Unfolder<W> {
        Unfolder{ inner, state: State::LineStart }
    }
}

impl<W: Write> Write for Unfolder<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut write_from = 0;
        for (i, c) in buf.iter().copied().enumerate() {
            self.state = match self.state {
                State::LineStart => match c {
                    b'\n' => State::LineStart,
                    _ => State::Text,
                },
                State::Text => match c {
                    b'\n' => State::NewlineAfterText,
                    _ => State::Text,
                },
                State::NewlineAfterText => {
                    if c == b' ' {
                        if i > 1 {
                            let write_until = i - 1;
                            self.inner.write(&buf[write_from..write_until])?;
                        }
                        write_from = i + 1;
                        State::Text
                    } else {
                        if i == 0 {
                            self.inner.write(b"\n")?;
                        }
                        match c {
                            b'\n' => State::LineStart,
                            _ => State::Text,
                        }
                    }
                },
            };
        }
        if self.state == State::NewlineAfterText {
            if buf.len() > write_from + 1 {
                let write_until = buf.len() - 1;
                self.inner.write(&buf[write_from..write_until])?;
            }
        } else {
            self.inner.write(&buf[write_from..])?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_a() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo\n bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_b() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo\n ")?;
        unfolder.write(b"bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_c() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo\n")?;
        unfolder.write(b" bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_d() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo")?;
        unfolder.write(b"\n bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_e() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo\n")?;
        unfolder.write(b"bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foo\nbar");
        Ok(())
    }

    #[test]
    pub fn test_f() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo\n")?;
        unfolder.write(b"\nbar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foo\n\nbar");
        Ok(())
    }

    #[test]
    pub fn test_g() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"a\n b\n")?;
        unfolder.write(b" c")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "abc");
        Ok(())
    }

    #[test]
    pub fn test_h() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"a\n")?;
        unfolder.write(b"")?;
        unfolder.write(b" b")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "ab");
        Ok(())
    }
}
