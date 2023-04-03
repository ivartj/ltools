use std::io::Result;
use crate::loc::{ Loc, LocWrite };
use crate::skip::{ Skipper, SkipState };

#[derive(PartialEq, Copy, Clone)]
enum State {
    Text,
    Newline,
}

pub struct Unfolder<LW: LocWrite> {
    inner: LW,
    state: State,
    skipstate: SkipState,
}

impl<LW: LocWrite> Unfolder<LW> {
    pub fn new(inner: LW) -> Unfolder<LW> {
        Unfolder{
            inner,
            state: State::Text,
            skipstate: SkipState::default(),
        }
    }

}

impl<LW: LocWrite> LocWrite for Unfolder<LW> {
    fn loc_write(&mut self, loc: Loc, buf: &[u8]) -> Result<usize> {
        let mut skipper = Skipper::new_with_state(&mut self.inner, loc, buf, self.skipstate);
        while let Some(c) = skipper.lookahead() {
            self.state = match (self.state, c) {
                (State::Text, b'\n') => {
                    skipper.begin_skip()?;
                    skipper.shift()?;
                    State::Newline
                },
                (State::Text, _) => {
                    skipper.shift()?;
                    State::Text
                },
                (State::Newline, b' ') => {
                    skipper.shift()?;
                    skipper.end_skip()?;
                    State::Text
                },
                (State::Newline, b'\n') => {
                    skipper.cancel_skip()?;
                    skipper.shift()?;
                    State::Newline
                }
                (State::Newline, _) => {
                    skipper.cancel_skip()?;
                    skipper.shift()?;
                    State::Text
                }
            }
        }
        self.skipstate = skipper.save_state();
        return Ok(buf.len())
    }

    fn loc_flush(&mut self, loc: Loc) -> Result<()> {
        self.skipstate.write_remainder(&mut self.inner)?;
        self.inner.loc_flush(loc)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::loc::{ LocWriteWrapper, WriteLocWrapper };
    use crate::loc::test::LocWrites;
    use std::io::{ Write, BufWriter };


    #[test]
    pub fn test_a() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::default(), b"foo\n bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_b() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::default(), b"foo\n ")?;
        unfolder.loc_write(Loc::default(), b"bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_c() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::default(), b"foo\n")?;
        unfolder.loc_write(Loc::default(), b" bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_d() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::default(), b"foo")?;
        unfolder.loc_write(Loc::default(), b"\n bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_e() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::default(), b"foo\n")?;
        unfolder.loc_write(Loc::default(), b"bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foo\nbar");
        Ok(())
    }

    #[test]
    pub fn test_f() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::default(), b"foo\n")?;
        unfolder.loc_write(Loc::default(), b"\nbar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foo\n\nbar");
        Ok(())
    }

    #[test]
    pub fn test_g() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::default(), b"a\n b\n")?;
        unfolder.loc_write(Loc::default(), b" c")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "abc");
        Ok(())
    }

    #[test]
    pub fn test_h() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::default(), b"a\n")?;
        unfolder.loc_write(Loc::default(), b"")?;
        unfolder.loc_write(Loc::default(), b" b")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "ab");
        Ok(())
    }

    #[test]
    pub fn test_i() -> Result<()> {
        // It probably does not have a big significance if it works in this way
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::default(), b"\n foo")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foo");
        Ok(())
    }

    #[test]
    pub fn test_loc() -> Result<()> {
        let mut writes = LocWrites::new();
        let mut unfolder = Unfolder::new(&mut writes);
        unfolder.loc_write(Loc::default(), b"a\n b")?;
        assert_eq!(writes[0], ( Loc{ line: 1, column: 1, offset: 0 }, String::from("a")));
        assert_eq!(writes[1], ( Loc{ line: 2, column: 2, offset: 3 }, String::from("b")));
        Ok(())
    }

    #[test]
    pub fn test_flush() -> Result<()> {
        let mut buf = Vec::new();
        let mut bufwriter = BufWriter::with_capacity(256, &mut buf);
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut bufwriter));
        let mut writer = WriteLocWrapper::new(&mut unfolder);
        writer.write(b"foo\n")?;
        writer.flush()?;
        (_, _) = bufwriter.into_parts(); // drop but dont flush
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foo\n");
        Ok(())
    }
}
