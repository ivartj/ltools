use crate::entry::{ Entry, WriteEntry };
use std::io::{
    Write,
    Result,
};
use std::write;

pub struct JsonEntryWriter<W: Write> {
    dest: W,
}

impl<W: Write> JsonEntryWriter<W> {
    pub fn new(dest: W) -> JsonEntryWriter<W> {
        JsonEntryWriter{
            dest,
        }
    }
}

fn write_json_string<W: Write>(w: &mut W, s: &str) -> Result<()> {
    let mut written: usize = 0;
    w.write_all(b"\"")?;
    for (i, c) in s.char_indices() {
        match c {
            '\\' | '"' | '\r' | '\n' => if written < i {
                w.write_all(&s.as_bytes()[written..i])?;
                written = i;
            },
            _ => (),
        }
        match c {
            '\\' | '"' => write!(w, "\\{c}")?,
            '\r' => w.write_all(b"\\r")?,
            '\n' => w.write_all(b"\\n")?,
            _ => (),
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
            if i != 0 {
                self.dest.write_all(b",")?;
            }
            write_json_string(&mut self.dest, attrtype)?;
            self.dest.write_all(b":[")?;
            for (i, value) in values.iter().enumerate() {
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

