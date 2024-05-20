use clap::{arg, command};
use ltools::crstrip::CrStripper;
use ltools::lexer::Lexer;
use ltools::loc::WriteLocWrapper;
use ltools::unfold::Unfolder;
use ltools::entry::{Entry, OwnedEntry, WriteEntry, EntryTokenWriter, write_attrval};
use ltools::filter::parser::{Filter,FilterType,filter as parse_filter};
use std::fs::File;
use std::io::{copy, Write, Stdout};

struct LFilter {
    filter: Filter,
    matched_output: Box<dyn Write>,
    unmatched_output: Option<Stdout>,
    matched_entries: Vec<OwnedEntry>,
}

fn parse_arguments() -> Result<LFilter, &'static str> {

    let mut matches = command!("lfilter")
        .disable_colored_help(true)
        .arg(arg!(<FILTER> "LDAP filter."))
        .arg(arg!([OUTPUT] "Output file for matched entries. Non-matched entries will be written to standard output."))
        .get_matches();

    let filter: String = match matches.remove_one::<String>("FILTER") {
        None => return Err("Missing argument FILTER"),
        Some(filter) => filter,
    };
    let (remainder, filter) = match parse_filter(&filter) {
        Ok(filter) => filter,
        Err(_) => return Err("failed to parse filter"),
    };
    if remainder.trim() != "" {
        return Err("failed to parse filter");
    }

    let matched_output: Option<String> = matches.remove_one::<String>("OUTPUT");
    let mut unmatched_output = None;
    let matched_output: Box<dyn Write> = match matched_output {
        None => Box::new(std::io::stdout()),
        Some(filepath) => {
            unmatched_output = Some(std::io::stdout());
            match File::create(filepath) {
                Ok(file) => Box::new(file),
                Err(_) => return Err("Failed to open output file"),
            }
        }
    };

    Ok(LFilter{
        filter,
        matched_output,
        unmatched_output,
        matched_entries: Vec::new(),
    })
}

fn is_match(entry: &Entry, filter: &Filter) -> bool {
    match filter {
        Filter::And(filters) => filters.iter()
            .all(|filter| is_match(entry, filter)),
        Filter::Or(filters) => filters.iter()
            .any(|filter| is_match(entry, filter)),
        Filter::Not(filter) => !is_match(entry, filter),
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

fn write_entry_normally<W: Write>(output: &mut W, entry: &Entry) -> std::io::Result<()> {
    if let Some(dn) = entry.get_one("dn") {
        write_attrval(output, "dn", dn)?;
    }

    for attr in entry.attributes().filter(|attr| *attr != "dn") {
        for value in entry.get(attr) {
            write_attrval(output, attr, value)?;
        }
    }
    output.write_all(b"\n")
}

impl WriteEntry for LFilter {
    fn write_entry(&mut self, entry: &Entry) -> std::io::Result<()> {
        if let Some(ref mut unmatched_output) = self.unmatched_output {
            if is_match(entry, &self.filter) {
                self.matched_entries.push(entry.into()); // defer writing matched entries so that
                                                         // they don't potentially interleave the
                                                         // unmatched entries if user passes
                                                         // something like >(cat) as output file
                Ok(())
            } else {
                write_entry_normally(unmatched_output, entry)
            }
        } else if is_match(entry, &self.filter) {
            write_entry_normally(&mut self.matched_output, entry)
        } else {
            Ok(())
        }
    }
}

fn get_result() -> Result<(), Box<dyn std::error::Error>> {
    let mut lfilter = parse_arguments()?;
    let token_writer = EntryTokenWriter::new(&mut lfilter);
    let lexer = Lexer::new(token_writer);
    let unfolder = Unfolder::new(lexer);
    let crstripper = CrStripper::new(unfolder);
    let mut wrapper = WriteLocWrapper::new(crstripper);
    copy(&mut std::io::stdin(), &mut wrapper)?;
    wrapper.flush()?;
    if let Some(ref mut unmatched_output) = lfilter.unmatched_output {
        unmatched_output.flush()?;
    }
    for entry in lfilter.matched_entries.iter() {
        write_entry_normally(&mut lfilter.matched_output, entry)?;
    }
    lfilter.matched_output.flush()?;
    Ok(())
}

fn main() {
    let result = get_result();
    if let Err(err) = result {
        eprintln!("lfilter: {}", err);
        std::process::exit(1);
    }
}
