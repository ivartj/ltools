use std::io::{
    Result,
    Error,
    ErrorKind,
};
use crate::loc::{ Loc, LocWrite };

const MAX_PREFIX: usize = 4;

#[derive(Debug, PartialEq)]
pub enum SkipToken {
    Byte(u8),
    End,
}

pub struct Skipper<'a, LW: LocWrite>  {
    inner: LW,
    loc: Loc,
    buf: &'a [u8],
    pos: usize,
    write_from: usize,
    write_from_loc: Loc,
    state: SkipState,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SkipState {
    Writing,
    SkippingFrom(Loc, usize),
    SkippingWithPrefix(Loc, [u8;MAX_PREFIX], usize),
}

impl SkipState {
    pub fn new() -> SkipState { SkipState::Writing }

    pub fn write_remainder<LW: LocWrite>(&self, dest: &mut LW) -> Result<()> {
        if let SkipState::SkippingWithPrefix(loc, prefixbuf, prefixlen) = *self {
            dest.loc_write(loc, &prefixbuf[..prefixlen])?;
        }
        Ok(())
    }
}

impl<'a, LW: LocWrite> Skipper<'a, LW> {
    pub fn new(inner: LW, loc: Loc, buf: &'a [u8]) -> Skipper<'a, LW> {
        Skipper{
            inner,
            loc,
            buf,
            pos: 0,
            write_from: 0,
            write_from_loc: loc,
            state: SkipState::Writing,
        }
    }

    pub fn new_with_state(inner: LW, loc: Loc, buf: &'a [u8], state: SkipState) -> Skipper<'a, LW> {
        Skipper{
            inner,
            loc,
            buf,
            pos: 0,
            write_from: 0,
            write_from_loc: loc,
            state,
        }
    }

    pub fn lookahead(&self) -> Option<u8> {
        if self.pos < self.buf.len() {
            Some(self.buf[self.pos])
        } else {
            None
        }
    }

    pub fn shift(&mut self) -> Result<Option<u8>> {
        let lookahead = match self.lookahead() {
            None => {
                return Err(Error::new(ErrorKind::Other, "call to .shift() after reaching end of buffer"));
            },
            Some(c) => c,
        };
        match self.state {
            SkipState::SkippingFrom(_, offset) => if self.pos + 1 - offset > MAX_PREFIX {
                return Err(Error::new(ErrorKind::Other, "skipped data exceeds maximum"));
            },
            SkipState::SkippingWithPrefix(_, _, prefix_length) => if prefix_length + self.pos + 1 > MAX_PREFIX {
                return Err(Error::new(ErrorKind::Other, "skipped data exceeds maximum"));
            },
            SkipState::Writing => (),
        }
        self.loc = self.loc.after(lookahead);
        self.pos += 1;
        if self.lookahead() == None {
            match self.state {
                SkipState::Writing => {
                    self.inner.loc_write(self.write_from_loc, &self.buf[self.write_from..])?;
                },
                SkipState::SkippingFrom(_, write_until) => {
                    self.inner.loc_write(self.write_from_loc, &self.buf[self.write_from..write_until])?;
                },
                SkipState::SkippingWithPrefix(..) => {},
            }
        }
        Ok(self.lookahead())
    }

    pub fn begin_skip(&mut self) -> Result<()> {
        if self.state != SkipState::Writing {
            return Err(Error::new(ErrorKind::Other, "call to .begin_skip() in state {:?}"));
        }
        self.state = SkipState::SkippingFrom(self.loc, self.pos);
        Ok(())
    }

    pub fn end_skip(&mut self) -> Result<()> {
        match self.state {
            SkipState::SkippingFrom(_, write_until) => {
                if self.lookahead() != None {
                    self.inner.loc_write(self.write_from_loc, &self.buf[self.write_from..write_until])?;
                }
            },
            SkipState::SkippingWithPrefix(..) => {},
            SkipState::Writing => return Err(Error::new(ErrorKind::Other, "call to .end_skip() while not skipping")),
        }
        self.write_from_loc = self.loc;
        self.write_from = self.pos;
        self.state = SkipState::Writing;
        Ok(())
    }

    pub fn cancel_skip(&mut self) -> Result<()> {
        if let SkipState::SkippingWithPrefix(loc, prefix_array, prefix_len) = self.state {
            self.inner.loc_write(loc, &prefix_array[..prefix_len])?;
        }
        self.state = SkipState::Writing;
        Ok(())
    }

    pub fn save_state(self) -> SkipState {
        match self.state {
            SkipState::SkippingFrom(loc, offset) => {
                let mut prefix_array = [0u8; 4];
                let prefix_slice = &self.buf[offset..];
                for (i, c) in prefix_slice.iter().copied().enumerate() {
                    prefix_array[i] = c;
                }
                SkipState::SkippingWithPrefix(loc, prefix_array, prefix_slice.len())
            },
            SkipState::SkippingWithPrefix(loc, prefix_array, prefix_len) => {
                let mut new_prefix_array = [0u8; 4];
                for (i,c) in prefix_array[..prefix_len].iter().chain(self.buf.iter()).copied().enumerate() {
                    new_prefix_array[i] = c;
                }
                SkipState::SkippingWithPrefix(loc, new_prefix_array, prefix_len + self.buf.len())
            },
            SkipState::Writing => self.state,
        }
    }

}

#[cfg(test)]
mod test {
    use super::*;
    use crate::loc::{
        Loc,
        test::LocWrites,
    };

    #[test]
    fn test_a() -> Result<()> {
        let mut writes = LocWrites::new();
        let mut skipper = Skipper::new(&mut writes, Loc::new(), b"abcd");
        skipper.shift()?;
        skipper.begin_skip()?;
        skipper.shift()?;
        skipper.end_skip()?;
        skipper.shift()?;
        skipper.shift()?;
        assert_eq!(skipper.lookahead(), None);
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].1, "a");
        assert_eq!(writes[1].1, "cd");
        Ok(())
    }

    #[test]
    fn test_b() -> Result<()> {
        let mut writes = LocWrites::new();
        let mut skipper = Skipper::new(&mut writes, Loc::new(), b"a\n");
        skipper.shift()?;
        skipper.begin_skip()?;
        skipper.shift()?;
        assert_eq!(skipper.lookahead(), None);
        let saved_state = skipper.save_state();
        let loc = b"a\n".iter().copied().fold(Loc::new(), |l, c| l.after(c));
        skipper = Skipper::new_with_state(&mut writes, loc, b" d", saved_state);
        skipper.shift()?;
        skipper.end_skip()?;
        skipper.shift()?;
        assert_eq!(skipper.lookahead(), None);
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].1, "a");
        assert_eq!(writes[0].0, Loc{ offset: 0, line: 1, column: 1});
        assert_eq!(writes[1].1, "d");
        assert_eq!(writes[1].0, Loc{ offset: 3, line: 2, column: 2});
        Ok(())
    }

    #[test]
    fn test_c() -> Result<()> {
        let mut writes = LocWrites::new();
        let mut skipper = Skipper::new(&mut writes, Loc::new(), b"abc");
        skipper.shift()?;
        skipper.begin_skip()?;
        skipper.shift()?;
        skipper.cancel_skip()?;
        skipper.shift()?;
        assert_eq!(skipper.lookahead(), None);
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].1, "abc");
        assert_eq!(writes[0].0, Loc{ offset: 0, line: 1, column: 1});
        Ok(())
    }

    #[test]
    fn test_d() -> Result<()> {
        let mut writes = LocWrites::new();
        let mut skipper = Skipper::new(&mut writes, Loc::new(), b"ab");
        skipper.shift()?;
        skipper.begin_skip()?;
        skipper.shift()?;
        assert_eq!(skipper.lookahead(), None);
        let saved_state = skipper.save_state();
        let loc = b"ab".iter().copied().fold(Loc::new(), |l, c| l.after(c));
        skipper = Skipper::new_with_state(&mut writes, loc, b"cd", saved_state);
        skipper.shift()?;
        skipper.cancel_skip()?;
        skipper.shift()?;
        assert_eq!(skipper.lookahead(), None);
        assert_eq!(writes.len(), 3);
        assert_eq!(writes[0].1, "a");
        assert_eq!(writes[0].0, Loc{ offset: 0, line: 1, column: 1});
        assert_eq!(writes[1].1, "b");
        assert_eq!(writes[1].0, Loc{ offset: 1, line: 1, column: 2});
        assert_eq!(writes[2].1, "cd");
        assert_eq!(writes[2].0, Loc{ offset: 2, line: 1, column: 3});
        Ok(())
    }
}

