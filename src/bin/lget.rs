use std::io::{stdin, stdout, copy, Write};
use ltools::unfold::Unfolder;
use ltools::crstrip::CrStripper;
use ltools::lexer::{ Lexer, Event, ReceiveEvent };
use ltools::base64::{ DecodeState, DecodeWriter };
use ltools::loc::{
    LocWriteWrapper,
    WriteLocWrapper,
};
use clap::{ command, arg, Arg };

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
    delimiter: u8,
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
            delimiter: b'\n',
        }
    }

    fn set_delimiter(&mut self, delimiter: u8) -> &mut Self {
        self.delimiter = delimiter;
        self
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
                    self.valuetype = ValueType::Base64;
                }
            },
            Event::ValueFinish => {
                if self.ismatch {
                    if self.valuetype == ValueType::Base64 {
                        // TODO: consider raising an error if it isn't in a valid end state
                        self.b64state = DecodeState::new();
                    }
                    self.dest.write(&[self.delimiter]).unwrap(); // todo
                    self.dest.flush().unwrap();
                }
                self.ismatch = true;
                self.attrtypepos = 0;
            }
        }
    }
}

fn parse_arguments() -> Result<(String, u8), &'static str> {
    let mut delimiter = b'\n';

    let matches = command!("lget")
        .disable_colored_help(true)
        .arg(arg!(<ATTRIBUTE> "The attribute type name to get values of."))
        .arg(Arg::new("null-delimit")
             .short('0').long("null-delimit")
             .action(clap::ArgAction::SetTrue)
             .help("Terminate output values with null bytes (0x00) instead of newlines."))
        .get_matches();

    if matches.get_flag("null-delimit") {
        delimiter = 0x00;
    }

    if let Some(attrtype) = matches.get_one::<String>("ATTRIBUTE") {
        Ok((attrtype.to_string(), delimiter))
    } else {
        // shouldn't happen when the argument is required
        Err("missing attribute type name on command line")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (attrtype, delimiter) = parse_arguments()?;
    let mut event_receiver = EventReceiver::new(attrtype, stdout());
    event_receiver.set_delimiter(delimiter);
    let lexer = Lexer::new(event_receiver);
    let unfolder = Unfolder::new(LocWriteWrapper::new(lexer));
    let mut crstripper = CrStripper::new(WriteLocWrapper::new(unfolder));
    copy(&mut stdin(), &mut crstripper)?;
    Ok(())
}
