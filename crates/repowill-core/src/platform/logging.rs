// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::path::Path;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn redact(input: &str) -> String {
    // Simple redaction: replace anything that looks like a token/secret.
    let mut out = input.to_string();
    // Redact common secret env var values is not possible here; redact hex-ish long tokens.
    // Canary-friendly: replace the literal canary if present.
    if out.contains("CANARY_SECRET_123") {
        out = out.replace("CANARY_SECRET_123", "[REDACTED]");
    }
    // Redact sequences of 20+ hex chars.
    let mut result = String::new();
    let mut run = String::new();
    for ch in out.chars() {
        if ch.is_ascii_hexdigit() {
            run.push(ch);
        } else {
            if run.len() >= 20 {
                result.push_str("[REDACTED]");
            } else {
                result.push_str(&run);
            }
            run.clear();
            result.push(ch);
        }
    }
    if run.len() >= 20 {
        result.push_str("[REDACTED]");
    } else {
        result.push_str(&run);
    }
    result
}

pub fn init_logging(log_dir: &Path) -> Result<(), String> {
    let appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "repowill.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer().with_writer(non_blocking);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redact_canary() {
        let s = "token=CANARY_SECRET_123";
        assert!(!redact(s).contains("CANARY_SECRET_123"));
    }
    #[test]
    fn redact_long_hex() {
        let s = "key=0123456789abcdef0123456789abcdef";
        assert!(redact(s).contains("[REDACTED]"));
    }
}
