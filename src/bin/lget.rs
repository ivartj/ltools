use std::io::{stdin, stdout, copy, Write};
use ltools::unfold::Unfolder;
use ltools::crstrip::CrStripper;
use ltools::lexer::{ Lexer, Token, TokenKind, ReceiveToken };
use ltools::base64::{ DecodeState, DecodeWriter };
use ltools::loc::WriteLocWrapper;
use clap::{ command, arg, Arg };
use ltools::cartesian::cartesian_product;

#[derive(PartialEq)]
enum ValueType {
    Text,
    Base64,
}

struct TokenReceiver<W: Write> {
    attrtype: String,
    ismatch: bool,
    dest: W,
    valuetype: ValueType,
    b64state: DecodeState,
    delimiter: u8,
}

impl<W: Write> TokenReceiver<W> {
    fn new(attrtype: String, dest: W) -> TokenReceiver<W> {
        TokenReceiver{
            attrtype,
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

impl<W: Write> ReceiveToken for TokenReceiver<W> {
    fn receive_token(&mut self, token: Token) {
        match token.kind {
            TokenKind::AttributeType => {
                self.ismatch = token.segment.to_ascii_lowercase() == self.attrtype.to_ascii_lowercase();
            },
            TokenKind::ValueText => {
                if self.ismatch {
                    self.dest.write(token.segment.as_bytes()).unwrap(); // todo
                    self.valuetype = ValueType::Text;
                }
            },
            TokenKind::ValueBase64 => {
                if self.ismatch {
                    let mut decoder = DecodeWriter::new_with_state(&mut self.dest, self.b64state);
                    decoder.write(token.segment.as_bytes()).unwrap(); // todo
                    self.b64state = decoder.get_state();
                    self.valuetype = ValueType::Base64;
                }
            },
            TokenKind::ValueFinish => {
                if self.ismatch {
                    if self.valuetype == ValueType::Base64 {
                        // TODO: consider raising an error if it isn't in a valid end state
                        self.b64state = DecodeState::new();
                    }
                    self.dest.write(&[self.delimiter]).unwrap(); // todo
                    self.dest.flush().unwrap();
                }
                self.ismatch = true;
            }
            TokenKind::EmptyLine => {},
        }
    }
}

struct TsvTokenReceiver<W: Write> {
    attributes: Vec<String>,
    entryvalues: Vec<Vec<Vec<u8>>>,
    attrmatch: Option<usize>, // index of currently matched attribute
    valuebuf: Vec<u8>,
    dest: W,
    valuetype: ValueType,
    b64state: DecodeState,
}

impl<W: Write> TsvTokenReceiver<W> {
    fn new(attributes: Vec<String>, dest: W) -> TsvTokenReceiver<W> {
        let entryvalues = attributes.iter().map(|_| Vec::new()).collect();
        TsvTokenReceiver{
            attributes,
            entryvalues,
            attrmatch: None,
            valuebuf: Vec::new(),
            dest,
            valuetype: ValueType::Text,
            b64state: DecodeState::new(),
        }
    }
}

impl<W: Write> ReceiveToken for TsvTokenReceiver<W> {
    fn receive_token(&mut self, token: Token) {
        match token.kind {
            TokenKind::AttributeType => {
                let attrlowercase = token.segment.to_ascii_lowercase();
                self.attrmatch = self.attributes.iter()
                    .position(|attr| attr.to_ascii_lowercase() == attrlowercase);
            },
            TokenKind::ValueText => {
                if self.attrmatch.is_some() {
                    self.valuebuf.write(token.segment.as_bytes()).unwrap(); // todo
                    self.valuetype = ValueType::Text;
                }
            },
            TokenKind::ValueBase64 => {
                if self.attrmatch.is_some() {
                    let mut decoder = DecodeWriter::new_with_state(&mut self.valuebuf, self.b64state);
                    decoder.write(token.segment.as_bytes()).unwrap(); // todo
                    self.b64state = decoder.get_state();
                    self.valuetype = ValueType::Base64;
                }
            },
            TokenKind::ValueFinish => {
                if let Some(attridx) = self.attrmatch {
                    if self.valuetype == ValueType::Base64 {
                        // TODO: consider raising an error if it isn't in a valid end state
                        self.b64state = DecodeState::new();
                    }
                    self.entryvalues[attridx].push(self.valuebuf.clone());
                    self.valuebuf.clear();
                }
            }
            TokenKind::EmptyLine => {
                for record in cartesian_product(&self.entryvalues) {
                    for (i, value) in record.iter().enumerate() {
                        if i != 0 {
                            self.dest.write(b"\t").unwrap(); // todo
                        }
                        self.dest.write(value).unwrap(); // todo
                    }
                    self.dest.write(b"\n").unwrap(); // todo
                }
                for v in self.entryvalues.iter_mut() {
                    v.clear();
                }
            },
        }
    }
}

fn parse_arguments() -> Result<(Vec<String>, u8), &'static str> {
    let mut delimiter = b'\n';

    let matches = command!("lget")
        .disable_colored_help(true)
        .arg(arg!(<ATTRIBUTES> ... "The attribute type name to get values of."))
        .arg(Arg::new("null-delimit")
             .short('0').long("null-delimit")
             .action(clap::ArgAction::SetTrue)
             .help("Terminate output values with null bytes (0x00) instead of newlines."))
        .get_matches();

    if matches.get_flag("null-delimit") {
        delimiter = 0x00;
    }

    if let Some(attrtype) = matches.get_many::<String>("ATTRIBUTES") {
        Ok((attrtype.cloned().collect(), delimiter))
    } else {
        // shouldn't happen when the argument is required
        Err("missing attribute type name on command line")
    }
}

fn receive_tokens<TR: ReceiveToken>(tr: TR) -> std::io::Result<()> {
    let lexer = Lexer::new(tr);
    let unfolder = Unfolder::new(lexer);
    let crstripper = CrStripper::new(unfolder);
    let mut wrapper = WriteLocWrapper::new(crstripper);
    copy(&mut stdin(), &mut wrapper)?;
    wrapper.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exit_code = {
        let (attributes, delimiter) = parse_arguments()?;
        let result = if attributes.len() == 1 {
            let mut token_receiver = TokenReceiver::new(attributes[0].clone(), stdout());
            token_receiver.set_delimiter(delimiter);
            receive_tokens(token_receiver)
        } else {
            let token_receiver = TsvTokenReceiver::new(attributes, stdout());
            receive_tokens(token_receiver)
        };
        if let Err(err) = result {
            eprintln!("{}", err);
            1
        } else {
            0
        }
    };
    std::process::exit(exit_code);
}
