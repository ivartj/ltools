use std::io::{stdin, stdout, copy, Write};
use ltools::unfold::Unfolder;
use ltools::crstrip::CrStripper;
use ltools::lexer::{ Lexer, Token, TokenKind, ReceiveToken };
use ltools::base64::{ DecodeState, DecodeWriter };
use ltools::loc::WriteLocWrapper;
use clap::{ command, arg };

#[derive(PartialEq)]
enum ValueType {
    Text,
    Base64,
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

struct CartesianProduct<'a, E> {
    vec: &'a Vec<Vec<E>>,
    counters: Vec<usize>,
}

fn cartesian_product<'a, E>(vec: &'a Vec<Vec<E>>) -> CartesianProduct<'a, E> {
    CartesianProduct{
        vec,
        counters: vec![0; vec.len()],
    }
}

impl<'a, E> Iterator for CartesianProduct<'a, E> {
    type Item = Vec<&'a E>;

    fn next(&mut self) -> Option<Vec<&'a E>> {
        if self.counters[0] == self.vec[0].len() {
            return None;
        }

        let retval = self.counters.iter()
            .copied()
            .enumerate()
            .map(|(i, counter)| &self.vec[i][counter])
            .collect();

        // increment counters
        for (i, counter) in self.counters.iter_mut().enumerate().rev() {
            *counter += 1;
            if *counter == self.vec[i].len() {
                if i != 0 {
                    *counter = 0;
                }
                continue;
            } else {
                break;
            }
        }

        Some(retval)
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

fn parse_arguments() -> Result<Vec<String>, &'static str> {
    let matches = command!("ldif2tsv")
        .disable_colored_help(true)
        .arg(arg!(<ATTRIBUTES> ... "Attributes to to write in the output tab-separated values"))
        .get_matches();

    if let Some(attributes) = matches.get_many("ATTRIBUTES") {
        Ok(attributes.cloned().collect())
    } else {
        // shouldn't happen when the argument is required
        Err("missing attribute type name on command line")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exit_code = {
        let attributes = parse_arguments()?;
        let token_receiver = TsvTokenReceiver::new(attributes, stdout());
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
