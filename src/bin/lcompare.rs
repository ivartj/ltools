use clap::{arg, command};
use ltools::crstrip::CrStripper;
use ltools::lexer::Lexer;
use ltools::loc::WriteLocWrapper;
use ltools::unfold::Unfolder;
use ltools::entry::{Entry, OwnedEntry, WriteEntry, EntryTokenWriter};
use ltools::base64::EncodeWriter;
use std::io::{copy, Read, Write};
use std::collections::BTreeMap;
use std::borrow::Cow;
use std::cmp::Ordering;

struct Parameters {
    old: String,
    new: String,
}

fn parse_arguments() -> Result<Parameters, &'static str> {
    let mut params = Parameters{
        old: "-".into(),
        new: "-".into(),
    };

    let matches = command!("lcompare")
        .disable_colored_help(true)
        .arg(arg!(<OLD> "The LDIF entry records from which the changerecords transition"))
        .arg(arg!(<NEW> "The LDIF entry records to which the changerecords transition"))
        .get_matches();

    if let Some(old) = matches.get_one::<String>("OLD") {
        params.old = old.clone();
    } else {
        // shouldn't happen when the argument is required
        return Err("missing LDIF input parameter")
    }

    if let Some(new) = matches.get_one::<String>("NEW") {
        params.new = new.clone();
    } else {
        // shouldn't happen when the argument is required
        return Err("missing LDIF input parameter")
    }

    return Ok(params);
}

struct EntryBTreeMap(BTreeMap<String, OwnedEntry>);

impl EntryBTreeMap {
    fn new() -> EntryBTreeMap {
        EntryBTreeMap(BTreeMap::new())
    }
}

impl WriteEntry for EntryBTreeMap {
    fn write_entry(&mut self, entry: &Entry) -> std::io::Result<()> {
        let dn: Option<Cow<str>> = entry.get_one_str("dn");
        let dn: Cow<str> = match dn {
            Some(dn) => dn,
            None => {
                // drop entries without DN
                return Ok(());
            }
        };
        self.0.insert(dn.into_owned(), entry.into());
        Ok(())
    }
}

fn read_entries<R: Read>(mut input: R) -> std::io::Result<EntryBTreeMap> {
    let mut entries = EntryBTreeMap::new();
    let token_writer = EntryTokenWriter::new(&mut entries);
    let lexer = Lexer::new(token_writer);
    let unfolder = Unfolder::new(lexer);
    let crstripper = CrStripper::new(unfolder);
    let mut wrapper = WriteLocWrapper::new(crstripper);
    copy(&mut input, &mut wrapper)?;
    wrapper.flush()?;
    Ok(entries)
}

fn write_attrval<W: Write>(w: &mut W, attr: &str, value: &[u8]) -> std::io::Result<()> {
    write!(w, "{}:", attr)?;
    if is_ldif_safe_string(value) {
        writeln!(w, " {}", String::from_utf8_lossy(value))?;
    } else {
        write!(w, ":")?;
        let mut w = w;
        let mut base64 = EncodeWriter::new(&mut w);
        base64.write(value)?;
        base64.flush()?;
        write!(w, "\n")?;
    }
    Ok(())
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
        if !(c <= 127) {
            return false;
        }
    }
    true
}

fn write_add<'a, 'b, W: Write>(w: &mut W, entry: &Entry<'a, 'b>) -> std::io::Result<()> {
    let dn: Cow<str> = match entry.get_one_str("dn") {
        Some(dn) => dn,
        None => return Ok(()),
    };
    let mut w = w;
    write_attrval(&mut w, "dn", dn.as_bytes())?;
    writeln!(w, "changetype: add")?;
    for attr in entry.attributes() {
        if attr.to_lowercase() == "dn" {
            continue;
        }
        for value in entry.get(&attr) {
            write_attrval(&mut w, &attr, value)?;
        }
    }
    writeln!(w, "")?;
    Ok(())
}

fn write_delete<'a, 'b, W: Write>(w: &mut W, entry: &Entry<'a, 'b>) -> std::io::Result<()> {
    let dn: Cow<str> = match entry.get_one_str("dn") {
        Some(dn) => dn,
        None => return Ok(()),
    };
    let mut w = w;
    write_attrval(&mut w, "dn", dn.as_bytes())?;
    writeln!(w, "changetype: delete")?;
    writeln!(w, "")?;
    Ok(())
}

enum ModifyChangeRecordOpType {
    Add,
    Delete,
    Replace,
}

struct ModifyChangeRecordOp<'a> {
    typ: ModifyChangeRecordOpType,
    attr: String,
    values: Vec<&'a [u8]>,
}

struct ModifyChangeRecord<'a> {
    dn: String,
    ops: Vec<ModifyChangeRecordOp<'a>>,
}

impl<'z> ModifyChangeRecord<'z> {
    fn new<'a, 'b, 'c, 'd>(old: &'z Entry<'a, 'b>, new: &'z Entry<'c, 'd>) -> Option<ModifyChangeRecord<'z>>
    where
        'b: 'z,
        'd: 'z
    {
        let dn: Cow<str> = match old.get_one_str("dn") {
            Some(dn) => dn,
            None => return None,
        };
        let mut modify = ModifyChangeRecord{
            dn: dn.into_owned(),
            ops: Vec::new(),
        };
        let mut old_attrs: Vec<Cow<str>> = old.attributes().collect();
        let mut new_attrs: Vec<Cow<str>> = new.attributes().collect();
        old_attrs.sort();
        new_attrs.sort();
        let mut old_iter = old_attrs.iter().peekable();
        let mut new_iter = new_attrs.iter().peekable();
        loop {
            match (old_iter.peek(), new_iter.peek()) {
                (Some(old_attr), Some(new_attr)) => {
                    match old_attr.cmp(new_attr) {
                        Ordering::Equal => {
                            let del_values: Vec<&[u8]> = old.get(&old_attr)
                                .filter(|old_value: &&[u8]| {
                                    new.get(&new_attr)
                                        .any(|new_value: &[u8]| {
                                            let equal = new_value == *old_value;
                                            equal
                                        }) == false
                                })
                                .collect();
                            let add_values: Vec<&[u8]> = new.get(&new_attr)
                                .filter(|new_value: &&[u8]| {
                                    old.get(&old_attr)
                                        .any(|old_value: &[u8]| {
                                            let equal = old_value == *new_value;
                                            equal
                                        }) == false
                                })
                                .collect();
                            if add_values.len() == 1 && del_values.len() == 1 {
                                // at least on eDirectory, replace works better on single-valued attributes
                                let op = ModifyChangeRecordOp{
                                    typ: ModifyChangeRecordOpType::Replace,
                                    attr: new_attr.to_string(),
                                    values: add_values,
                                };
                                modify.ops.push(op);
                            } else {
                                if del_values.len() != 0 {
                                    let op = ModifyChangeRecordOp{
                                        typ: ModifyChangeRecordOpType::Delete,
                                        attr: old_attr.to_string(),
                                        values: del_values,
                                    };
                                    modify.ops.push(op);
                                }
                                if add_values.len() != 0 {
                                    let op = ModifyChangeRecordOp{
                                        typ: ModifyChangeRecordOpType::Add,
                                        attr: new_attr.to_string(),
                                        values: add_values,
                                    };
                                    modify.ops.push(op);
                                }
                            }
                            old_iter.next();
                            new_iter.next();
                        },
                        Ordering::Less => {
                            let op = ModifyChangeRecordOp{
                                typ: ModifyChangeRecordOpType::Delete,
                                attr: old_attr.to_string(),
                                values: old.get(&old_attr)
                                    .collect(),
                            };
                            if op.values.len() != 0 {
                                modify.ops.push(op);
                            }
                            old_iter.next();
                        },
                        Ordering::Greater => {
                            let op = ModifyChangeRecordOp{
                                typ: ModifyChangeRecordOpType::Add,
                                attr: new_attr.to_string(),
                                values: new.get(&new_attr)
                                    .collect(),
                            };
                            if op.values.len() != 0 {
                                modify.ops.push(op);
                            }
                            new_iter.next();
                        },
                    }
                },
                (Some(old_attr), None) => {
                    let op = ModifyChangeRecordOp{
                        typ: ModifyChangeRecordOpType::Delete,
                        attr: old_attr.to_string(),
                        values: old.get(&old_attr)
                            .collect(),
                    };
                    modify.ops.push(op);
                    old_iter.next();
                },
                (None, Some(new_attr)) => {
                    let op = ModifyChangeRecordOp{
                        typ: ModifyChangeRecordOpType::Add,
                        attr: new_attr.to_string(),
                        values: new.get(&new_attr)
                            .collect(),
                    };
                    modify.ops.push(op);
                    new_iter.next();
                },
                (None, None) => break,
            }
        }
        if modify.ops.len() == 0 {
            None
        } else {
            Some(modify)
        }
    }
}

fn write_modify<W: Write>(w: &mut W, modify: &ModifyChangeRecord) -> std::io::Result<()> {
    let mut w = w;
    write_attrval(&mut w, "dn", modify.dn.as_bytes())?;
    writeln!(w, "changetype: modify")?;
    for op in modify.ops.iter() {
        match op.typ {
            ModifyChangeRecordOpType::Add => {
                writeln!(w, "add: {}", op.attr)?;
            },
            ModifyChangeRecordOpType::Delete => {
                writeln!(w, "delete: {}", op.attr)?;
            },
            ModifyChangeRecordOpType::Replace => {
                writeln!(w, "replace: {}", op.attr)?;
            },
        }
        for value in op.values.iter() {
            write_attrval(&mut w, &op.attr, value)?;
        }
        writeln!(w, "-")?;
    }
    writeln!(w, "")?;
    Ok(())
}

fn compare_entries(old_entries: &EntryBTreeMap, new_entries: &EntryBTreeMap) -> std::io::Result<()> {
    let mut old_iter = old_entries.0.iter().peekable();
    let mut new_iter = new_entries.0.iter().peekable();
    loop {
        match (old_iter.peek(), new_iter.peek()) {
            (Some((old_dn, old_entry)), Some((new_dn, new_entry))) => {
                match old_dn.cmp(new_dn) {
                    Ordering::Equal => {
                        if let Some(change) = ModifyChangeRecord::new(&old_entry, &new_entry) {
                            write_modify(&mut std::io::stdout(), &change)?;
                        }
                        old_iter.next();
                        new_iter.next();
                    },
                    Ordering::Less => {
                        write_delete(&mut std::io::stdout(), &old_entry)?;
                        old_iter.next();
                    }
                    Ordering::Greater => {
                        write_add(&mut std::io::stdout(), &new_entry)?;
                        new_iter.next();
                    }
                }
            },
            (Some((_, old_entry)), None) => {
                write_delete(&mut std::io::stdout(), &old_entry)?;
                old_iter.next();
            },
            (None, Some((_, new_entry))) => {
                write_add(&mut std::io::stdout(), &new_entry)?;
                new_iter.next();
            },
            (None, None) => break,
        }
    }
    Ok(())
}

fn do_io<Old: Read, New: Read>(old: &mut Old, new: &mut New) -> std::io::Result<()> {
    let old_entries = read_entries(old)?;
    let new_entries = read_entries(new)?;
    compare_entries(&old_entries, &new_entries)?;
    Ok(())
}

fn get_result() -> Result<(), Box<dyn std::error::Error>> {
    let params = parse_arguments()?;
    match (&params.old[..], &params.new[..]) {
        ("-", "-") => {
            return Err("both inputs can't be standard input".into())
        },
        ("-", new) => {
            let mut old = std::io::stdin();
            let mut new = std::fs::File::open(&new)?;
            do_io(&mut old, &mut new)?;
        },
        (old, "-") => {
            let mut old = std::fs::File::open(&old)?;
            let mut new = std::io::stdin();
            do_io(&mut old, &mut new)?;
        }
        (old, new) => {
            let mut old = std::fs::File::open(&old)?;
            let mut new = std::fs::File::open(&new)?;
            do_io(&mut old, &mut new)?;
        },
    }
    Ok(())
}

fn main() {
    let result = get_result();
    if let Err(err) = result {
        eprintln!("lcompare: {}", err);
        std::process::exit(1);
    }
}
