use std::io::Write;
use crate::cartesian::cartesian_product;
use crate::base64::{DecodeWriter, DecodeState};
use crate::lexer::{
    Token,
    TokenKind,
    WriteToken,
};

#[derive(PartialEq)]
enum ValueType {
    Text,
    Base64,
}

pub struct TsvTokenReceiver<W: Write> {
    attributes: Vec<String>,
    entryvalues: Vec<Vec<Vec<u8>>>,
    attrmatch: Option<usize>, // index of currently matched attribute
    valuebuf: Vec<u8>,
    dest: W,
    valuetype: ValueType,
    b64state: DecodeState,
}

impl<W: Write> TsvTokenReceiver<W> {
    pub fn new(attributes: Vec<String>, dest: W) -> TsvTokenReceiver<W> {
        let entryvalues = attributes.iter().map(|_| Vec::new()).collect();
        TsvTokenReceiver {
            attributes,
            entryvalues,
            attrmatch: None,
            valuebuf: Vec::new(),
            dest,
            valuetype: ValueType::Text,
            b64state: DecodeState::default(),
        }
    }
}

impl<W: Write> WriteToken for TsvTokenReceiver<W> {
    fn write_token(&mut self, token: Token) -> std::io::Result<()> {
        match token.kind {
            TokenKind::AttributeType => {
                let attrlowercase = token.segment.to_ascii_lowercase();
                self.attrmatch = self
                    .attributes
                    .iter()
                    .position(|attr| attr.to_ascii_lowercase() == attrlowercase);
            }
            TokenKind::ValueText => {
                if self.attrmatch.is_some() {
                    self.valuebuf.write_all(token.segment.as_bytes())?;
                    self.valuetype = ValueType::Text;
                }
            }
            TokenKind::ValueBase64 => {
                if self.attrmatch.is_some() {
                    let mut decoder =
                        DecodeWriter::new_with_state(&mut self.valuebuf, self.b64state);
                    decoder.write_all(token.segment.as_bytes())?;
                    self.b64state = decoder.get_state();
                    self.valuetype = ValueType::Base64;
                }
            }
            TokenKind::ValueFinish => {
                if let Some(attridx) = self.attrmatch {
                    if self.valuetype == ValueType::Base64 {
                        // TODO: consider raising an error if it isn't in a valid end state
                        self.b64state = DecodeState::default();
                    }
                    self.entryvalues[attridx].push(self.valuebuf.clone());
                    self.valuebuf.clear();
                }
            }
            TokenKind::EmptyLine => {
                for record in cartesian_product(&self.entryvalues) {
                    for (i, value) in record.iter().enumerate() {
                        if i != 0 {
                            self.dest.write_all(b"\t")?;
                        }
                        self.dest.write_all(value)?;
                    }
                    self.dest.write_all(b"\n")?;
                }
                for v in self.entryvalues.iter_mut() {
                    v.clear();
                }
            }
        }
        Ok(())
    }
}
