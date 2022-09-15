use std::io::Write;
use std::io::Result;

#[derive(PartialEq)]
enum State {
    LineStart,
    Text,
    Newline
}

pub struct Unfolder<'a, W> {
    inner: &'a mut W,
    state: State,
}

impl<'a, W: Write> Unfolder<'a, W> {
    pub fn new(inner: &'a mut W) -> Unfolder<'a, W> {
        Unfolder{ inner, state: State::LineStart }
    }
}

impl<'a, W: Write> Write for Unfolder<'a, W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut write_from: usize = 0;
        for (i, c) in buf.iter().enumerate() {
            self.state = match self.state {
                State::LineStart => if *c == b'\n' { State::LineStart } else { State::Text },
                State::Text => if *c == b'\n' { State::Newline } else { State::Text },
                State::Newline => match *c {
                    b' ' => {
                        if i != 0 && i >= write_from {
                            self.inner.write(&buf[write_from..i-1])?;
                        }
                        write_from = i + 1;
                        State::Text
                    },
                    _ => {
                        if i == 0 {
                            self.inner.write(b"\n")?;
                        }
                        if *c == b'\n' { State::LineStart } else { State::Text }
                    },
                }
            }
        }
        let write_to = if self.state == State::Newline {
            buf.len() - 1
        } else {
            buf.len()
        };
        if write_to - write_from > 0 {
            self.inner.write(&buf[write_from..write_to])?;
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
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo\n bar").unwrap();
        assert_eq!(buf.as_slice(), b"foobar");
    }

    #[test]
    pub fn test_b() {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo\n ").unwrap();
        unfolder.write(b"bar").unwrap();
        assert_eq!(buf.as_slice(), b"foobar");
    }

    #[test]
    pub fn test_c() {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo\n").unwrap();
        unfolder.write(b" bar").unwrap();
        assert_eq!(buf.as_slice(), b"foobar");
    }

    #[test]
    pub fn test_d() {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo").unwrap();
        unfolder.write(b"\n bar").unwrap();
        assert_eq!(buf.as_slice(), b"foobar");
    }

    #[test]
    pub fn test_e() {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(&mut buf);
        unfolder.write(b"foo\n").unwrap();
        unfolder.write(b"bar").unwrap();
        assert_eq!(buf.as_slice(), b"foo\nbar");
    }
}
