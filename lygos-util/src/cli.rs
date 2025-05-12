use std::{ffi::OsString, path::PathBuf};

use clap::Parser as _;
use std::process::Command;

pub trait CommandExt {
    fn display(&self) -> String;
}

impl CommandExt for Command {
    fn display(&self) -> String {
        let mut output = self.get_program().to_os_string();
        self.get_args().for_each(|arg| {
            output.push(" ");
            output.push(arg);
        });
        output.to_string_lossy().to_string()
    }
}

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
