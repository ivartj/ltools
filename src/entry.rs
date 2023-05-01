use std::collections::HashMap;
use std::borrow::Cow;

pub type Entry<'a, 'b> = HashMap<String, &'a Vec<EntryValue<'b>>>;

pub type EntryValue<'a> = Cow<'a, Vec<u8>>;


