use clap::{arg, command, ArgAction};
use ltools::crstrip::CrStripper;
use ltools::lexer::Lexer;
use ltools::loc::WriteLocWrapper;
use ltools::unfold::Unfolder;
use ltools::entry::{Entry, WriteEntry, EntryTokenWriter, write_attrval};
use std::io::{copy, Read, Write, Stdout};
use std::process::{Command, Stdio};

struct EntryProcessor<W: Write> {
    command: Command,
    output: W,
    attrs: Option<Vec<String>>,
}

impl<W: Write> EntryProcessor<W> {
    fn should_process_attr(&self, attr: &str) -> bool {
        if let Some(ref attrs) = self.attrs {
            attrs.iter().any(|arg_attr| arg_attr == attr)
        } else {
            true
        }
    }
}

fn parse_arguments() -> Result<EntryProcessor<Stdout>, &'static str> {

    let matches = command!("lprocess")
        .disable_colored_help(true)
        .allow_external_subcommands(true)
        .arg(arg!(ATTRIBUTE: -a --attribute <ATTRIBUTE> "Limit processing to the given attribute(s). Multiple attributes can be provided either by space-separating them or by providing this option multiple times.")
            .required(false)
            .value_delimiter(' ')
            .action(ArgAction::Append))
        .get_matches();

    let command: Command = if let Some((subcommand, args)) = matches.subcommand() {
        let args: Vec<String> = match args.get_many::<String>("") {
            Some(args) => args.cloned().collect(),
            None => vec![],
        };
        let mut command = Command::new(subcommand);
        command.args(args)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped());
        command
    } else {
        return Err("missing argument SUBCOMMAND");
    };

    let attrs: Option<Vec<String>> = matches.get_many::<String>("ATTRIBUTE")
        .map(|attrs| attrs.map(|attr| attr.to_lowercase()).collect());

    Ok(EntryProcessor{
        command,
        output: std::io::stdout(),
        attrs,
    })
}

fn process_value(command: &mut Command, value: &[u8]) -> std::io::Result<Vec<u8>> {
    let process = command.spawn()?;
    let mut value: Vec<u8> = value.into();
    if let (Some(mut stdin), Some(mut stdout)) = (process.stdin, process.stdout) {
        stdin.write_all(value.as_slice())?;
        drop(stdin);
        value.clear();
        stdout.read_to_end(&mut value)?;
    }
    Ok(value)
}

impl<W: Write> WriteEntry for EntryProcessor<W> {
    fn write_entry(&mut self, entry: &Entry) -> std::io::Result<()> {
        if let Some(dn) = entry.get_one("dn") {
            if self.should_process_attr("dn") {
                let dn = process_value(&mut self.command, dn)?;
                write_attrval(&mut self.output, "dn", dn.as_slice())?;
            } else {
                write_attrval(&mut self.output, "dn", dn)?;
            }
        }
        for attr in entry.attributes() {
            if attr == "dn" {
                continue;
            }
            let should_process_attr = self.should_process_attr(attr);
            for value in entry.get(attr) {
                if should_process_attr {
                    let value = process_value(&mut self.command, value)?;
                    write_attrval(&mut self.output, attr, value.as_slice())?;
                } else {
                    write_attrval(&mut self.output, attr, value)?;
                }
            }
        }
        self.output.write_all(b"\n")?;
        Ok(())
    }
}

fn get_result() -> Result<(), Box<dyn std::error::Error>> {
    let mut processor = parse_arguments()?;
    let token_writer = EntryTokenWriter::new(&mut processor);
    let lexer = Lexer::new(token_writer);
    let unfolder = Unfolder::new(lexer);
    let crstripper = CrStripper::new(unfolder);
    let mut wrapper = WriteLocWrapper::new(crstripper);
    copy(&mut std::io::stdin(), &mut wrapper)?;
    wrapper.flush()?;
    Ok(())
}

fn main() {
    let result = get_result();
    if let Err(err) = result {
        eprintln!("lcompare: {}", err);
        std::process::exit(1);
    }
}
