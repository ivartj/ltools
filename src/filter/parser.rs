use nom::{
    IResult,
    AsChar,
    character::complete::{ satisfy, char },
    bytes::complete::tag,
    branch::alt,
    sequence::{ preceded, pair, tuple, delimited },
    multi::{ fold_many0, many1 },
    combinator::map,
};
use crate::filter::{Filter, FilterType, AttributeDescription};

fn attribute_type(input: &str) -> IResult<&str, String> {
    let (input, start_char) = satisfy(|c| c.is_ascii_alphabetic())(input)?;
    let start_char = start_char.to_ascii_lowercase();
    fold_many0(
        satisfy(|c| c.is_ascii_alphanumeric() || c == '-'),
        move || start_char.to_string(),
        |mut s, c| { s.push(c.to_ascii_lowercase()); s},
    )(input)
}

fn filter_type(input: &str) -> IResult<&str, FilterType> {
    alt((
        map(tag("="), |_| FilterType::Equal),
        map(tag("~="), |_| FilterType::Approx),
        map(tag(">="), |_| FilterType::GreaterOrEqual),
        map(tag("<="), |_| FilterType::LessOrEqual),
    ))(input)
}

fn hex_digit_value(c: char) -> u8 {
    let c = c as u8;
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!(),
    }
}

fn attribute_value(input: &str) -> IResult<&str, Vec<u8>> {
    fold_many0(
        alt((
            map(
                preceded(
                    char('\\'),
                    pair(satisfy(AsChar::is_hex_digit), satisfy(AsChar::is_hex_digit))
                ),
                |(fst, snd)| vec![hex_digit_value(fst) * 16u8 + hex_digit_value(snd)]
            ),
            map(
                satisfy(|c| !"\0()*\x1b".chars().any(|b| b == c)),
                |c| { let mut v = Vec::new(); v.extend(c.encode_utf8(&mut [0u8;4]).as_bytes()); v }
            )
        )),
        Vec::new,
        |mut v, bytes| { v.extend(bytes); v },
    )(input)
}

fn simple_filter(input: &str) -> IResult<&str, Filter> {
    map(tuple((char('('), attribute_type, filter_type, attribute_value, char(')'))),
        |(_,atype, ftype, avalue, _)| {
            Filter::Simple(
                AttributeDescription{
                    attribute_type: atype,
                },
                ftype,
                avalue
            )
        })(input)
}

fn present_filter(input: &str) -> IResult<&str, Filter> {
    map(tuple((char('('), attribute_type, tag("=*)"))),
        |(_,atype, _)| {
            Filter::Present(
                AttributeDescription{
                    attribute_type: atype,
                },
            )
        })(input)
}

fn not_filter(input: &str) -> IResult<&str, Filter> {
    map(delimited(tag("(!"), filter, char(')')),
        |inner_filter| Filter::Not(Box::new(inner_filter)),
    )(input)
}

fn and_filter(input: &str) -> IResult<&str, Filter> {
    map(delimited(tag("(&"), many1(filter), char(')')),
        Filter::And
    )(input)
}

fn or_filter(input: &str) -> IResult<&str, Filter> {
    map(delimited(tag("(|"), many1(filter), char(')')),
        Filter::Or
    )(input)
}

pub fn filter(input: &str) -> IResult<&str, Filter> {
    alt((
        simple_filter,
        present_filter,
        not_filter,
        and_filter,
        or_filter,
    ))(input)
}


#[cfg(test)]
mod test {
    use super::*;

    impl AttributeDescription {
        fn new(attribute_type: String) -> AttributeDescription {
            AttributeDescription{
                attribute_type,
            }
        }
    }

    #[test]
    fn test_filter_type() {
        assert_eq!(filter_type("="), Ok(("", FilterType::Equal)));
        assert_eq!(filter_type("~="), Ok(("", FilterType::Approx)));
        assert_eq!(filter_type(">="), Ok(("", FilterType::GreaterOrEqual)));
        assert_eq!(filter_type("<="), Ok(("", FilterType::LessOrEqual)));
    }

    #[test]
    fn test_attribute_value() {
        assert_eq!(attribute_value("(\0\x1b)*"), Ok(("(\0\x1b)*", vec![])));
        assert_eq!(attribute_value("\\1b\\00foo"), Ok(("", vec![b'\x1b', b'\0', b'f', b'o', b'o'])));
    }

    #[test]
    fn test_simple_filter() {
        assert_eq!(filter("(ou=sa)"), Ok(("", Filter::Simple(AttributeDescription::new(String::from("ou")), FilterType::Equal, vec![b's', b'a']))));
    }

    #[test]
    fn test_present_filter() {
        assert_eq!(filter("(ou=*)"), Ok(("", Filter::Present(AttributeDescription::new(String::from("ou"))))));
    }

    #[test]
    fn test_not_filter() {
        assert_eq!(filter("(!(ou=*))"), Ok(("", Filter::Not(Box::new(Filter::Present(AttributeDescription::new(String::from("ou"))))))));
    }

    #[test]
    fn test_and_filter() {
        assert_eq!(
            filter("(&(f=*)(o>=o))"),
            Ok(("", Filter::And(vec![
                Filter::Present(AttributeDescription::new(String::from("f"))),
                Filter::Simple(AttributeDescription::new(String::from("o")), FilterType::GreaterOrEqual, vec![b'o']),
            ]))));
    }
}
