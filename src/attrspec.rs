use std::borrow::Cow;
use nom::Err;
use nom::sequence::terminated;
use nom::combinator::eof;
use crate::entry::EntryValue;
use crate::base64::EncodeWriter;
use std::ops::Deref;
use std::io::Write;

pub struct AttrSpec {
    pub attribute: String, // in original case
    pub attribute_lowercase: String,
    pub value_filters: Vec<ValueFilter>,
}

impl AttrSpec {
    pub fn parse(input: &str) -> std::io::Result<AttrSpec> {
        let iresult = terminated(parser::attr_spec, eof)(input)
            .map(|(_, spec)| spec);
        match iresult {
            Err(err) => {
                let parser_location = match err {
                    Err::Error(e) => e.input,
                    Err::Failure(e) => e.input,
                    Err::Incomplete(_) => unreachable!("unreachable"),
                };
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to parse '{}' starting at '{}'", input, parser_location),
                ))
            },
            Ok(attrspec) => { Ok(attrspec) },
        }
    }

    pub fn filter_values<'a, 'b>(&'a self, values: impl Iterator<Item = &'b [u8]>) -> Cow<'a, Vec<EntryValue<'b>>> {
        let values: Vec<EntryValue<'b>> = values.map(|value: &[u8]| Cow::Owned(Vec::from(value))).collect();
        let mut values: Cow<Vec<EntryValue<'b>>> = Cow::Owned(values);
        for filter in self.value_filters.iter() {
            values = filter.filter_values(values);
        }
        values
    }
}

pub enum ValueFilter {
    NullCoalesce(Vec<EntryValue<'static>>), // static because values are never borrowed
    Base64,
    Hex,
}

impl ValueFilter {
    pub fn filter_values<'a, 'b, 'c>(&'a self, values: Cow<'b, Vec<EntryValue<'c>>>) -> Cow<'b, Vec<EntryValue<'c>>>
        where 'a: 'b
    {
        match self {
            ValueFilter::NullCoalesce(default_values) => {
                if values.is_empty() {
                    Cow::Borrowed(default_values)
                } else {
                    values
                }
            },
            ValueFilter::Base64 => {
                Cow::Owned(
                    values.deref().iter().map(|value| {
                        let mut buf: Vec<u8> = Vec::new();
                        let mut base64encoder = EncodeWriter::new(&mut buf);
                        base64encoder.write_all(&value.deref()[..]).unwrap();
                        base64encoder.flush().unwrap();
                        Cow::Owned(buf)
                    }).collect()
                )
            },
            ValueFilter::Hex => {
                Cow::Owned(
                    values.deref().iter().map(|value| {
                        let mut buf: Vec<u8> = Vec::new();
                        for byte in value.iter() {
                            _ = write!(&mut buf, "{:02x}", byte);
                        }
                        Cow::Owned(buf)
                    }).collect()
                )
            },
        }
    }
}

mod parser {
    use super::*;
    use nom::{
        IResult,
        combinator::map,
        multi::{ fold_many0, many0 },
        branch::alt,
        sequence::{ pair, preceded },
        bytes::complete::{ tag, take_while },
        character::complete::{
            satisfy,
            digit1,
            char,
        },
    };

    pub(super) fn attr_spec(input: &str) -> IResult<&str, AttrSpec> {
        map(
            pair(attribute, many0(value_filter)),
            |(attribute, value_filters)| AttrSpec{
                attribute_lowercase: attribute.to_ascii_lowercase(),
                attribute,
                value_filters
            },
        )(input)
    }

    fn attribute(input: &str) -> IResult<&str, String> {
        alt((attribute_name, attribute_oid))(input)
    }

    fn attribute_name(input: &str) -> IResult<&str, String> {
        // Underscores are not legal in LDAP attribute type names, but we allow
        // them here because they appear in attributes such as loaded_class_count
        // under NetIQ IDM's cn=jvm_stats,cn=monitor subtree.
        let (input, start_char) = satisfy(|c| c.is_ascii_alphabetic())(input)?;
        fold_many0(
            satisfy(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            move || start_char.to_string(),
            |mut s, c| { s.push(c); s},
        )(input)
    }

    fn attribute_oid(input: &str) -> IResult<&str, String> {
        let (input, start_number) = digit1(input)?;
        fold_many0(
            map(pair(char('.'), digit1), |(dot, number)| String::from(dot) + number),
            move || start_number.to_string(),
            |mut s, c| { s.push_str(&c); s },
        )(input)
    }

    fn value_filter(input: &str) -> IResult<&str, ValueFilter> {
        alt((null_coalesce, base64, hex))(input)
    }

    fn null_coalesce(input: &str) -> IResult<&str, ValueFilter> {
        map(
            preceded(tag(":-"), take_while(|_| true)),
            |value: &str| ValueFilter::NullCoalesce(vec![Cow::Owned(value.into())]),
        )(input)
    }

    fn base64(input: &str) -> IResult<&str, ValueFilter> {
        map(tag(".base64"), |_| ValueFilter::Base64)(input)
    }

    fn hex(input: &str) -> IResult<&str, ValueFilter> {
        map(tag(".hex"), |_| ValueFilter::Hex)(input)
    }

}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_invalid_input() {
        let result = AttrSpec::parse("#");
        assert!(result.is_err());
    }
}
