use clap::{arg, command, Arg};
use ltools::base64::{DecodeState, DecodeWriter};
use ltools::crstrip::CrStripper;
use ltools::lexer::{Lexer, WriteToken, Token, TokenKind};
use ltools::loc::WriteLocWrapper;
use ltools::unfold::Unfolder;
use ltools::tsv::TsvEntryWriter;
use ltools::csv::CsvEntryWriter;
use ltools::json::JsonEntryWriter;
use ltools::entry::EntryTokenWriter;
use ltools::attrspec::AttrSpec;
use std::io::{copy, stdin, stdout, Write};

#[derive(PartialEq)]
enum ValueType {
    Text,
    Base64,
}

struct OctetStreamTokenWriter<W: Write> {
    attrtype: String,
    ismatch: bool,
    dest: W,
    valuetype: ValueType,
    b64state: DecodeState,
    delimiter: u8,
}

impl<W: Write> OctetStreamTokenWriter<W> {
    fn new(attrtype: &str, dest: W) -> OctetStreamTokenWriter<W> {
        OctetStreamTokenWriter {
            attrtype: attrtype.to_ascii_lowercase(),
            ismatch: true, // true until non-matching char is seen
            dest,
            valuetype: ValueType::Text,
            b64state: DecodeState::default(),
            delimiter: b'\n',
        }
    }

    fn set_delimiter(&mut self, delimiter: u8) -> &mut Self {
        self.delimiter = delimiter;
        self
    }
}

impl<W: Write> WriteToken for OctetStreamTokenWriter<W> {
    fn write_token(&mut self, token: Token) -> std::io::Result<()> {
        match token.kind {
            TokenKind::AttributeType => {
                self.ismatch =
                    token.segment.to_ascii_lowercase() == self.attrtype;
            }
            TokenKind::ValueText => {
                if self.ismatch {
                    self.dest.write_all(token.segment.as_bytes())?;
                    self.valuetype = ValueType::Text;
                }
            }
            TokenKind::ValueBase64 => {
                if self.ismatch {
                    let mut decoder = DecodeWriter::new_with_state(&mut self.dest, self.b64state);
                    decoder.write_all(token.segment.as_bytes())?;
                    self.b64state = decoder.get_state();
                    self.valuetype = ValueType::Base64;
                }
            }
            TokenKind::ValueFinish => {
                if self.ismatch {
                    if self.valuetype == ValueType::Base64 {
                        // TODO: consider raising an error if it isn't in a valid end state
                        self.b64state = DecodeState::default();
                    }
                    self.dest.write_all(&[self.delimiter])?;
                    self.dest.flush()?;
                }
                self.ismatch = true;
            }
            TokenKind::EntryFinish => {}
        }
        Ok(())
    }
}

#[derive(PartialEq, Eq)]
enum OutputFormat {
    Tsv,
    Csv,
    Json,
}

fn parse_arguments() -> Result<(Vec<String>, u8, OutputFormat), &'static str> {
    let mut delimiter = b'\n';
    let mut output_format = OutputFormat::Tsv;

    let matches = command!("lget")
        .disable_colored_help(true)
        .arg(arg!(<ATTRIBUTES> ... "The attribute type names to get values of."))
        .arg(
            Arg::new("null-delimit")
                .short('0')
                .long("null-delimit")
                .action(clap::ArgAction::SetTrue)
                .help("Terminate output values with null bytes (0x00) instead of newlines."),
        )
        .arg(Arg::new("json")
             .short('j')
             .long("json")
             .action(clap::ArgAction::SetTrue)
             .help("Write specified attributes for each entry as a JSON object with string array values."),
        )
        .arg(Arg::new("csv")
             .short('c')
             .long("csv")
             .action(clap::ArgAction::SetTrue)
             .help("Write values using the CSV format, including a header."),
        )
        .get_matches();

    if matches.get_flag("null-delimit") {
        delimiter = 0x00;
    }

    let output_formats = ["json", "csv"];
    if output_formats.iter().filter(|format| matches.get_flag(format)).count() > 1 {
        return Err("options specify mutually exclusive output formats")
    }
    if matches.get_flag("json") {
        output_format = OutputFormat::Json;
    }
    if matches.get_flag("csv") {
        output_format = OutputFormat::Csv;
    }

    if let Some(attrtype) = matches.get_many::<String>("ATTRIBUTES") {
        Ok((attrtype.cloned().collect(), delimiter, output_format))
    } else {
        // shouldn't happen when the argument is required
        Err("missing attribute type name on command line")
    }
}

fn write_tokens<TR: WriteToken>(tr: TR) -> std::io::Result<()> {
    let lexer = Lexer::new(tr);
    let unfolder = Unfolder::new(lexer);
    let crstripper = CrStripper::new(unfolder);
    let mut wrapper = WriteLocWrapper::new(crstripper);
    copy(&mut stdin(), &mut wrapper)?;
    wrapper.flush()?;
    Ok(())
}

fn get_result() -> Result<(), Box<dyn std::error::Error>> {
    let (attrspec_strings, delimiter, output_format) = parse_arguments()?;
    let mut attrspecs: Vec<AttrSpec> = Vec::new();
    for spec in attrspec_strings.iter() {
        attrspecs.push(AttrSpec::parse(spec)?);
    }
    if attrspecs.len() == 1
        && attrspecs[0].value_filters.is_empty()
        && output_format == OutputFormat::Tsv
    {
        let mut token_receiver = OctetStreamTokenWriter::new(&attrspecs[0].attribute.to_ascii_lowercase(), stdout());
        token_receiver.set_delimiter(delimiter);
        write_tokens(token_receiver)?;
    } else {
        let attributes = attrspecs.iter()
            .map(|spec| spec.attribute.to_ascii_lowercase())
            .collect();
        match output_format {
            OutputFormat::Tsv => {
                let mut entry_writer = TsvEntryWriter::new(attrspecs, stdout());
                entry_writer.set_record_separator(delimiter);
                let token_writer = EntryTokenWriter::new(attributes, &mut entry_writer);
                write_tokens(token_writer)?;
            },
            OutputFormat::Csv => {
                let mut entry_writer = CsvEntryWriter::new(attrspecs, stdout());
                let token_writer = EntryTokenWriter::new(attributes, &mut entry_writer);
                write_tokens(token_writer)?;
            },
            OutputFormat::Json => {
                let mut entry_writer = JsonEntryWriter::new(attrspecs, stdout());
                entry_writer.set_record_separator(delimiter);
                let mut token_writer = EntryTokenWriter::new(attributes, &mut entry_writer);
                token_writer.set_ignore_entries_without_dn(true);
                write_tokens(token_writer)?;
            },
        }
    };
    Ok(())
}

fn main() {
    let result = get_result();
    if let Err(err) = result {
        eprintln!("lget: {}", err);
        std::process::exit(1);
    }
}
