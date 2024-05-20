use std::collections::HashMap;
use std::borrow::{ Cow, Borrow };
use std::io::{ Result, Write };
use std::ops::Deref;
use crate::base64::{EncodeWriter, DecodeWriter, DecodeState};
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

pub struct Entry<'a, 'b>
where 'a: 'b
{
    attr2values: HashMap<String, Cow<'a, Vec<EntryValue<'b>>>>,
}

const NO_VALUES: &Vec<EntryValue<'static>> = &vec![];

impl<'a, 'b> Entry<'a, 'b> {
    pub fn get(&self, attr: &str) -> impl Iterator<Item = &[u8]> {
        let attr = attr.to_ascii_lowercase();
        let values: Option<&Vec<Cow<Vec<u8>>>> = self.attr2values.get(&attr)
            .map(|values| values.borrow());
        let values: &Vec<Cow<Vec<u8>>> = values.unwrap_or(NO_VALUES);
        values.iter()
            .map(|value: &Cow<Vec<u8>>| {
                let value: &Vec<u8> = value.borrow();
                let value: &[u8] = &value[..];
                value
            })
    }

    pub fn get_one(&self, attr: &str) -> Option<&[u8]> {
        if let Some(value) = self.get(attr).next() {
            Some(value.borrow())
        } else {
            None
        }
    }

    pub fn get_str(&self, attr: &str) -> impl Iterator<Item = Cow<str>> {
        // lifetimes are confusing
        let values: Vec<Cow<str>> = self.get(attr)
            .map(String::from_utf8_lossy)
            .collect();
        values.into_iter()
    }

    pub fn get_one_str(&self, attr: &str) -> Option<Cow<str>> {
        self.get_one(attr)
            .map(String::from_utf8_lossy)
    }

    pub fn attributes(&self) -> impl Iterator<Item = &str>
    {
        let attrs: Vec<&str> = self.attr2values.iter()
            .filter(|(_, values)| {
                !values.is_empty()
            })
            .map(|(attr, _)| attr.borrow())
            .collect();
        attrs.into_iter()
    }
}

pub type OwnedEntry = Entry<'static, 'static>;

impl<'a, 'b> From<&Entry<'a, 'b>> for OwnedEntry {
    fn from(entry: &Entry<'a, 'b>) -> OwnedEntry {
        let attr2values: HashMap<String, Cow<'static, Vec<EntryValue<'static>>>> = entry.attr2values.iter()
            .map(|(attr, values): (&String, &Cow<Vec<EntryValue>>)| {
                let attr = attr.to_ascii_lowercase();
                let values = Cow::Owned(
                    values.iter()
                        .map(|value: &EntryValue| Cow::Owned(value.deref().clone()))
                        .collect()
                );
                (attr, values)
            })
            .collect();
        Entry{
            attr2values,
        }
    }
}

pub type EntryValue<'a> = Cow<'a, Vec<u8>>;

impl<const N: usize> From<[(&str, &[u8]); N]> for Entry<'static, 'static> {
    fn from(array: [(&str, &[u8]); N]) -> Entry<'static, 'static> {
        let mut attr2values: HashMap<String, Cow<Vec<EntryValue>>> = HashMap::new();
        for (attr, value) in array.into_iter() {
            let mut values: Option<&mut Cow<Vec<EntryValue>>> = attr2values.get_mut(attr);
            if values.is_none() {
                attr2values.insert(attr.to_owned(), Cow::Owned(Vec::new()));
                values = attr2values.get_mut(attr)
            }
            if let Some(values) = values {
                let values: &mut Vec<EntryValue> = values.to_mut();
                values.push(Cow::Owned(Vec::from(value)));
            }
        }
        Entry{
            attr2values,
        }
    }
}

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
    Start,
    Version,
    BeforeEntry,
    Ignoring,
    Processing,
}

pub struct EntryTokenWriter<'a, W: WriteEntry> {
    all_attributes: bool,
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
    pub fn new(dest: W) -> EntryTokenWriter<'a, W> {
        EntryTokenWriter{
            all_attributes: true,
            state: WriterState::Start,
            attr2index: HashMap::new(),
            attrvalues: Vec::new(),
            attrmatch: None,
            valuebuf: Vec::new(),
            dest,
            valuetype: ValueType::Text,
            b64state: DecodeState::default(),
            ignore_entries_without_dn: false,
        }
    }

    pub fn new_for_attributes(attributes: Vec<String>, dest: W) -> EntryTokenWriter<'a, W> {
        let attrvalues = attributes.iter()
            .map(|_| Vec::new())
            .collect();
        let attr2index = attributes.into_iter()
            .map(|attr| attr.to_ascii_lowercase())
            .enumerate()
            .map(|(v, k)| (k, v))
            .collect();
        EntryTokenWriter{
            all_attributes: false,
            state: WriterState::Start,
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
                if self.state == WriterState::Start {
                    if attrlowercase == "version" {
                        self.state = WriterState::Version;
                    } else {
                        self.state = WriterState::BeforeEntry;
                    }
                }
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
                    let index: Option<usize> = self.attr2index.get(&attrlowercase).copied();
                    if index.is_none() && self.all_attributes {
                        let index: usize = self.attrvalues.len();
                        self.attrvalues.push(Vec::new());
                        self.attr2index.insert(attrlowercase, index);
                        Some(index)
                    } else {
                        index
                    }
                } else {
                    None
                };
            }
            TokenKind::ValueText => {
                if self.state == WriterState::Version {
                    self.state = WriterState::BeforeEntry;
                }
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
                    let attr2values: HashMap<String, Cow<Vec<EntryValue>>> = self.attr2index.iter()
                        .map(|(attr, index)| (attr.clone(), Cow::Borrowed(&self.attrvalues[*index])))
                        .collect();
                    self.dest.write_entry(&Entry{
                        attr2values,
                    })?;
                    for values in self.attrvalues.iter_mut() {
                        values.clear();
                    }
                }
                self.state = WriterState::BeforeEntry;
            }
        }
        Ok(())
    }
}

pub fn write_attrval<W: Write>(w: &mut W, attr: &str, value: &[u8]) -> std::io::Result<()> {
    write!(w, "{}:", attr)?;
    if is_ldif_safe_string(value) {
        writeln!(w, " {}", String::from_utf8_lossy(value))?;
    } else {
        write!(w, ":")?;
        let mut w = w;
        let mut base64 = EncodeWriter::new(&mut w);
        base64.write_all(value)?;
        base64.flush()?;
        writeln!(w)?;
    }
    Ok(())
}

pub fn write_entry_normally<W: Write>(w: &mut W, entry: &Entry) -> std::io::Result<()> {
    if let Some(dn) = entry.get_one("dn") {
        write_attrval(w, "dn", dn)?;
    }

    for attr in entry.attributes().filter(|attr| *attr != "dn") {
        for value in entry.get(attr) {
            write_attrval(w, attr, value)?;
        }
    }
    w.write_all(b"\n")
}

fn is_ldif_safe_string(value: &[u8]) -> bool {
    if let Some(c) = value.iter().copied().next() {
        if matches!(c, b'<' | b':') {
            return false;
        }
    }

    for c in value.iter().copied() {
        if matches!(c, b'\0' | b'\n' | b'\r' | b' ') {
            return false;
        }
        if c > 127 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::lexer::Lexer;
    use crate::loc::{ Loc, LocWrite };

    impl WriteEntry for Vec<OwnedEntry> {
        fn write_entry(&mut self, entry: &Entry) -> Result<()> {
            self.push(entry.into());
            Ok(())
        }
    }

    #[test]
    fn entry_token_writer_test_a() -> Result<()> {
        let ldif = br#"
version: 1
dn: cn=foo
cn: foo

search: 2

dn: cn=bar
CN: bar
"#;
        let mut entries = Vec::new();
        let mut token_writer = EntryTokenWriter::new_for_attributes(vec!["dn".into(), "cn".into()], &mut entries);
        token_writer.set_ignore_entries_without_dn(true);
        let mut lexer = Lexer::new(token_writer);

        lexer.loc_write(Loc::default(), ldif)?;
        lexer.loc_flush(Loc::default())?;

        assert_eq!(entries[0].get_one_str("dn"), Some(Cow::Borrowed("cn=foo")));
        assert_eq!(entries[0].get_one_str("cn"), Some(Cow::Borrowed("foo")));

        // this tests that there's no residue from the previous entry
        assert_eq!(entries[1].get_one_str("dn"), Some(Cow::Borrowed("cn=bar")));
        assert_eq!(entries[1].get_one_str("cn"), Some(Cow::Borrowed("bar")));

        // Because 'search: 2' does not start with a dn, it should not be regarded as an entry
        assert_eq!(entries.len(), 2);
        Ok(())
    }

}

