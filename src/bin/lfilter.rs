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
    matched_output: Option<Box<dyn Write>>,
    unmatched_output: Option<Stdout>,
    matched_entries: Vec<OwnedEntry>,
    found_match: bool,
}

fn parse_arguments() -> Result<LFilter, &'static str> {

    let mut matches = command!("lfilter")
        .disable_colored_help(true)
        .arg(arg!(<FILTER> "LDAP filter."))
        .arg(arg!([OUTPUT] "Output file for matched entries. Non-matched entries will be written to standard output."))
        .arg(arg!(-q --quiet "Do not output to standard output unless it is specified as an explicit output.")
            .action(clap::ArgAction::SetTrue))
        .get_matches();

    let filter: Filter = match matches.get_one::<String>("FILTER") {
        None => return Err("missing argument FILTER"),
        Some(filter) => match Filter::parse(filter) {
            Ok(filter) => filter,
            Err(_) => return Err("failed to parse filter"),
        },
    };

    let mut quiet = false;
    if matches.get_flag("quiet") {
        quiet = true;
    }

    let matched_output: Option<String> = matches.remove_one::<String>("OUTPUT");
    let mut unmatched_output = None;
    let matched_output: Option<Box<dyn Write>> = match matched_output {
        None => if quiet { None } else { Some(Box::new(std::io::stdout())) },
        Some(filepath) => {
            unmatched_output = if quiet { None } else { Some(std::io::stdout()) };
            match File::create(filepath) {
                Ok(file) => Some(Box::new(file)),
                Err(_) => return Err("Failed to open output file"),
            }
        }
    };

    Ok(LFilter{
        filter,
        matched_output,
        unmatched_output,
        matched_entries: Vec::new(),
        found_match: false,
    })
}

impl WriteEntry for LFilter {
    fn write_entry(&mut self, entry: &Entry) -> std::io::Result<()> {
        if self.filter.is_match(entry) {
            self.found_match = true;
            if self.unmatched_output.is_some() {
                self.matched_entries.push(entry.into()); // defer writing matched entries so that
                                                         // they don't potentially interleave the
                                                         // unmatched entries if user passes
                                                         // something like >(cat) as output file
            } else if let Some(ref mut matched_output) = self.matched_output {
                write_entry_normally(matched_output, entry)?;
            }
        } else if let Some(ref mut unmatched_output) = self.unmatched_output {
            write_entry_normally(unmatched_output, entry)?;
        }
        Ok(())
    }
}

fn get_result() -> Result<i32, Box<dyn std::error::Error>> {
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
    if let Some(mut matched_output) = lfilter.matched_output {
        for entry in lfilter.matched_entries.iter() {
            write_entry_normally(&mut matched_output, entry)?;
        }
        matched_output.flush()?;
    }
    let status = if lfilter.found_match { 0 } else { 1 };
    Ok(status)
}

fn main() {
    let result = get_result();
    match result {
        Err(err) => {
            eprintln!("lfilter: {}", err);
            std::process::exit(2); // based on what grep's man page says
        },
        Ok(status) => {
            std::process::exit(status);
        }
    }
}
