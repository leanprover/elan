use errors::*;
use time;
use elan_utils::{raw, utils};
use serde_json;

use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum TelemetryEvent {
    LeanRun { duration_ms: u64, exit_code: i32, errors: Option<Vec<String>> },
    ToolchainUpdate { toolchain: String, success: bool } ,
    TargetAdd { toolchain: String, target: String, success: bool },
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LogMessage {
    log_time_s: i64,
    event: TelemetryEvent,
    version: i32,
}

impl LogMessage {
    pub fn get_event(&self) -> TelemetryEvent {
        self.event.clone()
    }
}

#[derive(Debug)]
pub struct Telemetry {
    telemetry_dir: PathBuf
}

const LOG_FILE_VERSION: i32 = 1;
const MAX_TELEMETRY_FILES: usize = 100;

impl Telemetry {
    pub fn new(telemetry_dir: PathBuf) -> Telemetry {
        Telemetry { telemetry_dir: telemetry_dir }
    }

    pub fn log_telemetry(&self, event: TelemetryEvent) -> Result<()> {
        Ok(())
    }

    pub fn clean_telemetry_dir(&self) -> Result<()> {
        let telemetry_dir_contents = self.telemetry_dir.read_dir();

        let contents = try!(telemetry_dir_contents.chain_err(|| ErrorKind::TelemetryCleanupError));

        let mut telemetry_files: Vec<PathBuf> = Vec::new();

        for c in contents {
            let x = c.unwrap();
            let filename = x.path().file_name().unwrap().to_str().unwrap().to_owned();
            if filename.starts_with("log") && filename.ends_with("json") {
                telemetry_files.push(x.path());
            }
        }

        if telemetry_files.len() < MAX_TELEMETRY_FILES {
            return Ok(());
        }

        let dl: usize = telemetry_files.len() - MAX_TELEMETRY_FILES;
        let dl = dl + 1 as usize;

        telemetry_files.sort();
        telemetry_files.dedup();

        for i in 0..dl {
            let i = i as usize;
            try!(fs::remove_file(&telemetry_files[i]).chain_err(|| ErrorKind::TelemetryCleanupError));
        }

        Ok(())
    }
}
