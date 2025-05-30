use std::{
    sync::mpsc::{self, SyncSender, Receiver},
    thread,
    collections::HashSet,
};
use colored::{Color, Colorize};
use log::Level;

pub struct AsyncLogger {
    sender: Option<SyncSender<LogMessage>>,    // wrapped in Option for clean shutdown
    whitelist: HashSet<String>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

#[derive(Debug)]
struct LogMessage {
    message: String,
    level: Level,
    timestamp: String,
    module: String,
    line: Option<u32>,
}

impl AsyncLogger {
    pub fn new() -> Self {
        // Explicitly specify the channel type
        let (sender, receiver): (SyncSender<LogMessage>, Receiver<LogMessage>) = mpsc::sync_channel(100);

        let thread_handle = thread::spawn(move || {
            for msg in receiver {
                let color: Color = match msg.level {
                    Level::Error => Color::Red,
                    Level::Warn => Color::Yellow,
                    Level::Info => Color::Green,
                    Level::Debug => Color::Cyan,
                    Level::Trace => Color::White,
                };

                let target: String = match msg.line {
                    Some(line_number) => format!("{}@{}", msg.module, line_number),
                    None => msg.module,
                };

                println!(
                    "{} {} [{}] {}",
                    msg.timestamp,
                    msg.level.to_string().color(color),
                    target,
                    msg.message.color(color),
                );
            }
        });

        AsyncLogger {
            sender: Some(sender),
            whitelist: HashSet::new(),      // temporarily empty (will be populated in `init()`)
            thread_handle: Some(thread_handle),
        }
    }

    pub fn whitelist_module(&mut self, module: &str) {
        self.whitelist.insert(module.to_string());
    }
}

impl log::Log for AsyncLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        let target: &str = metadata.target();

        // allow if any parent module is whitelisted
        self.whitelist.iter().any(|whitelisted| {
            target.starts_with(whitelisted) &&
                (target.len() == whitelisted.len() || target.as_bytes()[whitelisted.len()] == b':') 
        })
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if let Some(sender) = &self.sender {
            let _ = sender.send(LogMessage {
                module: record.module_path().unwrap_or("unknown").to_string(),
                level: record.level(),
                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                message: record.args().to_string(),
                line: record.line(),
            });
        }
    }

    fn flush(&self) {}
}

impl Drop for AsyncLogger {
    fn drop(&mut self) {
        // close channel by taking the sender
        self.sender.take();

        // wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}


/// Initialize the logger. This function should be called once at the start of your main function.
/// 
/// Example use: `biologischer_log::init(env!("CARGO_CRATE_NAME"))`
pub fn init(crate_name: &str) {
    let mut logger = AsyncLogger::new();
    logger.whitelist_module(&crate_name);   // Auto-whitelist the crate
    log::set_boxed_logger(Box::new(logger)).expect("Failed to set boxed logger");
    log::set_max_level(log::LevelFilter::Info);
}