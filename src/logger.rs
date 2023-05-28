use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use instant::Instant;
use log::{Level, LevelFilter};
use simple_logger::SimpleLogger;

pub struct Logger {
   start: Instant,
   simple_logger: SimpleLogger,
   file: Option<Mutex<File>>,
}

impl Logger {
   pub fn new(filename: Option<&Path>) -> Self {
      Self {
         start: Instant::now(),
         simple_logger: SimpleLogger::new()
            .with_level(LevelFilter::Warn)
            .with_module_level("netcanv", LevelFilter::Debug)
            .env(),
         file: filename
            .and_then(|filename| match File::create(filename) {
               Ok(file) => Some(file),
               Err(error) => {
                  eprintln!("unable to log to file: {error:?}");
                  None
               }
            })
            .map(Mutex::new),
      }
   }

   pub fn init(filename: Option<&Path>) -> Result<(), log::SetLoggerError> {
      log::set_max_level(LevelFilter::Trace);
      log::set_boxed_logger(Box::new(Self::new(filename)))
   }
}

impl log::Log for Logger {
   fn enabled(&self, metadata: &log::Metadata) -> bool {
      metadata.level() <= Level::Debug
   }

   fn log(&self, record: &log::Record) {
      self.simple_logger.log(record);

      if self.enabled(record.metadata()) {
         if let Some(file_mutex) = &self.file {
            let mut file = file_mutex.lock().unwrap();
            let time = self.start.elapsed().as_secs_f64();
            let _ = writeln!(
               file,
               "[{time:>3.3}] {} {}: {}",
               record.module_path().unwrap_or(""),
               record.level(),
               record.args()
            );
            let _ = file.flush();
         }
      }
   }

   fn flush(&self) {
      self.simple_logger.flush();
      if let Some(file_mutex) = &self.file {
         let mut file = file_mutex.lock().unwrap();
         let _ = file.flush();
      }
   }
}
