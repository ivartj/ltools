use std::io::{ Write, Result };

enum State {
    S0,
    S2,
    S4,
}

pub struct Encoder {
    state: State,
    remainder: u8,
    buf: Vec<u8>,
}

impl Encoder {
    pub fn new() -> Encoder {
        Encoder{ state: State::S0, remainder: 0u8, buf: Vec::new() }
    }

    pub fn get_buffer<'a>(&'a mut self) -> &'a [u8] {
        self.buf.as_slice()
    }

    pub fn clear_buffer(&mut self) {
        self.buf.clear();
    }
}

const DIGITS: [u8;64] = [
    b'A',
    b'B',
    b'C',
    b'D',
    b'E',
    b'F',
    b'G',
    b'H',
    b'I',
    b'J',
    b'K',
    b'L',
    b'M',
    b'N',
    b'O',
    b'P',
    b'Q',
    b'R',
    b'S',
    b'T',
    b'U',
    b'V',
    b'W',
    b'X',
    b'Y',
    b'Z',
    b'a',
    b'b',
    b'c',
    b'd',
    b'e',
    b'f',
    b'g',
    b'h',
    b'i',
    b'j',
    b'k',
    b'l',
    b'm',
    b'n',
    b'o',
    b'p',
    b'q',
    b'r',
    b's',
    b't',
    b'u',
    b'v',
    b'w',
    b'x',
    b'y',
    b'x',
    b'0',
    b'1',
    b'2',
    b'3',
    b'4',
    b'5',
    b'6',
    b'7',
    b'8',
    b'9',
    b'+',
    b'/',
];

const F2BITS: u8 = 0xc0; // first 2 bits
const F4BITS: u8 = 0xf0; // first 4 bits
const F6BITS: u8 = 0xfc; // first 6 bits
const L2BITS: u8 = !F6BITS; // last 2 bits
const L4BITS: u8 = !F4BITS; // last 4 bits
const L6BITS: u8 = !F2BITS; // last 6 bits

impl Write for Encoder {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        for c in buf.into_iter() {
            match self.state {
                State::S0 => {
                    let dindex = ((c & F6BITS) >> 2) as usize;
                    self.buf.write(&DIGITS[dindex..dindex+1])?;
                    self.remainder = (c & L2BITS) << 4;
                    self.state = State::S2;
                },
                State::S2 => {
                    let dindex = (self.remainder + ((c & F4BITS) >> 4)) as usize;
                    self.buf.write(&DIGITS[dindex..dindex+1])?;
                    self.remainder = (c & L4BITS) << 2;
                    self.state = State::S4;
                }
                State::S4 => {
                    let mut dindex = (self.remainder + ((c & F2BITS) >> 6)) as usize;
                    self.buf.write(&DIGITS[dindex..dindex+1])?;
                    dindex = (c & L6BITS) as usize;
                    self.buf.write(&DIGITS[dindex..dindex+1])?;
                    self.state = State::S0;
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        match self.state {
            State::S0 => (),
            State::S2 => {
                let dindex = self.remainder as usize;
                self.buf.write(&DIGITS[dindex..dindex+1])?;
                self.buf.write(b"==")?;
            },
            State::S4 => {
                let dindex = self.remainder as usize;
                self.buf.write(&DIGITS[dindex..dindex+1])?;
                self.buf.write(b"=")?;
            },
        }
        self.state = State::S0;
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() -> Result<()> {
        let mut encoder = Encoder::new();
        encoder.write(b"Hello world")?;
        encoder.flush()?;
        assert_eq!(unsafe { std::str::from_utf8_unchecked(encoder.buf.as_slice()) }, "SGVsbG8gd29ybGQ=");
        Ok(())
    }
}

