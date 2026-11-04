// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::io::{self, IsTerminal};

pub fn is_tty() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

pub fn is_agent() -> bool {
    std::env::var("REPOWILL_AGENT").is_ok()
        || std::env::var("CI").is_ok()
        || std::env::var("OPENCODE").is_ok()
}

pub fn confirmation_code() -> String {
    // 6-digit code for human channel.
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u32;
    format!("{:06}", n % 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation_code_is_six_digits() {
        let c = confirmation_code();
        assert_eq!(c.len(), 6);
        assert!(c.chars().all(|ch| ch.is_ascii_digit()));
    }
}
