use std::{ffi::OsString, path::PathBuf};

use clap::Parser as _;

#[derive(Debug, clap::Parser)]
pub struct CliArgs {
    pub input: PathBuf,
}

impl CliArgs {
    pub fn deligate_parse() -> Self {
        Self::parse()
    }

    pub fn deligate_parse_from<I, T>(iter: I) -> Self
        where
            I: IntoIterator<Item = T>,
            T: Into<OsString> + Clone,
    {
        Self::parse_from(iter)
    }
}
