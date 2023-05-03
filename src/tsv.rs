use std::io::{
    Write,
    Result,
};
use std::collections::HashMap;
use std::borrow::Cow;
use crate::cartesian::cartesian_product;
use crate::base64::{DecodeWriter, DecodeState};
use crate::lexer::{
    Token,
    TokenKind,
    WriteToken,
};
use crate::attrspec::AttrSpec;
use crate::entry::{Entry, EntryValue};

#[derive(PartialEq)]
enum ValueType {
    Text,
    Base64,
}

pub trait WriteEntry {
    fn write_entry(&mut self, attr2values: &Entry) -> Result<()>;
}

impl<W: WriteEntry> WriteEntry for &mut W {
    fn write_entry(&mut self, attr2values: &Entry) -> Result<()> {
        (*self).write_entry(attr2values)
    }
}

pub struct HashMapTokenWriter<'a, W: WriteEntry> {
    attr2index: HashMap<String, usize>,
    attrvalues: Vec<Vec<EntryValue<'a>>>,
    attrmatch: Option<usize>, // index of currently matched attribute
    valuebuf: Vec<u8>,
    dest: W,
    valuetype: ValueType,
    b64state: DecodeState,
    record_separator: u8,
}

impl<'a, W: WriteEntry> HashMapTokenWriter<'a, W> {
    pub fn new(attributes: Vec<String>, dest: W) -> HashMapTokenWriter<'a, W> {
        let attrvalues = attributes.iter()
            .map(|_| Vec::new())
            .collect();
        let attr2index = attributes.into_iter()
            .map(|attr| attr.to_ascii_lowercase())
            .enumerate()
            .map(|(v, k)| (k, v))
            .collect();
        HashMapTokenWriter {
            attr2index,
            attrvalues,
            attrmatch: None,
            valuebuf: Vec::new(),
            dest,
            valuetype: ValueType::Text,
            b64state: DecodeState::default(),
            record_separator: b'\n',
        }
    }

    pub fn set_record_separator(&mut self, record_separator: u8) -> &mut Self {
        self.record_separator = record_separator;
        self
    }
}

impl<'a, W: WriteEntry> WriteToken for HashMapTokenWriter<'a, W> {
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
            TokenKind::EmptyLine => {
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

pub struct TsvHashMapWriter<W: Write> {
    attrspecs: Vec<AttrSpec>,
    dest: W,
    record_separator: u8,
}

impl<W: Write> TsvHashMapWriter<W> {
    pub fn new(attrspecs: Vec<AttrSpec>, dest: W) -> TsvHashMapWriter<W> {

        TsvHashMapWriter {
            attrspecs,
            dest,
            record_separator: b'\n',
        }
    }

    pub fn set_record_separator(&mut self, record_separator: u8) -> &mut Self {
        self.record_separator = record_separator;
        self
    }
}

impl<W: Write> WriteEntry for TsvHashMapWriter<W> {
    fn write_entry(&mut self, attr2values: &HashMap<String, &Vec<EntryValue>>) -> Result<()> {
        let attrvalues: Vec<Vec<EntryValue>> = self.attrspecs.iter()
            .map(|attrspec| attrspec.filter_values(attr2values.get(&attrspec.attribute).unwrap()).into_owned())
            .collect();
        for record in cartesian_product(&attrvalues) {
            for (i, value) in record.iter().enumerate() {
                if i != 0 {
                    self.dest.write_all(b"\t")?;
                }
                self.dest.write_all(value)?;
            }
            self.dest.write_all(&[self.record_separator])?;
        }
        Ok(())
    }
}

