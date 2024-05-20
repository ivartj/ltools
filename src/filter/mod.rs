pub mod parser;

use crate::entry::Entry;
use crate::filter::parser::filter as parse_filter;

#[derive(Debug, PartialEq)]
pub enum Filter {
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Box<Filter>),
    Simple(AttributeDescription, FilterType, Vec<u8>),
    Present(AttributeDescription),
    // TODO: Substring(AttributeDescription, ...
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
                match filtertype {
                    FilterType::Equal => entry.get(attr).any(|value| value == filtervalue),
                    _ => false,
                }
            }
        }
    }
}
