use std::collections::HashSet;
use std::str::Chars;
use log::{Record, Level, Metadata, LevelFilter};
use chrono::Local;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::sync::mpsc;
use colored::{Color, Colorize};

struct LogMessage {
    level: Level,
    target: String,
    line: Option<u32>,
    message: String,
    timestamp: String,
}

#[derive(Debug)]
pub struct CustomLogger {
    sender: Mutex<Option<mpsc::Sender<LogMessage>>>,
    thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
    allowed_modules: RwLock<HashSet<String>>,
}

impl CustomLogger {
    fn new(root_module: &'static str) -> Self {
        let (sender, receiver) = mpsc::channel::<LogMessage>();

        // create separate thread to handle printing so it doesn't block the main thread
        let receiver = Arc::new(Mutex::new(receiver));

        let thread_handle = thread::Builder::new().spawn(move || {
            loop {
                let receiver = receiver.lock().expect("Could not lock receiver");
                if let Ok(log_message) = receiver.recv() {
                    // set color based on log level
                    let color: Color = match log_message.level {
                        Level::Error => Color::Red,
                        Level::Warn => Color::Yellow,
                        Level::Info => Color::Green,
                        Level::Debug => Color::Cyan,
                        Level::Trace => Color::White,
                    };

                    let target: String = match log_message.line {
                        Some(line_number) => format!("{}@{}", log_message.target, line_number),
                        None => log_message.target,
                    };

                    println!(
                        "{} {} [{}] {}",
                        log_message.timestamp,
                        log_message.level.to_string().color(color),
                        target,
                        log_message.message.color(color),
                    );
                } else {
                    // channel disconnected, exit thread
                    break
                }
            }
        }).expect("Failed to spawn logging thread");

        CustomLogger {
            sender: Mutex::new(Some(sender)),
            thread_handle: Mutex::new(Some(thread_handle)),
            allowed_modules: RwLock::new(HashSet::from([root_module.to_string()])),
        }
    }

    pub fn shutdown(&self) {
        // drop sender to close the channel
        self.sender.lock().expect("Could not lock sender").take();

        // wait for the logging thread to finish
        if let Some(handle) = self.thread_handle.lock().expect("Could not lock thread handle").take() {
            handle.join().expect("Logging thread panicked");
        }
    }

    pub fn allow_module(&self, module_name: &str) -> &Self {
        let mut allowed_modules = self.allowed_modules.write()
            .expect("Could not acquire allowed modules list lock");
        allowed_modules.insert(module_name.to_string());
        self
    }

    pub fn disallow_module(&self, module_name: &str) -> &Self {
        let mut allowed_modules = self.allowed_modules.write()
            .expect("Could not acquire allowed modules list lock");
        allowed_modules.remove(&module_name.to_string());
        self
    }

    fn check_target_allowed(&self, target: &str) -> bool {
        let allowed_modules = self.allowed_modules.read()
            .expect("Could not acquire allowed modules list lock");
        
        for module_name in allowed_modules.iter() {
            // check if exactly equal
            if module_name == target { return true }

            // if target is shorter, it can't start with the module name, so not allowed
            if target.len() < module_name.len() { continue }

            // check if starts with "{module_name}::"
            let mut starts_with: bool = true;
            let mut module_name_chars: Chars = module_name.chars();
            let mut target_name_chars: Chars = target.chars();
            // check the "{module_name}"
            for _ in 0..module_name.len() {
                if !(module_name_chars.next() == target_name_chars.next()) {
                    starts_with = false;
                    break
                }
            }
            // check the "::"
            if target_name_chars.next() != Some(':') || target_name_chars.next() != Some(':') {
                starts_with = false;
            }
            if starts_with { return true }
        }
        false
    }
}

impl log::Log for CustomLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        let timestamp: String = Local::now().format("%H:%M:%S%.3f").to_string();
        let target: String = record.target().to_string();
        let line: Option<u32> = record.line();
        let message: String = record.args().to_string();

        // check if module is allowed; if not, don't print
        if !self.check_target_allowed(record.target()) { return }

        // Prepare the log message
        let log_message = LogMessage {
            level: record.level(),
            target,
            line,
            message,
            timestamp,
        };

        // send log message to the thread for printing
        if let Some(sender) = self.sender.lock().expect("Could not lock sender").as_ref() {
            if let Err(e) = sender.send(log_message) {
                eprintln!("Failed to send log message: {}", e);
            }
        }
    }

    fn flush(&self) {}
}

/// Initialize the logger. This function should be called once, in the beginning of your main function.
/// This will initialize the logger to ignore all logs that aren't coming from your program.
/// You can later allow certain modules using `logger.allow_module(module_name)`.
///
/// # Arguments
/// * `root_module`: the name of your program's cargo crate. Should be set to `env!("CARGO_PKG_NAME")`.

/// # Example
/// ```
/// let logger = biologischer_log::init_logger(env!("CARGO_CRATE_NAME"));
/// logger.allow_module("rocket");
/// ```
pub fn init_logger(root_module: &'static str) -> Arc<CustomLogger> {
    let logger = Arc::new(CustomLogger::new(root_module));
    log::set_boxed_logger(Box::new(logger.clone())).expect("Failed to set logger");
    log::set_max_level(LevelFilter::Trace);
    logger
}
