use clap::{command, Arg};
use std::io::Write;
use std::matches;

fn parse_arguments() -> Result<bool, &'static str> {
    let mut reverse_escaping = false;

    let matches = command!("lescape")
        .disable_colored_help(true)
        .arg(
            Arg::new("reverse")
                .short('r')
                .long("reverse")
                .action(clap::ArgAction::SetTrue)
                .help("Reverse the escaping."),
        )
        .get_matches();

    if matches.get_flag("reverse") {
        reverse_escaping = true;
    }

    Ok(reverse_escaping)
}

fn lescape<W: Write>(mut dest: W, buf: &[u8]) -> std::io::Result<usize> {
    let mut written = 0;
    for (i, c) in buf.iter().copied().enumerate() {
        if !c.is_ascii() || matches!(c, b'\\' | b'*' | b'(' | b')' | b':' | b'\0') {
            if i > written {
                dest.write(&buf[written..i])?;
            }
            write!(dest, "\\{c:02x}")?;
            written = i + 1;
        }
    }
    if written != buf.len() {
        dest.write(&buf[written..])?;
    }
    Ok(buf.len())
}

struct LEscaper<W: Write> {
    dest: W,
}

impl<W: Write> LEscaper<W> {
    fn new(dest: W) -> LEscaper<W> {
        LEscaper{
            dest,
        }
    }
}

struct LUnescaper<W: Write> {
    dest: W,
    state: LUnescaperState,
}

#[derive(Eq, PartialEq, Copy, Clone)]
enum LUnescaperState {
    Normal,
    Backslash, // after backslash
    FirstDigit(u8), // after first hex digit
}

impl<W: Write> LUnescaper<W> {
    fn new(dest: W) -> LUnescaper<W> {
        LUnescaper{
            dest,
            state: LUnescaperState::Normal,
        }
    }
}

fn hexdigit_to_lower_bits(digit: u8) -> u8 {
    match digit.to_ascii_lowercase() {
        b'0' => 0,
        b'1' => 1,
        b'2' => 2,
        b'3' => 3,
        b'4' => 4,
        b'5' => 5,
        b'6' => 6,
        b'7' => 7,
        b'8' => 8,
        b'9' => 9,
        b'a' => 10,
        b'b' => 11,
        b'c' => 12,
        b'd' => 13,
        b'e' => 14,
        b'f' => 15,
        _ => panic!(),
    }
}

impl<W: Write> Write for LUnescaper<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut written = 0;
        for (i, c) in buf.iter().copied().enumerate() {
            self.state = match (self.state, c) {
                (LUnescaperState::Normal, b'\\') => {
                    if written < buf.len() {
                        self.dest.write(&buf[written..i])?;
                        written = i;
                    }
                    LUnescaperState::Backslash
                },
                (LUnescaperState::Normal, _) => LUnescaperState::Normal,
                (LUnescaperState::Backslash, d1) => {
                    if d1.is_ascii_hexdigit() {
                        LUnescaperState::FirstDigit(d1)
                    } else {
                        return std::io::Result::Err(
                            std::io::Error::new(std::io::ErrorKind::Other,
                                                format!("invalid hexadecimal digit 0x{d1:02x}")));
                    }
                },
                (LUnescaperState::FirstDigit(d1), d2) => {
                    if !d2.is_ascii_hexdigit() {
                        return std::io::Result::Err(
                            std::io::Error::new(std::io::ErrorKind::Other,
                                                format!("invalid hexadecimal digit 0x{d2:02x}")));
                    }
                    let byte = (hexdigit_to_lower_bits(d1) << 4) | hexdigit_to_lower_bits(d2);
                    self.dest.write(&[byte])?;
                    written = i + 1;
                    LUnescaperState::Normal
                },
            }
        }
        if self.state == LUnescaperState::Normal {
            if written < buf.len() {
                self.dest.write(&buf[written..])?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.dest.flush()
    }
}

impl<W: Write> Write for LEscaper<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        lescape(&mut self.dest, buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.dest.flush()
    }
}

fn get_result() -> Result<(), Box<dyn std::error::Error>> {
    let reverse_escaping = parse_arguments()?;
    if !reverse_escaping {
        let mut lescaper = LEscaper::new(std::io::stdout());
        std::io::copy(&mut std::io::stdin(), &mut lescaper)?;
    } else {
        let mut lunescaper = LUnescaper::new(std::io::stdout());
        std::io::copy(&mut std::io::stdin(), &mut lunescaper)?;
    }
    Ok(())
}

fn main() {
    let result = get_result();
    if let Err(err) = result {
        eprintln!("lescape: {}", err);
        std::process::exit(1);
    }
}
