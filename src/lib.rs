use log::{Record, Level, Metadata, LevelFilter};
use chrono::Local;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc;
use std::io::Write;

struct LogMessage {
	level: Level,
	target: String,
	message: String,
	timestamp: String,
}

#[derive(Debug)]
pub struct CustomLogger {
	sender: Mutex<Option<mpsc::Sender<LogMessage>>>,
	thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
	root_module: String,
}

impl CustomLogger {
	fn new(root_module: String) -> Self {
		let (sender, receiver) = mpsc::channel::<LogMessage>();

		// create separate thread to handle printing so it doesn't block the main thread
		let receiver = Arc::new(Mutex::new(receiver));
		
		let thread_handle = thread::Builder::new().spawn(move || {
			let stdout = StandardStream::stdout(ColorChoice::Always);

			loop {
				let receiver = receiver.lock().expect("Could not lock receiver");
				if let Ok(log_message) = receiver.recv() {
					let mut handle = stdout.lock();
					let mut color_spec = ColorSpec::new();

					// set color based on log level
					match log_message.level {
						Level::Error => color_spec.set_fg(Some(Color::Red)),
						Level::Warn => color_spec.set_fg(Some(Color::Yellow)),
						Level::Info => color_spec.set_fg(Some(Color::Green)),
						Level::Debug => color_spec.set_fg(Some(Color::Cyan)),
						Level::Trace => color_spec.set_fg(Some(Color::White)),
					};

					handle.set_color(&color_spec).expect("Failed to set color");

					let message = format!(
						"{} [{} @ {}] {}\n",
						log_message.timestamp,
						log_message.level,
						log_message.target,
						log_message.message
					);
					
					handle.write_all(message.as_bytes()).expect("Failed to write log message");
					handle.reset().expect("Failed to reset color");
				} else {
					// channel disconnected, exit thread
					break
				}
			}
		}).expect("Failed to spawn logging thread");

		CustomLogger {
            sender: Mutex::new(Some(sender)),
            thread_handle: Mutex::new(Some(thread_handle)),
			root_module,
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
}

impl log::Log for CustomLogger {
	fn enabled(&self, metadata: &Metadata) -> bool {
		metadata.level() <= Level::Info &&
		metadata.target().starts_with(&self.root_module)
	}

	fn log(&self, record: &Record) {
		let timestamp = Local::now().format("%H:%M:%S%.3f").to_string();
		let target = record.target().to_string();
		let message = record.args().to_string();

		// Prepare the log message
		let log_message = LogMessage {
			level: record.level(),
			target,
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

pub fn init_logger() -> Arc<CustomLogger> {
	let mut root_module: String = module_path!()
		.split("::")
		.next()
		.expect("Failed to extract crate root")
		.to_string();
	root_module.push_str("::");

	let logger = Arc::new(CustomLogger::new(root_module));
	log::set_boxed_logger(Box::new(logger.clone())).expect("Failed to set logger");
	log::set_max_level(LevelFilter::Trace);
	logger
}
