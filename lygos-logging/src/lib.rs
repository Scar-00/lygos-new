use ariadne::{CharSet, Color, Config, Label, Report, ReportKind, sources};
use lygos_token::Location;
use std::{collections::HashMap, fs, io::IsTerminal, ops::Range, path::Path};

#[derive(Clone, Debug)]
pub struct ErrorLabel {
    loc: Location,
    msg: String,
}

impl ErrorLabel {
    pub fn new(path: impl AsRef<Path>, range: Range<usize>, msg: impl ToString) -> Self {
        Self {
            loc: Location::new(path.as_ref().to_path_buf(), range),
            msg: msg.to_string(),
        }
    }
    pub fn from(loc: &Location, msg: impl ToString) -> Self {
        Self {
            loc: loc.clone(),
            msg: msg.to_string(),
        }
    }
}

impl<'a> From<&'a ErrorLabel> for Label<(String, Range<usize>)> {
    fn from(value: &'a ErrorLabel) -> Self {
        Label::new((
            value.loc.file.display().to_string(),
            value.loc.range.clone(),
        ))
        .with_message(&value.msg)
        .with_color(Color::Red)
    }
}

pub struct Logger;

impl Logger {
    /// Render a diagnostic without exiting, panicking, or depending on a terminal.
    /// Missing source files fall back to a textual report preserving every label.
    pub fn render_diagnostic(
        message: &str,
        labels: &[ErrorLabel],
        help: Option<&str>,
        note: Option<&str>,
        color: bool,
    ) -> String {
        let fallback = || {
            let mut report = format!("Error: {message}\n");
            for label in labels {
                report += &format!(
                    "  {}:{}: {}\n",
                    label.loc.file.display(),
                    label.loc.range.start,
                    label.msg
                );
            }
            if let Some(note) = note {
                report += &format!("Note: {note}\n");
            }
            if let Some(help) = help {
                report += &format!("Help: {help}\n");
            }
            report
        };
        let mut source_map = HashMap::new();
        let mut visible = Vec::new();
        for (index, label) in labels.iter().enumerate() {
            let path = label.loc.file.display().to_string();
            if path.is_empty() || path == "internal" {
                continue;
            }
            let content = if let Some(content) = source_map.get(&path) {
                content
            } else {
                let Ok(content) = fs::read_to_string(&label.loc.file) else {
                    return fallback();
                };
                source_map.entry(path.clone()).or_insert(content)
            };
            let length = content.chars().count();
            let start = label.loc.range.start.min(length);
            let end = label.loc.range.end.max(start).min(length);
            visible.push((index, path, start..end, label.msg.clone()));
        }
        let Some((_, path, range, _)) = visible.first() else {
            return fallback();
        };
        let mut builder = Report::build(ReportKind::Error, (path.clone(), range.clone()))
            .with_message(message)
            .with_config(
                Config::default()
                    .with_char_set(CharSet::Unicode)
                    .with_color(color)
                    .with_multiline_arrows(false),
            );
        for (index, path, range, message) in visible {
            builder.add_label(
                Label::new((path, range))
                    .with_message(message)
                    .with_color(if index == 0 {
                        Color::Red
                    } else {
                        Color::Yellow
                    })
                    .with_order(index as i32),
            );
        }
        if let Some(help) = help {
            builder = builder.with_help(help);
        }
        if let Some(note) = note {
            builder = builder.with_note(note);
        }
        let mut output = Vec::new();
        if builder
            .finish()
            .write(sources(source_map), &mut output)
            .is_err()
        {
            return fallback();
        }
        String::from_utf8(output).unwrap_or_else(|_| fallback())
    }

    pub fn emit_diagnostic(
        message: impl AsRef<str>,
        labels: &[ErrorLabel],
        help: Option<&str>,
        note: Option<&str>,
    ) {
        let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        let report = Self::render_diagnostic(message.as_ref(), labels, help, note, color);
        use std::io::Write;
        let _ = std::io::stderr().write_all(report.as_bytes());
    }

    pub fn diagnostic(
        message: impl AsRef<str>,
        labels: &[ErrorLabel],
        help: Option<&str>,
        note: Option<&str>,
    ) -> ! {
        Self::emit_diagnostic(message, labels, help, note);
        std::process::exit(1);
    }

    pub fn token_expected(loc: &Location, msg: impl AsRef<str>, info: impl AsRef<str>) -> ! {
        Self::diagnostic(msg, &[ErrorLabel::from(loc, info.as_ref())], None, None)
    }
    pub fn token_expected_help(
        loc: &Location,
        msg: impl AsRef<str>,
        info: impl AsRef<str>,
        help: impl AsRef<str>,
    ) -> ! {
        Self::diagnostic(
            msg,
            &[ErrorLabel::from(loc, info.as_ref())],
            Some(help.as_ref()),
            None,
        )
    }
    pub fn error_msg(path: impl AsRef<Path>, msg: impl AsRef<str>) -> ! {
        Self::diagnostic(
            msg.as_ref(),
            &[ErrorLabel::new(path, 0..0, msg.as_ref())],
            None,
            None,
        )
    }
    pub fn error_msg_label(msg: impl AsRef<str>, label: ErrorLabel) -> ! {
        Self::diagnostic(msg, &[label], None, None)
    }
    pub fn error_msg_label_info(
        msg: impl AsRef<str>,
        label: ErrorLabel,
        info: impl AsRef<str>,
    ) -> ! {
        Self::diagnostic(msg, &[label], None, Some(info.as_ref()))
    }
    pub fn error_msg_labels(msg: impl AsRef<str>, labels: &[ErrorLabel]) -> ! {
        Self::diagnostic(msg, labels, None, None)
    }
}
