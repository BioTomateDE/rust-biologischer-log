use std::thread;
use std::sync::{Arc, Mutex, MutexGuard};
use std::collections::HashSet;
use std::io::Write;
use log::Level;
use colored::{Color, Colorize};

struct LogWorker {
    handle: Option<thread::JoinHandle<()>>,
    messages: Arc<Mutex<Vec<String>>>,
}

pub struct AsyncLogger {
    worker: Arc<Mutex<LogWorker>>,
    whitelist: HashSet<String>,
}


impl AsyncLogger {
    pub fn new() -> Self {
        let messages: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let worker: Arc<Mutex<LogWorker>> = Arc::new(Mutex::new(LogWorker {
            messages: messages.clone(),
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
        // Get a sync channel for panic messages
        let (panic_sender, panic_receiver) = std::sync::mpsc::sync_channel(1);
        let panic_sender = Arc::new(Mutex::new(panic_sender));

        // 1. Panic Hook
        std::panic::set_hook({
            let panic_sender = panic_sender.clone();
            Box::new(move |panic_info| {
                let msg = format!("PANIC: {}", panic_info);
                // Sync write to stderr FIRST
                let _ = std::io::stderr().write_all(msg.as_bytes());
                // Then notify logger thread
                if let Ok(sender) = panic_sender.lock() {
                    let _ = sender.send(msg);
                }
            })
        });

        // 2. Normal Exit Hook
        ctrlc::set_handler({
            let logger = self.worker.clone();
            move || {
                // Force flush remaining messages
                if let Ok(mut worker) = logger.lock() {
                    let messages_clone: Arc<Mutex<Vec<String>>> = worker.messages.clone();
                    let messages: MutexGuard<Vec<String>> = messages_clone.lock().expect("Could not lock messages");
                    let _ = std::io::stderr().write_all(messages.join("\n").as_bytes());
                    worker.handle.take();
                }
                std::process::exit(0);
            }
        }).unwrap();

        // 3. Log thread watches for panics
        let worker: Arc<Mutex<LogWorker>> = self.worker.clone();
        thread::spawn(move || {
            // This will block until panic occurs
            if let Ok(panic_msg) = panic_receiver.recv() {
                if let Ok(mut worker) = worker.lock() {
                    // Add panic to regular log queue
                    worker.messages.lock().expect("Could not lock messages").push(panic_msg);
                    worker.handle.take();
                }
            }
        });
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

