use std::thread;
use std::sync::{Arc, Mutex, MutexGuard};
use std::collections::HashSet;
use log::Level;
use colored::{Color, Colorize};

struct LogWorker {
    handle: Option<thread::JoinHandle<()>>,
}

pub struct AsyncLogger {
    worker: Arc<Mutex<LogWorker>>,
    whitelist: HashSet<String>,
}


impl AsyncLogger {
    pub fn new() -> Self {
        let messages: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let worker: Arc<Mutex<LogWorker>> = Arc::new(Mutex::new(LogWorker {
            handle: None,
        }));
        
        let thread_handle = thread::spawn(move || {
            loop {
                let mut messages: MutexGuard<Vec<String>> = messages.lock().expect("Could not lock messages");
                for message in messages.drain(..) {
                    println!("{}", message);
                }
            }
        });

        worker.lock().expect("Could not lock log worker").handle = Some(thread_handle);

        // Hook into process exit
        let logger = AsyncLogger {
            worker,
            whitelist: HashSet::new(),
        };
        logger.install_hooks();
        logger
    }

    fn install_hooks(&self) {
        std::panic::set_hook(Box::new(|info| {
            // 1. Extract panic message (works for all payload types)
            let msg = match info.payload().downcast_ref::<&str>() {
                Some(s) => s.to_string(),
                None => match info.payload().downcast_ref::<String>() {
                    Some(s) => s.clone(),
                    None => "<unknown error>".to_string(),
                },
            };

            // 2. Format location (works in release mode)
            let location = info.location().map(|loc| {
                format!("{}:{}", loc.file(), loc.line())
            }).unwrap_or_else(|| "<unknown location>".to_string());

            // 3. Print exactly ONCE with clear formatting
            eprintln!("\n=== PANIC ===");
            eprintln!("Thread panicked at '{}'", msg);
            eprintln!("Location: {}\n", location);

            // 4. Immediate hard exit (no duplicate handlers)
            std::process::abort(); // Use abort() instead of exit() to prevent unwind
        }));
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

        let color: Color = match record.level() {
            Level::Error => Color::Red,
            Level::Warn => Color::Yellow,
            Level::Info => Color::Green,
            Level::Debug => Color::Cyan,
            Level::Trace => Color::White,
        };

        let target: &String = match (record.module_path(), record.line()) {
            (Some(module_path), Some(line_number)) => &format!("@ {module_path}:{line_number} "),
            (Some(module_path), None) => &format!("@ {module_path} "),
            (None, Some(line_number)) => &format!("@ {line_number} "),
            (None, None) => &"".to_string(),
        };

        println!(
            "{} {} {}| {}",
            chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            record.level().to_string().color(color),
            target,
            record.args().to_string().color(color),
        );
    }

    fn flush(&self) {}
}

impl Drop for AsyncLogger {
    fn drop(&mut self) {
        self.worker.lock().expect("Could not lock worker").handle.take();
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

