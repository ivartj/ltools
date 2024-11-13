pub mod parser;

use crate::entry::Entry;
use crate::filter::parser::filter as parse_filter;
use std::mem::swap;
use std::collections::BTreeSet;

#[derive(Debug, PartialEq)]
pub enum Filter {
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Box<Filter>),
    Simple(AttributeDescription, FilterType, Vec<u8>),
    Present(AttributeDescription),
    Substring(AttributeDescription, Vec<GlobPart>),
    // TODO: Extensible(...
}

#[derive(Debug, Eq, PartialEq)]
pub struct AttributeDescription {
    pub attribute_type: String,
    // TODO: add options
}

#[derive(Debug, Eq, PartialEq)]
pub enum FilterType {
    Equal,
    Approx,
    GreaterOrEqual,
    LessOrEqual,
}

#[derive(Debug, Eq, PartialEq)]
pub enum GlobPart {
    Wildcard,
    Literal(u8),
}

impl Filter {
    pub fn parse(s: &str) -> Result<Filter, &'static str> {
        let (remainder, filter) = match parse_filter(s) {
            Ok(filter) => filter,
            Err(_) => return Err("failed to parse LDAP filter"),
        };
        if remainder.trim() != "" {
            return Err("failed to parse LDAP filter");
        }
        Ok(filter)
    }

    pub fn is_match(&self, entry: &Entry) -> bool {
        match self {
            Filter::And(filters) => filters.iter()
                .all(|filter| filter.is_match(entry)),
            Filter::Or(filters) => filters.iter()
                .any(|filter| filter.is_match(entry)),
            Filter::Not(filter) => !filter.is_match(entry),
            Filter::Present(attrdesc) => {
                let attr = &attrdesc.attribute_type;
                entry.get(attr).count() != 0
            }
            Filter::Simple(attrdesc, filtertype, filtervalue) => {
                let attr = &attrdesc.attribute_type;
                let equal = entry.get(attr).any(|value| {
                    let value = value.to_ascii_lowercase();
                    value == *filtervalue
                });
                match filtertype {
                    FilterType::Equal | FilterType::Approx => equal,
                    FilterType::GreaterOrEqual | FilterType::LessOrEqual => todo!(),
                }
            },
            Filter::Substring(attrdesc, glob) => {
                let attr = &attrdesc.attribute_type;
                for value in entry.get(attr) {
                    if is_match(glob, value) {
                        return true;
                    }
                }
                false
            },
        }
    }
}

fn is_match(glob: &[GlobPart], value: &[u8]) -> bool {
    let mut old_states: BTreeSet<usize> = BTreeSet::new(); // indices into glob
    let mut new_states: BTreeSet<usize> = BTreeSet::new();
    let mut post_wildcard_states: BTreeSet<usize> = BTreeSet::new();

    // insert initial states
    old_states.insert(0);
    if glob.get(0) == Some(&GlobPart::Wildcard) {
        if let Some(first_non_wildcard) = (1..glob.len()).find(|idx| !matches!(glob.get(*idx), Some(&GlobPart::Wildcard))) {
            old_states.insert(first_non_wildcard);
        }
    }

    for value_byte in value.iter().copied() {
        let value_byte_lowercased = value_byte.to_ascii_lowercase();
        for state in old_states.iter() {
            match glob.get(*state) {
                Some(GlobPart::Literal(glob_byte)) => {
                    if value_byte == *glob_byte || value_byte_lowercased == *glob_byte {
                        new_states.insert(*state + 1);
                    }
                },
                Some(GlobPart::Wildcard) => {
                    new_states.insert(*state);
                },
                None => {},
            }
        }
        for state in new_states.iter().copied() {
            if glob.get(state) == Some(&GlobPart::Wildcard) {
                let mut post_wildcard_state = state + 1;
                while glob.get(post_wildcard_state) == Some(&GlobPart::Wildcard) {
                    // handle consecutive wildcards as one wildcard
                    post_wildcard_state += 1;
                }
                post_wildcard_states.insert(post_wildcard_state);
            }
        }
        new_states.extend(post_wildcard_states.iter());
        post_wildcard_states.clear();
        swap(&mut old_states, &mut new_states);
        new_states.clear();
    }
    old_states.iter().any(|state| *state == glob.len())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::entry::{OwnedEntry, EntryTokenWriter};
    use crate::lexer::Lexer;
    use crate::loc::WriteLocWrapper;
    use std::io::Write;

    #[test]
    fn test_1() -> Result<(), Box<dyn std::error::Error>> {
        let ldif = br#"
dn: cn=foo
cn: foo
"#;
        let mut entries: Vec<OwnedEntry> = Vec::new();
        let token_writer = EntryTokenWriter::new(&mut entries);
        let mut lexer = Lexer::new(token_writer);
        let mut wrapper = WriteLocWrapper::new(&mut lexer);
        wrapper.write_all(ldif)?;
        wrapper.flush()?;

        let filter = Filter::parse("(cn=FOO)")?;
        if let Some(entry) = entries.get(0) {
            assert!(filter.is_match(entry));
        }

        let filter = Filter::parse("(cn=f*)")?;
        if let Some(entry) = entries.get(0) {
            assert!(filter.is_match(entry));
        }

        let filter = Filter::parse("(cn=foo*)")?;
        if let Some(entry) = entries.get(0) {
            assert!(filter.is_match(entry));
        }

        Ok(())
    }

    #[test]
    fn test_2() -> Result<(), Box<dyn std::error::Error>> {
        let ldif = br#"
dn: cn=MyDriver,cn=driverset1,o=system
DirXML-ConfigValues:: c3JjUm9vdAo=
"#;
        let mut entries: Vec<OwnedEntry> = Vec::new();
        let token_writer = EntryTokenWriter::new(&mut entries);
        let mut lexer = Lexer::new(token_writer);
        let mut wrapper = WriteLocWrapper::new(&mut lexer);
        wrapper.write_all(ldif)?;
        wrapper.flush()?;

        let Some(entry) = entries.get(0) else {
            panic!();
        };
        let filter = Filter::parse("(DirXML-ConfigValues=*srcRoot*)")?;

        assert!(filter.is_match(entry));

        Ok(())
    }

}
