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
        // Hook panics
        std::panic::set_hook({
            let logger = self.worker.clone();
            Box::new(move |_| {
                logger.lock().expect("Could not lock logger").handle.take();
            })
        });

        // Hook process exit
        ctrlc::set_handler({
            let logger = self.worker.clone();
            move || {
                logger.lock().expect("Could not lock logger").handle.take();
                std::process::exit(0);
            }
        }).expect("Could not set ctrlc handler");
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
            (Some(module_path), Some(line_number)) => &format!("[{module_path}@{line_number}] "),
            (Some(module_path), None) => &format!("[{module_path}] "),
            (None, Some(line_number)) => &format!("[@{line_number}] "),
            (None, None) => &"".to_string(),
        };

        println!(
            "{} {} {}{}",
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

