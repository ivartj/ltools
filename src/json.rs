use crate::attrspec::AttrSpec;
use crate::entry::{ Entry, WriteEntry };
use std::io::{
    Write,
    Result,
};
use std::write;
use std::borrow::Cow;

pub struct JsonEntryWriter<W: Write> {
    dest: W,
    record_separator: u8,
    attrspecs: Vec<AttrSpec>,
}

impl<W: Write> JsonEntryWriter<W> {
    pub fn new(attrspecs: Vec<AttrSpec>, dest: W) -> JsonEntryWriter<W> {
        JsonEntryWriter{
            dest,
            record_separator: b'\n',
            attrspecs,
        }
    }

    pub fn set_record_separator(&mut self, c: u8) -> &mut Self {
        self.record_separator = c;
        self
    }
}

fn write_json_string<W: Write>(w: &mut W, s: &str) -> Result<()> {
    let mut written: usize = 0;
    w.write_all(b"\"")?;
    let mut utf16buf: [u16;2] = [0;2];
    for (i, c) in s.char_indices() {
        if !c.is_ascii() || c.is_ascii_control() || c == '\\' || c == '"' {
            if i > written {
                w.write_all(&s.as_bytes()[written..i])?;
                written = i;
            }
            match c {
                '\\' | '"' => write!(w, "\\{c}")?,
                '\r' => w.write_all(b"\\r")?,
                '\n' => w.write_all(b"\\n")?,
                '\t' => w.write_all(b"\\t")?,
                c => {
                    for unit in c.encode_utf16(&mut utf16buf).iter() {
                        write!(w, "\\u{unit:04}")?;
                    }
                }
            }
            written += 1;
        }
    }
    if written < s.len() {
        w.write_all(&s.as_bytes()[written..])?;
    }
    w.write_all(b"\"")?;
    Ok(())
}

impl<W: Write> WriteEntry for JsonEntryWriter<W> {
    fn write_entry(&mut self, entry: &Entry) -> Result<()> {
        self.dest.write_all(b"{")?;
        for (i, (attrtype, values)) in entry.iter().enumerate() {
            let mut cow_values = Cow::Borrowed(*values);
            let mut attrtype: &str = attrtype;
            if let Some(attrspec) = self.attrspecs.iter()
                .find(|spec| spec.attribute_lowercase == *attrtype)
            {
              cow_values = attrspec.filter_values(values);
              attrtype = &attrspec.attribute;
            }
            if i != 0 {
                self.dest.write_all(b",")?;
            }
            write_json_string(&mut self.dest, attrtype)?;
            self.dest.write_all(b":[")?;
            for (i, value) in cow_values.iter().enumerate() {
                if i != 0 {
                    self.dest.write_all(b",")?;
                }
                let value = String::from_utf8_lossy(value);
                write_json_string(&mut self.dest, &value)?;
            }
            self.dest.write_all(b"]")?;
        }
        self.dest.write_all(b"}\n")?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn write_json_string_test_a() -> Result<()> {
        let mut buf = Vec::new();
        write_json_string(&mut buf, "foo\nbar")?;
        assert_eq!(String::from_utf8_lossy(&buf), r#""foo\nbar""#);
        Ok(())
    }

    #[test]
    fn write_json_string_test_b() -> Result<()> {
        let mut buf = Vec::new();
        write_json_string(&mut buf, "\n")?;
        assert_eq!(String::from_utf8_lossy(&buf), r#""\n""#);
        Ok(())
    }

    #[test]
    fn write_json_string_test_c() -> Result<()> {
        let mut buf = Vec::new();
        write_json_string(&mut buf, "foo\tbar\0baz\r\n")?;
        assert_eq!(String::from_utf8_lossy(&buf), r#""foo\tbar\u0000baz\r\n""#);
        Ok(())
    }
}
