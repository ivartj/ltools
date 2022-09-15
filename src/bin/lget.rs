use std::io::{stdin, stdout, Result, copy, Write, BufWriter};
use ltools::unfold::Unfolder;
use ltools::lexer::{ Lexer, Event, ReceiveEvent };

struct EventReceiver<'a, W: Write> {
    attrtype: String,
    attrtypepos: usize,
    ismatch: bool,
    dest: &'a mut W,
}

impl<'a, W: Write> EventReceiver<'a, W> {
    fn new(attrtype: String, dest: &'a mut W) -> EventReceiver<'a, W> {
        EventReceiver{
            attrtype,
            attrtypepos: 0,
            ismatch: true, // true until non-matching char is seen
            dest,
        }
    }
}

impl<'a, W: Write> ReceiveEvent for EventReceiver<'a, W> {
    fn receive_event(&mut self, event: Event) {
        match event {
            Event::TypeChar(c) => {
                if self.ismatch {
                    self.ismatch = self.attrtypepos < self.attrtype.len()
                        && self.attrtype.as_bytes().get(self.attrtypepos).unwrap() == &(c as u8);
                    self.attrtypepos += 1;
                }
            },
            Event::TypeFinish => (),
            Event::ValueText(text) => {
                if self.ismatch {
                    self.dest.write(text.as_bytes()).unwrap(); // todo
                }
            },
            Event::ValueFinish => {
                if self.ismatch {
                    self.dest.write(b"\n").unwrap(); // todo
                }
                self.ismatch = true;
                self.attrtypepos = 0;
            }
            _ => todo!(),
        }
    }
}

fn main() -> Result<()> {
    let attrtype = std::env::args().nth(1).unwrap();
    let mut bufwriter = BufWriter::new(stdout());
    let mut event_receiver = EventReceiver::new(attrtype, &mut bufwriter);
    let mut lexer = Lexer::new(&mut event_receiver);
    let mut unfolder = Unfolder::new(&mut lexer);
    copy(&mut stdin(), &mut unfolder)?;
    bufwriter.flush()?;
    Ok(())
}
