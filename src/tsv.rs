use std::io::{
    Write,
    Result,
};
use std::collections::HashMap;
use crate::cartesian::cartesian_product;
use crate::attrspec::AttrSpec;
use crate::entry::{
    EntryValue,
    WriteEntry,
};

pub struct TsvEntryWriter<W: Write> {
    attrspecs: Vec<AttrSpec>,
    dest: W,
    record_separator: u8,
}

impl<W: Write> TsvEntryWriter<W> {
    pub fn new(attrspecs: Vec<AttrSpec>, dest: W) -> TsvEntryWriter<W> {

        TsvEntryWriter {
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

impl<W: Write> WriteEntry for TsvEntryWriter<W> {
    fn write_entry(&mut self, attr2values: &HashMap<String, &Vec<EntryValue>>) -> Result<()> {
        let attrvalues: Vec<Vec<EntryValue>> = self.attrspecs.iter()
            .map(|attrspec| attrspec.filter_values(attr2values.get(&attrspec.attribute_lowercase).unwrap()).into_owned())
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

#[cfg(test)]
mod test {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn test_case_difference() -> Result<()> {
        let attrspecs = vec![AttrSpec::parse("dn")?, AttrSpec::parse("CN")?];
        let mut output: Vec<u8> = Vec::new();
        let mut tsv_entry_writer = TsvEntryWriter::new(attrspecs, &mut output);
        tsv_entry_writer.write_entry(&HashMap::from([
            (String::from("cn"), &vec![Cow::Owned(Vec::from(&b"foo"[..]))]),
            (String::from("dn"), &vec![Cow::Owned(Vec::from(&b"cn=foo"[..]))]),
        ]))?;
        Ok(())
    }
}

