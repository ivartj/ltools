use std::io::{ Result, Write };

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Loc {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Loc {
    pub fn new() -> Loc {
        Loc{
            line: 1,
            column: 1,
            offset: 0,
        }
    }

    pub fn after(self, c: u8) -> Self {
        match c {
            b'\n' => Loc {
                line: self.line + 1,
                column: 1,
                offset: self.offset + 1,
            },
            _ => Loc {
                line: self.line,
                column: self.column + 1,
                offset: self.offset + 1,
            },
        }
    }
}

pub trait LocWrite {
    fn loc_write(&mut self, loc: Loc, buf: &[u8]) -> Result<usize>;
    fn loc_flush(&mut self, loc: Loc) -> Result<()>;
}

pub struct LocWriteWrapper<W: Write> {
    inner: W,
}

impl<W: Write> LocWriteWrapper<W> {
    pub fn new(inner: W) -> LocWriteWrapper<W> {
        LocWriteWrapper{
            inner
        }
    }
}

impl<W: Write> LocWrite for LocWriteWrapper<W> {
    fn loc_write(&mut self, _: Loc, buf: &[u8]) -> Result<usize> {
        self.inner.write(buf)
    }

    fn loc_flush(&mut self, _: Loc) -> Result<()> {
        self.inner.flush()
    }
}

pub struct WriteLocWrapper<LW: LocWrite> {
    inner: LW,
    loc: Loc,
}

impl<LW: LocWrite> WriteLocWrapper<LW> {
    pub fn new(inner: LW) -> WriteLocWrapper<LW> {
        WriteLocWrapper{
            inner,
            loc: Loc::new(),
        }
    }
}

impl<LW: LocWrite> Write for WriteLocWrapper<LW> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.loc_write(self.loc, buf)?;
        self.loc = buf.iter().copied().fold(self.loc, |loc, c| loc.after(c));
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.loc_flush(self.loc)
    }
}

impl<LW: LocWrite> LocWrite for &mut LW {
    fn loc_write(&mut self, loc: Loc, buf: &[u8]) -> Result<usize> {
        (**self).loc_write(loc, buf)
    }

    fn loc_flush(&mut self, loc: Loc) -> Result<()> {
        (**self).loc_flush(loc)
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use core::ops::Deref;

    /* Utility for testing */
    /* we define it this way instead of `type LocWrites = Vec<(Loc, String)>` because in the current
     * version of Rust, the compiler claims that the Write implementation for Vec<u8> causes a
     * conflict */
    pub struct LocWrites {
        vec: Vec<(Loc, String)>,
    }

    impl LocWrites {
        pub fn new() -> LocWrites {
            LocWrites{ vec: Vec::new() }
        }
    }

    impl Deref for LocWrites {
        type Target = [(Loc, String)];

        fn deref(&self) -> &Self::Target {
            self.vec.as_ref()
        }
    }

    impl LocWrite for &mut LocWrites {
        fn loc_write(&mut self, loc: Loc, buf: &[u8]) -> Result<usize> {
            self.vec.push((loc, String::from_utf8_lossy(buf).to_string()));
            Ok(buf.len())
        }

        fn loc_flush(&mut self, _: Loc) -> Result<()> {
            Ok(())
        }
    }
}

