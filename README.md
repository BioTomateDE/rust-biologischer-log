## Features:
- Colored output for different severity levels
- Automatic tracing of modules and functions
- Threaded printing to not block the main thread
- **Muting all other modules** to prevent spam

## Example output:
`23:14:52.826 [WARN @ deserialize::sounds::parse_sound] Sound with name "abc_123_a" has audio data length 82642; but was expected to be 82734.`

## Example usage:
- `Cargo.toml`:
```toml
...
[dependencies]
biologischer-log = { git = "https://github.com/BioTomateDE/rust-biologischer-log.git" }
log = "0.4.27"   # put whatever version you have
```

- `src/main.rs`:
```rust
use biologischer_log::init_logger;
use log::{info, warn};

fn main() {
    // Initialize the logger with your crate name.
    // Call this function in the beginning of your main function.
    let logger = init_logger(env!("CARGO_CRATE_NAME"));
   
    // Do your program stuff, logging with the `debug`, `info`, `warn`,
    // and `error` functions from the `log` crate.
    info!("Hello world");
    warn!("This is a warning");

    // Shutdown the logger. Call this method right before you exit
    // your program so that the logging thread can finish,
    // allowing all messages to get printed
    logger.shutdown();
}
```
