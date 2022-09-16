use std::io::{stdin, stdout, Result, copy, Write, BufWriter};
use ltools::unfold::Unfolder;
use ltools::crstrip::CrStripper;
use ltools::lexer::{ Lexer, Event, ReceiveEvent };
use ltools::base64::{ DecodeState, DecodeWriter };

#[derive(PartialEq)]
enum ValueType {
    None,
    Text,
    Base64,
}

struct EventReceiver<W: Write> {
    attrtype: String,
    attrtypepos: usize,
    ismatch: bool,
    dest: W,
    valuetype: ValueType,
    b64state: DecodeState,
}

impl<W: Write> EventReceiver<W> {
    fn new(attrtype: String, dest: W) -> EventReceiver<W> {
        EventReceiver{
            attrtype,
            attrtypepos: 0,
            ismatch: true, // true until non-matching char is seen
            dest,
            valuetype: ValueType::Text,
            b64state: DecodeState::new(),
        }
    }
}

impl<W: Write> Drop for EventReceiver<W> {
    fn drop(&mut self) {
        self.dest.flush().unwrap();
    }
}

impl<W: Write> ReceiveEvent for EventReceiver<W> {
    fn receive_event(&mut self, event: Event) {
        match event {
            Event::TypeChar(c) => {
                if self.ismatch {
                    self.ismatch = self.attrtypepos < self.attrtype.len()
                        && self.attrtype.as_bytes()[self.attrtypepos].to_ascii_lowercase() == (c as u8).to_ascii_lowercase();
                    self.attrtypepos += 1;
                }
            },
            Event::TypeFinish => {
                if self.ismatch {
                    if self.attrtypepos != self.attrtype.len() {
                        self.ismatch = false;
                    }
                    self.valuetype = ValueType::None
                }
            },
            Event::ValueText(text) => {
                if self.ismatch {
                    self.dest.write(text.as_bytes()).unwrap(); // todo
                    self.valuetype = ValueType::Text;
                }
            },
            Event::ValueBase64(code) => {
                if self.ismatch {
                    let mut decoder = DecodeWriter::new_with_state(&mut self.dest, self.b64state);
                    decoder.write(code.as_bytes()).unwrap(); // todo
                    self.b64state = decoder.get_state();
                }
            },
            Event::ValueFinish => {
                if self.ismatch {
                    if self.valuetype == ValueType::Base64 {
                        // TODO: consider raising an error if it isn't in a valid end state
                        self.b64state = DecodeState::new();
                    }
                    self.dest.write(b"\n").unwrap(); // todo
                }
                self.ismatch = true;
                self.attrtypepos = 0;
            }
        }
    }
}

fn main() -> Result<()> {
    let attrtype = std::env::args().nth(1).unwrap();
    let bufwriter = BufWriter::new(stdout());
    let event_receiver = EventReceiver::new(attrtype, bufwriter);
    let lexer = Lexer::new(event_receiver);
    let unfolder = Unfolder::new(lexer);
    let mut crstripper = CrStripper::new(unfolder);
    copy(&mut stdin(), &mut crstripper)?;
    Ok(())
}
