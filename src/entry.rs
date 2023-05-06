use std::collections::HashMap;
use std::borrow::Cow;
use std::io::{ Result, Write };
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


pub type Entry<'a, 'b> = HashMap<String, &'a Vec<EntryValue<'b>>>;

pub type EntryValue<'a> = Cow<'a, Vec<u8>>;

pub trait WriteEntry {
    fn write_entry(&mut self, attr2values: &Entry) -> Result<()>;
}

impl<W: WriteEntry> WriteEntry for &mut W {
    fn write_entry(&mut self, attr2values: &Entry) -> Result<()> {
        (*self).write_entry(attr2values)
    }
}

pub struct EntryTokenWriter<'a, W: WriteEntry> {
    attr2index: HashMap<String, usize>,
    attrvalues: Vec<Vec<EntryValue<'a>>>,
    attrmatch: Option<usize>, // index of currently matched attribute
    valuebuf: Vec<u8>,
    dest: W,
    valuetype: ValueType,
    b64state: DecodeState,
}

impl<'a, W: WriteEntry> EntryTokenWriter<'a, W> {
    pub fn new(attributes: Vec<String>, dest: W) -> EntryTokenWriter<'a, W> {
        let attrvalues = attributes.iter()
            .map(|_| Vec::new())
            .collect();
        let attr2index = attributes.into_iter()
            .map(|attr| attr.to_ascii_lowercase())
            .enumerate()
            .map(|(v, k)| (k, v))
            .collect();
        EntryTokenWriter {
            attr2index,
            attrvalues,
            attrmatch: None,
            valuebuf: Vec::new(),
            dest,
            valuetype: ValueType::Text,
            b64state: DecodeState::default(),
        }
    }
}

impl<'a, W: WriteEntry> WriteToken for EntryTokenWriter<'a, W> {
    fn write_token(&mut self, token: Token) -> Result<()> {
        match token.kind {
            TokenKind::AttributeType => {
                let attrlowercase = token.segment.to_ascii_lowercase();
                self.attrmatch = self.attr2index.get(&attrlowercase).copied();
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
                    self.attrvalues[attridx].push(Cow::Owned(self.valuebuf.clone()));
                    self.valuebuf.clear();
                }
            }
            TokenKind::EntryFinish => {
                let attr2values: HashMap<String, &Vec<EntryValue>> = self.attr2index.iter()
                    .map(|(attr, index)| (attr.to_owned(), &self.attrvalues[*index]))
                    .collect();
                self.dest.write_entry(&attr2values)?;
                for v in self.attrvalues.iter_mut() {
                    v.clear();
                }
            }
        }
        Ok(())
    }
}

