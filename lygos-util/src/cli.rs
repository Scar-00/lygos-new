use std::{ffi::{OsStr, OsString}, path::PathBuf};

use clap::{Parser as _, ValueEnum};
use std::process::Command;

pub trait CommandExt {
    fn from_array<S: AsRef<OsStr>>(iter: impl IntoIterator<Item = S>) -> Self;
    fn display(&self) -> String;
}

impl CommandExt for Command {
    fn from_array<S: AsRef<OsStr>>(iter: impl IntoIterator<Item = S>) -> Self {
        let mut iter = iter.into_iter();
        let program = iter.next();
        let mut cmd = Command::new(program.as_ref().map(AsRef::as_ref).unwrap_or(OsStr::new("")));
        iter.for_each(|arg| { cmd.arg(arg); });
        cmd
    }

    fn display(&self) -> String {
        let mut output = self.get_program().to_os_string();
        self.get_args().for_each(|arg| {
            output.push(" ");
            output.push(arg);
        });
        output.to_string_lossy().to_string()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum EmitType {
    LlvmIr,
    AST,
}

#[derive(Debug, clap::Parser)]
pub struct CliArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(value_enum, short, long)]
    pub emit_type: Option<EmitType>
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
