use std::io::Result;
use crate::loc::{ Loc, LocWrite };
use crate::skip::{ Skipper, SkipState };

#[derive(PartialEq)]
enum State {
    LineStart,
    Text,
    NewlineAfterText,
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
            state: State::LineStart,
            skipstate: SkipState::new(),
        }
    }

}

impl<LW: LocWrite> LocWrite for Unfolder<LW> {
    fn loc_write(&mut self, loc: Loc, buf: &[u8]) -> Result<usize> {
        let mut skipper = Skipper::<&mut LW>::new_with_state(&mut self.inner, loc, buf, self.skipstate);
        loop {
            let c = match skipper.lookahead() {
                None => break,
                Some(c) => c,
            };
            self.state = match self.state {
                State::LineStart => match c {
                    b'\n' => { skipper.shift()?; State::LineStart },
                    _ => { skipper.shift()?; State::Text },
                },
                State::Text => match c {
                    b'\n' => { skipper.begin_skip()?; skipper.shift()?; State::NewlineAfterText },
                    _ => { skipper.shift()?; State::Text },
                },
                State::NewlineAfterText => {
                    if c == b' ' {
                        skipper.shift()?;
                        skipper.end_skip()?;
                    } else {
                        skipper.cancel_skip()?;
                    }
                    State::Text
                },
            };
        }
        self.skipstate = skipper.save_state();
        Ok(buf.len())
    }

    fn loc_flush(&mut self, loc: Loc) -> Result<()> {
        self.skipstate.write_remainder(&mut self.inner)?;
        self.inner.loc_flush(loc)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::loc::LocWriteWrapper;

    #[test]
    pub fn test_a() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::new(), b"foo\n bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_b() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::new(), b"foo\n ")?;
        unfolder.loc_write(Loc::new(), b"bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_c() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::new(), b"foo\n")?;
        unfolder.loc_write(Loc::new(), b" bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_d() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::new(), b"foo")?;
        unfolder.loc_write(Loc::new(), b"\n bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foobar");
        Ok(())
    }

    #[test]
    pub fn test_e() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::new(), b"foo\n")?;
        unfolder.loc_write(Loc::new(), b"bar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foo\nbar");
        Ok(())
    }

    #[test]
    pub fn test_f() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::new(), b"foo\n")?;
        unfolder.loc_write(Loc::new(), b"\nbar")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "foo\n\nbar");
        Ok(())
    }

    #[test]
    pub fn test_g() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::new(), b"a\n b\n")?;
        unfolder.loc_write(Loc::new(), b" c")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "abc");
        Ok(())
    }

    #[test]
    pub fn test_h() -> Result<()> {
        let mut buf = Vec::new();
        let mut unfolder = Unfolder::new(LocWriteWrapper::new(&mut buf));
        unfolder.loc_write(Loc::new(), b"a\n")?;
        unfolder.loc_write(Loc::new(), b"")?;
        unfolder.loc_write(Loc::new(), b" b")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "ab");
        Ok(())
    }

    use crate::loc::test::LocWrites;

    #[test]
    pub fn test_loc_a() -> Result<()> {
        let mut writes = LocWrites::new();
        let mut unfolder = Unfolder::new(&mut writes);
        unfolder.loc_write(Loc::new(), b"a\n b")?;
        assert_eq!(writes[0], ( Loc{ line: 1, column: 1, offset: 0 }, String::from("a")));
        assert_eq!(writes[1], ( Loc{ line: 2, column: 2, offset: 3 }, String::from("b")));
        Ok(())
    }
}
