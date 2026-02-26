use std::{cell::RefCell, fs, path::Path, ops::Range};

use ariadne::{sources, CharSet, Color, Config, Label, Report, ReportKind, Source};
use lygos_token::Location;

thread_local! {
    static LOGGER: RefCell<Logger> = {
        RefCell::new(Logger::new())
    }
}


#[derive(Clone)]
pub struct ErrorLabel {
    loc: Location,
    msg: String,
}

impl ErrorLabel {
    pub fn new(path: impl AsRef<Path>, range: Range<usize>, msg: impl ToString) -> Self {
        Self{ loc: Location::new(path.as_ref().to_path_buf(), range), msg: msg.to_string() }
    }

    pub fn from(loc: &Location, msg: impl ToString) -> Self {
        Self{ loc: loc.clone(), msg: msg.to_string() }
    }
}

impl<'a> From<&'a ErrorLabel> for Label<(String, Range<usize>)> {
    fn from(value: &'a ErrorLabel) -> Self {
        Label::new((value.loc.file.display().to_string(), value.loc.range.clone()))
            .with_message(value.msg.clone())
            .with_color(Color::Red)
    }
}

pub struct Logger {
    config: Config,
}

impl Logger {
    fn new() -> Self {
        Self{
            config: Config::default()
                .with_char_set(CharSet::Unicode)
                .with_color(true)
                .with_multiline_arrows(false),
        }
    }
}

impl Logger {
    fn load_file(path: impl AsRef<Path>) -> String {
        match fs::read_to_string(&path) {
            Ok(content) => {
                content
            }
            Err(e) => {
                panic!("failed to load source file({}): {e}", path.as_ref().display())
            }
        }
    }

    pub fn token_expected(loc: &Location, msg: impl AsRef<str>, info: impl AsRef<str>) -> ! {
        let path = &loc.file.display().to_string();
        Report::build(ReportKind::Error, (path, loc.range.clone()))
            .with_message(msg.as_ref())
            .with_label(
                Label::new((path, loc.range.clone()))
                .with_message(info.as_ref())
                .with_color(Color::Red)
            )
            .with_config(LOGGER.with_borrow(|logger| logger.config))
            .finish()
            .eprint((path, Source::from(Self::load_file(&loc.file))))
            .unwrap();
        std::process::exit(1);
    }

    pub fn token_expected_help(loc: &Location, msg: impl AsRef<str>, info: impl AsRef<str>, help: impl AsRef<str>) -> ! {
        let path = &loc.file.display().to_string();
        Report::build(ReportKind::Error, (path, loc.range.clone()))
            .with_message(msg.as_ref())
            .with_label(
                Label::new((path, loc.range.clone()))
                .with_message(info.as_ref())
                .with_color(Color::Red)
            )
            .with_help(help.as_ref())
            .with_config(LOGGER.with_borrow(|logger| logger.config))
            .finish()
            .eprint((path, Source::from(Self::load_file(&loc.file))))
            .unwrap();
        std::process::exit(1);
    }

    pub fn error_msg(path: impl AsRef<Path>, msg: impl AsRef<str>) -> ! {
        let path = &path.as_ref().display().to_string();
        Report::build(ReportKind::Error, (path, 0..=0))
            .with_message(msg.as_ref())
            .with_config(LOGGER.with_borrow(|logger| logger.config))
            .finish()
            .eprint((path, Source::from(Self::load_file(path))))
            .unwrap();

        std::process::exit(1);
    }

    pub fn error_msg_label(msg: impl AsRef<str>, label: ErrorLabel) -> ! {
        Report::build(ReportKind::Error, ("".to_string(), 0..0))
            .with_message(msg.as_ref())
            .with_label(Label::from(&label))
            .with_config(LOGGER.with_borrow(|logger| logger.config))
            .finish()
            .eprint((
                label.loc.file.to_string_lossy().to_string(),
                Source::from(Self::load_file(label.loc.file))
            ))
            .unwrap();

        std::process::exit(1);
    }

    pub fn error_msg_label_info(msg: impl AsRef<str>, label: ErrorLabel, info: impl AsRef<str>) -> ! {
        Report::build(ReportKind::Error, ("".to_string(), 0..0))
            .with_message(msg.as_ref())
            .with_label(Label::from(&label))
            .with_note(info.as_ref())
            .with_config(LOGGER.with_borrow(|logger| logger.config))
            .finish()
            .eprint((
                label.loc.file.to_string_lossy().to_string(),
                Source::from(Self::load_file(label.loc.file))
            ))
            .unwrap();

        std::process::exit(1);
    }

    pub fn error_msg_labels(msg: impl AsRef<str>, labels: &[ErrorLabel]) -> ! {
        let sources_vec = labels.iter().map(|label| {
            (
                label.loc.file.to_string_lossy().to_string(),
                Self::load_file(&label.loc.file)
            )
        }).collect::<Vec<(String, String)>>();

        let mut builder = Report::build(ReportKind::Error, ("".to_string(), 0..0));
        labels.iter().for_each(|label| {
            builder.add_label(Label::from(label));
        });
        builder.with_message(msg.as_ref())
            .finish()
            .eprint(sources(sources_vec))
            .unwrap();
        std::process::exit(1);
    }
}
