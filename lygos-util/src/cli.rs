use std::{ffi::{OsStr, OsString}, path::PathBuf};

use clap::{Parser, ValueEnum};
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
        output.display().to_string()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum EmitType {
    LlvmIr,
    AST,
}

#[derive(Debug, Parser)]
pub struct CliArgs {
    #[arg(global = true)]
    pub input: Option<PathBuf>,
    #[arg(short, long, global = true)]
    pub output: Option<PathBuf>,
    #[arg(value_enum, short, long, global = true)]
    pub emit_type: Option<EmitType>
}

impl CliArgs {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    pub fn parse_from<I, T>(iter: I) -> Self
        where
            I: IntoIterator<Item = T>,
            T: Into<OsString> + Clone,
    {
        <Self as Parser>::parse_from(iter)
    }
}
