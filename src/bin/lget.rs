use std::io::{stdin, stdout, copy, Write};
use ltools::unfold::Unfolder;
use ltools::crstrip::CrStripper;
use ltools::lexer::{ Lexer, Token, TokenKind, ReceiveToken };
use ltools::base64::{ DecodeState, DecodeWriter };
use ltools::loc::WriteLocWrapper;
use clap::{ command, arg, Arg };

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
    let exit_code = {
        let (attrtype, delimiter) = parse_arguments()?;
        let mut token_receiver = TokenReceiver::new(attrtype, stdout());
        token_receiver.set_delimiter(delimiter);
        let lexer = Lexer::new(token_receiver);
        let unfolder = Unfolder::new(lexer);
        let crstripper = CrStripper::new(unfolder);
        let result = copy(&mut stdin(), &mut WriteLocWrapper::new(crstripper));
        if let Err(err) = result {
            eprintln!("{}", err);
            1
        } else {
            0
        }
    };
    std::process::exit(exit_code);
}
