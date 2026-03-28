use std::thread;
use std::time::Duration;

use crate::DbError;

pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 50,
        }
    }
}

impl RetryConfig {
    pub fn execute<T, F>(&self, mut f: F) -> Result<T, DbError>
    where
        F: FnMut() -> Result<T, DbError>,
    {
        let mut attempts = 0;
        loop {
            match f() {
                Ok(val) => return Ok(val),
                Err(DbError::DuckDb(ref e)) if is_busy_error(e) && attempts < self.max_retries => {
                    attempts += 1;
                    let delay = self.base_delay_ms * (1 << (attempts - 1));
                    tracing::warn!(attempt = attempts, delay_ms = delay, "database locked, retrying");
                    thread::sleep(Duration::from_millis(delay));
                }
                Err(DbError::DuckDb(ref e)) if is_busy_error(e) => {
                    return Err(DbError::Locked(self.max_retries));
                }
                Err(e) => return Err(e),
            }
        }
    }
}

fn is_busy_error(e: &duckdb::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("locked") || msg.contains("busy")
}
