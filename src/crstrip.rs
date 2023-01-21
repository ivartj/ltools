use std::io::Result;
use crate::loc::{ Loc, LocWrite };
use crate::skip::{ Skipper, SkipState };

#[derive(Copy, Clone, PartialEq)]
enum State {
    Normal,
    Cr,
}

pub struct CrStripper<LW: LocWrite> {
    inner: LW,
    state: State,
    skipstate: SkipState,
}

impl<LW: LocWrite> CrStripper<LW> {
    pub fn new(inner: LW) -> CrStripper<LW> {
        CrStripper{ inner, state: State::Normal, skipstate: SkipState::default() }
    }
}

impl<LW: LocWrite> LocWrite for CrStripper<LW> {
    fn loc_write(&mut self, loc: Loc, buf: &[u8]) -> Result<usize> {
        let mut skipper = Skipper::new_with_state(&mut self.inner, loc, buf, self.skipstate);
        while let Some(c) = skipper.lookahead() {
            self.state = match (self.state, c) {
                (State::Normal, b'\r') => {
                    skipper.begin_skip()?;
                    skipper.shift()?;
                    State::Cr
                },
                (State::Normal, _) => {
                    skipper.shift()?;
                    State::Normal
                },
                (State::Cr, b'\r') => {
                    skipper.cancel_skip()?;
                    skipper.begin_skip()?;
                    skipper.shift()?;
                    State::Cr
                },
                (State::Cr, b'\n') => {
                    skipper.end_skip()?;
                    skipper.shift()?;
                    State::Normal
                },
                (State::Cr, _) => {
                    skipper.cancel_skip()?;
                    skipper.shift()?;
                    State::Normal
                },
            };
        }
        self.skipstate = skipper.save_state();
        Ok(buf.len())
    }

    fn loc_flush(&mut self, loc: Loc) -> Result<()> {
        self.skipstate.write_remainder(&mut self.inner)?;
        self.inner.loc_flush(loc)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::loc::LocWriteWrapper;

    #[test]
    pub fn test_a() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(LocWriteWrapper::new(&mut buf));
        crstripper.loc_write(Loc::default(), b"foo\r\nbar")?;
        assert_eq!(String::from_utf8_lossy(buf.as_slice()), "foo\nbar");
        Ok(())
    }

    #[test]
    pub fn test_b() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(LocWriteWrapper::new(&mut buf));
        crstripper.loc_write(Loc::default(), b"foo\r")?;
        crstripper.loc_write(Loc::default(), b"\nbar")?;
        assert_eq!(String::from_utf8_lossy(buf.as_slice()), "foo\nbar");
        Ok(())
    }

    #[test]
    pub fn test_c() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(LocWriteWrapper::new(&mut buf));
        crstripper.loc_write(Loc::default(), b"foo\r\r\nbar")?;
        assert_eq!(String::from_utf8_lossy(buf.as_slice()), "foo\r\nbar");
        Ok(())
    }

    #[test]
    pub fn test_d() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(LocWriteWrapper::new(&mut buf));
        crstripper.loc_write(Loc::default(), b"foo\r\rbar")?;
        assert_eq!(String::from_utf8_lossy(buf.as_slice()), "foo\r\rbar");
        Ok(())
    }

    #[test]
    pub fn test_e() -> Result<()> {
        let mut buf = Vec::new();
        let mut crstripper = CrStripper::new(LocWriteWrapper::new(&mut buf));
        crstripper.loc_write(Loc::default(), b"a\r")?;
        crstripper.loc_write(Loc::default(), b"")?;
        crstripper.loc_write(Loc::default(), b"\nb")?;
        assert_eq!(String::from_utf8_lossy(&buf[..]), "a\nb");
        Ok(())
    }
}

