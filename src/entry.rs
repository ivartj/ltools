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

#[derive(Eq, PartialEq)]
enum WriterState {
    BeforeEntry,
    Ignoring,
    Processing,
}

pub struct EntryTokenWriter<'a, W: WriteEntry> {
    state: WriterState,
    attr2index: HashMap<String, usize>,
    attrvalues: Vec<Vec<EntryValue<'a>>>,
    attrmatch: Option<usize>, // index of currently matched attribute
    valuebuf: Vec<u8>,
    dest: W,
    valuetype: ValueType,
    b64state: DecodeState,
    ignore_entries_without_dn: bool,
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
            state: WriterState::BeforeEntry,
            attr2index,
            attrvalues,
            attrmatch: None,
            valuebuf: Vec::new(),
            dest,
            valuetype: ValueType::Text,
            b64state: DecodeState::default(),
            ignore_entries_without_dn: false,
        }
    }

    pub fn set_ignore_entries_without_dn(&mut self, value: bool) -> &mut Self {
        self.ignore_entries_without_dn = value;
        self
    }
}

impl<'a, W: WriteEntry> WriteToken for EntryTokenWriter<'a, W> {
    fn write_token(&mut self, token: Token) -> Result<()> {
        match token.kind {
            TokenKind::AttributeType => {
                let attrlowercase = token.segment.to_ascii_lowercase();
                if self.state == WriterState::BeforeEntry
                {
                    // We ignore entries that don't start with a dn.
                    // This might be information from ldapsearch about the search result or an LDIF
                    // version specifier.
                    self.state = if !self.ignore_entries_without_dn || attrlowercase == "dn" {
                        WriterState::Processing
                    } else {
                        WriterState::Ignoring
                    };
                }
                self.attrmatch = if self.state == WriterState::Processing {
                    self.attr2index.get(&attrlowercase).copied()
                } else {
                    None
                };
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
                if self.state == WriterState::Processing {
                    let entry: Entry = self.attr2index.iter()
                        .map(|(attr, idx)| (attr.clone(), &self.attrvalues[*idx]))
                        .collect();
                    self.dest.write_entry(&entry)?;
                    self.attrvalues.iter_mut().for_each(|values| values.clear());
                }
                self.state = WriterState::BeforeEntry;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::lexer::Lexer;
    use crate::loc::{ Loc, LocWrite };

    impl WriteEntry for Vec<HashMap<String, Vec<String>>> {
        fn write_entry(&mut self, entry: &Entry) -> Result<()> {
            let owned_entry: HashMap<String, Vec<String>> = entry.iter()
                .map(|(attr, values)| {
                    (attr.to_owned(), values.iter().map::<String, _>(|value| String::from_utf8_lossy(value).into_owned()).collect())
                 }).collect();
            self.push(owned_entry);
            Ok(())
        }
    }

    #[test]
    fn entry_token_writer_test_a() -> Result<()> {
        let ldif = br#"
dn: cn=foo
cn: foo

dn: cn=bar
cn: bar
"#;
        let mut entries = Vec::new();
        let token_writer = EntryTokenWriter::new(vec!["dn".into(), "cn".into()], &mut entries);
        let mut lexer = Lexer::new(token_writer);

        lexer.loc_write(Loc::default(), ldif)?;
        lexer.loc_flush(Loc::default())?;

        assert_eq!(entries[0]["dn"], vec!["cn=foo"]);
        assert_eq!(entries[0]["cn"], vec!["foo"]);

        // this tests that there's no residue from the previous entry
        assert_eq!(entries[1]["dn"], vec!["cn=bar"]);
        assert_eq!(entries[1]["cn"], vec!["bar"]);
        Ok(())
    }

}

