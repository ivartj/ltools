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

pub struct CsvEntryWriter<W: Write> {
    attrspecs: Vec<AttrSpec>,
    dest: W,
    write_header: bool,
}

impl<W: Write> CsvEntryWriter<W> {
    pub fn new(attrspecs: Vec<AttrSpec>, dest: W) -> CsvEntryWriter<W> {

        CsvEntryWriter {
            attrspecs,
            dest,
            write_header: true,
        }
    }
}

fn csv_escape<W: Write> (dest: &mut W, field: &[u8]) -> Result<()> {
    let field_needs_escaping = field.iter()
        .copied()
        .any(|c| {
            match c {
                b',' | b'\n' | b'\r' | b'"' => true,
                _ => false,
            }
        });
    if !field_needs_escaping {
        dest.write_all(field)?;
        return Ok(());
    }
    dest.write_all(b"\"")?;
    for c in field.iter().copied() {
        if c == b'"' {
            dest.write_all(b"\"\"")?;
        } else {
            dest.write_all(&[c])?;
        }
    }
    dest.write_all(b"\"")?;
    Ok(())
}

impl<W: Write> WriteEntry for CsvEntryWriter<W> {
    fn write_entry(&mut self, attr2values: &HashMap<String, &Vec<EntryValue>>) -> Result<()> {
        if self.write_header {
            for (i, attrspec) in self.attrspecs.iter().enumerate() {
                if i != 0 {
                    self.dest.write_all(b",")?;
                }
                csv_escape(&mut self.dest, attrspec.attribute.as_bytes())?;
            }
            self.dest.write_all(b"\r\n")?;
            self.write_header = false;
        }
        let attrvalues: Vec<Vec<EntryValue>> = self.attrspecs.iter()
            .map(|attrspec| attrspec.filter_values(attr2values.get(&attrspec.attribute_lowercase).unwrap()).into_owned())
            .collect();
        for record in cartesian_product(&attrvalues) {
            for (i, value) in record.iter().enumerate() {
                if i != 0 {
                    self.dest.write_all(b",")?;
                }
                csv_escape(&mut self.dest, value)?;
            }
            self.dest.write_all(b"\r\n")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn test_a() -> Result<()> {
        let attrspecs = vec![AttrSpec::parse("DN")?, AttrSpec::parse("xmldata")?];
        let mut output: Vec<u8> = Vec::new();
        let mut csv_entry_writer = CsvEntryWriter::new(attrspecs, &mut output);
        csv_entry_writer.write_entry(&HashMap::from([
            (String::from("dn"), &vec![Cow::Owned(Vec::from(b"cn=foo,dc=example,dc=com".as_slice()))]),
            (String::from("xmldata"), &vec![Cow::Owned(Vec::from(b"<?xml version=\"1.0\"?><xml/>".as_slice()))]),
        ]))?;
        let expected = b"DN,xmldata\r\n\"cn=foo,dc=example,dc=com\",\"<?xml version=\"\"1.0\"\"?><xml/>\"\r\n";
        assert_eq!(String::from_utf8_lossy(output.as_slice()), String::from_utf8_lossy(expected));
        Ok(())
    }
}

