use log::*;
use std::io;
use std::io::Write;

pub use log::{debug, error, info, trace, warn, LevelFilter};

fn loglevel_ansi_color(level: Level) -> &'static str {
    match level {
        Level::Error => "\x1B[1;31m", // Red
        Level::Warn => "\x1B[1;33m",  // Yellow
        Level::Info => "\x1B[1;34m",  // Blue
        Level::Debug => "\x1B[1;35m", // Magenta
        Level::Trace => "\x1B[1;36m", // Cyan
    }
}

pub struct Logger {
    ///  include the file:line of the log call
    pub show_location: bool,
    //// Maximum log level visible. Useful for removing debug and trace calls in release builds
    pub max_level: LevelFilter,
}

impl Default for Logger {
    fn default() -> Self {
        Self {
            show_location: true,
            max_level: LevelFilter::Debug,
        }
    }
}

impl Logger {
    /// Installs the logger. No log messages with a severity level lower than `level` will be
    /// printed.
    pub fn install(self) {
        let level = self.max_level;
        log::set_boxed_logger(Box::new(self))
            .map(|()| log::set_max_level(level))
            .expect("Failed to install ivy-logger");
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let (mut stdin_read, mut stderr_read);

        let color = loglevel_ansi_color(record.level());
        // let mut writer = File::create("tmp").unwrap();
        let writer: &mut dyn Write = if record.level() >= Level::Warn {
            stderr_read = io::stderr();
            &mut stderr_read
        } else {
            stdin_read = io::stdout();
            &mut stdin_read
        };

        if self.show_location {
            writeln!(
                writer,
                "{color}{}\x1B[0;0m {}:{} - {}",
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args(),
                color = color
            )
        } else {
            writeln!(
                writer,
                "{color}{}\x1B[0;0m - {}",
                record.level(),
                record.args(),
                color = color
            )
        }
        .expect("Failed to write log message to stream");
    }

    fn flush(&self) {}
}
