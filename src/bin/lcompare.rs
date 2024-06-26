use clap::{arg, command, ArgAction};
use ltools::crstrip::CrStripper;
use ltools::entry::{Entry, EntryTokenWriter, OwnedEntry, WriteEntry, write_attrval};
use ltools::lexer::Lexer;
use ltools::loc::WriteLocWrapper;
use ltools::unfold::Unfolder;
use std::borrow::Cow;
use std::cmp::{Ord, Ordering};
use std::collections::BTreeMap;
use std::io::{copy, Read, Write};
use std::ops::Deref;
use std::iter::Peekable;

struct Parameters {
    old: String,
    new: String,
    invert: bool,
    force: bool,
    attrs: Vec<String>,       // should be lowercase
    defer_attrs: Vec<String>, // should be lowercase
}

fn parse_arguments() -> Result<Parameters, &'static str> {
    let mut params = Parameters {
        old: "-".into(),
        new: "-".into(),
        attrs: Vec::new(),
        invert: false,
        force: false,
        defer_attrs: Vec::new(),
    };

    let matches = command!("lcompare")
        .disable_colored_help(true)
        .arg(arg!(<OLD> "The LDIF entry records from which the changerecords transition"))
        .arg(arg!(<NEW> "The LDIF entry records to which the changerecords transition"))
        .arg(arg!([ATTRIBUTES] ... "In modify and add changerecords, limit changes to attributes in ATTRIBUTES, or if the -v option is given, every attribute except for those in ATTRIBUTES"))
        .arg(arg!(defer: --defer <ATTRIBUTE> "Defer addition and modification of the given attribute. This is useful to avoid referential integrity errors.")
            .required(false)
            .action(ArgAction::Append)
        )
        .arg(arg!(invert: -v --invert "In modify and add changerecords, compare based on every attribute except for those in ATTRIBUTES").action(ArgAction::SetTrue))
        .arg(arg!(force: -f --force "Allow lcompare to output delete changerecords").action(ArgAction::SetTrue))
        .get_matches();

    if let Some(old) = matches.get_one::<String>("OLD") {
        params.old = old.clone();
    } else {
        // shouldn't happen when the argument is required
        return Err("missing LDIF input parameter");
    }

    if let Some(new) = matches.get_one::<String>("NEW") {
        params.new = new.clone();
    } else {
        // shouldn't happen when the argument is required
        return Err("missing LDIF input parameter");
    }

    params.attrs = matches.get_many::<String>("ATTRIBUTES")
        .map(|attrs| attrs.map(|attr| attr.to_lowercase()).collect())
        .unwrap_or_else(Vec::new);
    params.invert = matches.get_flag("invert") != params.attrs.is_empty();

    params.defer_attrs = matches.get_many::<String>("defer")
        .map(|attrs| attrs.map(|attr| attr.to_lowercase()).collect())
        .unwrap_or_else(Vec::new);
    if params.invert {
        params.attrs.extend(params.defer_attrs.iter().cloned());
    }

    params.force = matches.get_flag("force");

    Ok(params)
}

#[derive(PartialEq, Eq)]
struct DnKey(String);

impl Deref for DnKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Ord for DnKey {
    fn cmp(&self, other: &Self) -> Ordering {
        // we want shorter ancestor DNs to be ordered before longer descendant DNs
        let cmp = self.0.len().cmp(&other.0.len());
        match cmp {
            Ordering::Equal => self.0.cmp(&other.0),
            _ => cmp,
        }
    }
}

impl PartialOrd for DnKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct EntryBTreeMap(BTreeMap<DnKey, OwnedEntry>);

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
        self.0.insert(DnKey(dn.to_lowercase()), entry.into());
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

fn write_add<W: Write>(
    w: &mut W,
    entry: &Entry<'_, '_>,
    attrs: &[String],
    invert: bool,
) -> std::io::Result<()> {
    let dn: Cow<str> = match entry.get_one_str("dn") {
        Some(dn) => dn,
        None => return Ok(()),
    };
    let mut w = w;
    write_attrval(&mut w, "dn", dn.as_bytes())?;
    writeln!(w, "changetype: add")?;
    for attr in entry
        .attributes()
        .filter(|attr| invert != attrs.iter().any(|arg_attr| attr.lowercase == arg_attr))
    {
        if attr.lowercase == "dn" {
            continue;
        }
        for value in entry.get(attr.name) {
            write_attrval(&mut w, attr.name, value)?;
        }
    }
    writeln!(w)?;
    Ok(())
}

fn write_delete<W: Write>(w: &mut W, dn: &str) -> std::io::Result<()> {
    let mut w = w;
    write_attrval(&mut w, "dn", dn.as_bytes())?;
    writeln!(w, "changetype: delete")?;
    writeln!(w)?;
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
    fn new<'a, 'b, 'c, 'd>(
        old: Option<&'z Entry<'a, 'b>>,
        new: &'z Entry<'c, 'd>,
        attrs: &[String],
        invert: bool,
    ) -> Option<ModifyChangeRecord<'z>>
    where
        'b: 'z,
        'd: 'z,
    {
        let dn: Cow<str> = match new.get_one_str("dn") {
            Some(dn) => dn,
            None => return None,
        };
        let mut modify = ModifyChangeRecord {
            dn: dn.into_owned(),
            ops: Vec::new(),
        };

        let mut old_attrs: Vec<&str> = match old {
            Some(old) => old
                .attributes()
                .map(|attr| attr.lowercase)
                .filter(|lattr| lattr != &"dn")
                .filter(|lattr| invert != attrs.iter().any(|arg_attr| arg_attr == *lattr))
                .collect(),
            None => Vec::new(),
        };
        let mut new_attrs: Vec<&str> = new
            .attributes()
            .map(|attr| attr.lowercase)
            .filter(|lattr| lattr != &"dn")
            .filter(|lattr| invert != attrs.iter().any(|arg_attr| arg_attr == *lattr))
            .collect();

        old_attrs.sort();
        new_attrs.sort();
        let comparison = SortedComparison{
            old_iter: old_attrs.iter().peekable(),
            new_iter: new_attrs.iter().peekable(),
            compare_items: Ord::cmp,
        };
        for op in comparison {
            match op {
                Diff::Add(new_attr) => {
                    let op = ModifyChangeRecordOp {
                        typ: ModifyChangeRecordOpType::Add,
                        attr: new_attr.to_string(),
                        values: new.get(new_attr).collect(),
                    };
                    if !op.values.is_empty() {
                        modify.ops.push(op);
                    }
                },
                Diff::Delete(old_attr) => {
                    if let Some(old) = old {
                        let op = ModifyChangeRecordOp {
                            typ: ModifyChangeRecordOpType::Delete,
                            attr: old_attr.to_string(),
                            values: old.get(old_attr).collect(),
                        };
                        if !op.values.is_empty() {
                            modify.ops.push(op);
                        }
                    }
                },
                Diff::Modify(old_attr, new_attr) => {
                    let del_values: Vec<&[u8]> = {
                        if let Some(old) = old {
                            old.get(old_attr)
                                .filter(|old_value: &&[u8]| {
                                    !new.get(new_attr)
                                        .any(|new_value: &[u8]| new_value == *old_value)
                                })
                                .collect()
                        } else {
                            Vec::new()
                        }
                    };
                    let add_values: Vec<&[u8]> = new
                        .get(new_attr)
                        .filter(|new_value: &&[u8]| {
                            if let Some(old) = old {
                                !old.get(old_attr)
                                    .any(|old_value: &[u8]| old_value == *new_value)
                            } else {
                                true
                            }
                        })
                        .collect();
                    if add_values.len() == 1 && del_values.len() == 1 && new.get(new_attr).count() == 1 {
                        // at least on eDirectory, replace works better on single-valued attributes
                        let op = ModifyChangeRecordOp {
                            typ: ModifyChangeRecordOpType::Replace,
                            attr: new_attr.to_string(),
                            values: add_values,
                        };
                        modify.ops.push(op);
                    } else {
                        if !del_values.is_empty() {
                            let op = ModifyChangeRecordOp {
                                typ: ModifyChangeRecordOpType::Delete,
                                attr: old_attr.to_string(),
                                values: del_values,
                            };
                            modify.ops.push(op);
                        }
                        if !add_values.is_empty() {
                            let op = ModifyChangeRecordOp {
                                typ: ModifyChangeRecordOpType::Add,
                                attr: new_attr.to_string(),
                                values: add_values,
                            };
                            modify.ops.push(op);
                        }
                    }
                },
            }
        }
        if modify.ops.is_empty() {
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
            }
            ModifyChangeRecordOpType::Delete => {
                writeln!(w, "delete: {}", op.attr)?;
            }
            ModifyChangeRecordOpType::Replace => {
                writeln!(w, "replace: {}", op.attr)?;
            }
        }
        for value in op.values.iter() {
            write_attrval(&mut w, &op.attr, value)?;
        }
        writeln!(w, "-")?;
    }
    writeln!(w)?;
    Ok(())
}

fn compare_entries(
    old_entries: &EntryBTreeMap,
    new_entries: &EntryBTreeMap,
    params: &Parameters,
) -> std::io::Result<()> {
    let comparison = SortedComparison{
        old_iter: old_entries.0.iter().peekable(),
        new_iter: new_entries.0.iter().peekable(),
        compare_items: |(old_dn, _), (new_dn, _)| old_dn.cmp(new_dn),
    };
    let mut deferred_deletes: Vec<Cow<str>> = Vec::new();
    let mut deferred_modifies: Vec<ModifyChangeRecord> = Vec::new();
    for op in comparison {
        match op {
            Diff::Add((_, new_entry)) => {
                write_add(
                    &mut std::io::stdout(),
                    new_entry,
                    &params.attrs,
                    params.invert,
                )?;
                if let Some(defer) =
                    ModifyChangeRecord::new(None, new_entry, &params.defer_attrs, false)
                {
                    deferred_modifies.push(defer)
                }
            },
            Diff::Delete((_, old_entry)) => {
                if let Some(dn) = old_entry.get_one_str("dn") {
                    if params.force {
                        deferred_deletes.push(dn);
                    } else {
                        eprintln!("lcompare: no --force option given, not writing delete changerecord for {}", dn);
                    }
                }
            },
            Diff::Modify((_, old_entry), (_, new_entry)) => {
                if let Some(change) = ModifyChangeRecord::new(
                    Some(old_entry),
                    new_entry,
                    &params.attrs,
                    params.invert,
                ) {
                    write_modify(&mut std::io::stdout(), &change)?;
                }
                if let Some(defer) = ModifyChangeRecord::new(
                    Some(old_entry),
                    new_entry,
                    &params.defer_attrs,
                    false,
                ) {
                    deferred_modifies.push(defer)
                }
            },
        }
    }
    for modify in deferred_modifies.iter() {
        write_modify(&mut std::io::stdout(), modify)?;
    }
    for delete in deferred_deletes.iter().rev() {
        write_delete(&mut std::io::stdout(), delete)?;
    }
    Ok(())
}

struct SortedComparison<T, O, N, F>
    where T: Copy,
          O: Iterator<Item = T>,
          N: Iterator<Item = T>,
          F: Fn(&T, &T) -> Ordering,
{
    old_iter: Peekable<O>,
    new_iter: Peekable<N>,
    compare_items: F,
}

enum Diff<T> {
    Add(T),
    Delete(T),
    Modify(T, T),
}

impl<T, O, N, F> Iterator for SortedComparison<T, O, N, F>
where T: Copy,
      O: Iterator<Item = T>,
      N: Iterator<Item = T>,
      F: Fn(&T, &T) -> Ordering,
{
    type Item = Diff<T>;

    fn next(&mut self) -> Option<Diff<T>> {
        match (self.old_iter.peek(), self.new_iter.peek()) {
            (Some(old), Some(new)) => {
                match (self.compare_items)(old, new) {
                    Ordering::Less => {
                        let op = Diff::Delete(*old);
                        self.old_iter.next();
                        Some(op)
                    },
                    Ordering::Greater => {
                        let op = Diff::Add(*new);
                        self.new_iter.next();
                        Some(op)
                    },
                    Ordering::Equal => {
                        let op = Diff::Modify(*old, *new);
                        self.old_iter.next();
                        self.new_iter.next();
                        Some(op)
                    },
                }
            },
            (Some(old), None) => {
                let op = Diff::Delete(*old);
                self.old_iter.next();
                Some(op)
            },
            (None, Some(new)) => {
                let op = Diff::Add(*new);
                self.new_iter.next();
                Some(op)
            },
            (None, None) => None,
        }
    }
}

fn do_io<Old: Read, New: Read>(
    old: &mut Old,
    new: &mut New,
    params: &Parameters,
) -> std::io::Result<()> {
    let old_entries = read_entries(old)?;
    let new_entries = read_entries(new)?;
    compare_entries(&old_entries, &new_entries, params)?;
    Ok(())
}

fn get_result() -> Result<(), Box<dyn std::error::Error>> {
    let params = parse_arguments()?;
    match (&params.old[..], &params.new[..]) {
        ("-", "-") => return Err("both inputs can't be standard input".into()),
        ("-", new) => {
            let mut old = std::io::stdin();
            let mut new = std::fs::File::open(new)?;
            do_io(&mut old, &mut new, &params)?;
        }
        (old, "-") => {
            let mut old = std::fs::File::open(old)?;
            let mut new = std::io::stdin();
            do_io(&mut old, &mut new, &params)?;
        }
        (old, new) => {
            let mut old = std::fs::File::open(old)?;
            let mut new = std::fs::File::open(new)?;
            do_io(&mut old, &mut new, &params)?;
        }
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let old = vec![1,3,5];
        let new = vec![2,3,4];
        let comparison = SortedComparison{
            old_iter: old.iter().peekable(),
            new_iter: new.iter().peekable(),
            compare_items: |x, y| x.cmp(y),
        };

        for op in comparison {
            match op {
                Diff::Add(x) => println!("add {}", x),
                Diff::Delete(x) => println!("del {}", x),
                Diff::Modify(x, y) => println!("mod {} {}", x, y),
            }
        }
    }
}

