use nom::{
    IResult,
    AsChar,
    character::complete::{ satisfy, char },
    bytes::complete::tag,
    branch::alt,
    sequence::{ preceded, pair },
    multi::fold_many0,
    combinator::{ map },
};

#[derive(Debug)]
enum Filter {
    And(Box<[Filter]>),
    Or(Box<[Filter]>),
    Not(Box<Filter>),
    Simple(AttributeDescription, FilterType, String),
    Present(AttributeDescription),
    // TODO: Substring(AttributeDescription, ...
    // TODO: Extensible(...
}

#[derive(Debug, PartialEq)]
struct AttributeDescription {
    attribute_type: String,
    // TODO: add options
}

#[derive(Debug, PartialEq)]
enum FilterType {
    Equal,
    Approx,
    GreaterOrEqual,
    LessOrEqual,
}

fn attribute_type(input: &str) -> IResult<&str, String> {
    let (input, start_char) = satisfy(|c| c.is_ascii_alphabetic())(input)?;
    fold_many0(
        satisfy(|c| c.is_ascii_alphanumeric()),
        move || start_char.to_string(),
        |mut s, c| { s.push(c); s},
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

fn assertion_value(input: &str) -> IResult<&str, Vec<u8>> {
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
        || Vec::new(),
        |mut v, bytes| { v.extend(bytes); v },
    )(input)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_filter_type() {
        assert_eq!(filter_type("="), Ok(("", FilterType::Equal)));
        assert_eq!(filter_type("~="), Ok(("", FilterType::Approx)));
        assert_eq!(filter_type(">="), Ok(("", FilterType::GreaterOrEqual)));
        assert_eq!(filter_type("<="), Ok(("", FilterType::LessOrEqual)));
    }

    #[test]
    fn test_assertion_value() {
        assert_eq!(assertion_value("(\0\x1b)*"), Ok(("(\0\x1b)*", vec![])));
        assert_eq!(assertion_value("\\1b\\00foo"), Ok(("", vec![b'\x1b', b'\0', b'f', b'o', b'o'])));
    }
}
