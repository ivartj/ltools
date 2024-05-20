use clap::{arg, command};
use ltools::crstrip::CrStripper;
use ltools::lexer::Lexer;
use ltools::loc::WriteLocWrapper;
use ltools::unfold::Unfolder;
use ltools::entry::{Entry, OwnedEntry, WriteEntry, EntryTokenWriter, write_entry_normally};
use ltools::filter::Filter;
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

    let filter: Filter = match matches.get_one::<String>("FILTER") {
        None => return Err("missing argument FILTER"),
        Some(filter) => match Filter::parse(filter) {
            Ok(filter) => filter,
            Err(_) => return Err("failed to parse filter"),
        },
    };

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

impl WriteEntry for LFilter {
    fn write_entry(&mut self, entry: &Entry) -> std::io::Result<()> {
        if let Some(ref mut unmatched_output) = self.unmatched_output {
            if self.filter.is_match(entry) {
                self.matched_entries.push(entry.into()); // defer writing matched entries so that
                                                         // they don't potentially interleave the
                                                         // unmatched entries if user passes
                                                         // something like >(cat) as output file
                Ok(())
            } else {
                write_entry_normally(unmatched_output, entry)
            }
        } else if self.filter.is_match(entry) {
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
