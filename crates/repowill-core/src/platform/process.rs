// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::io::Read;
use std::process::{ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use process_wrap::std::CommandWrap;
#[cfg(windows)]
use process_wrap::std::JobObject;
#[cfg(unix)]
use process_wrap::std::ProcessGroup;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Auto,
    Cmd,
    Pwsh,
    Powershell,
    Bash,
    Sh,
    None,
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub shell: Shell,
    pub env: Vec<(String, String)>,
    pub timeout: Option<Duration>,
    pub cap_bytes: usize,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            shell: Shell::None,
            env: vec![],
            timeout: Some(Duration::from_secs(300)),
            cap_bytes: 5 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

pub fn resolve_bin(name: &str) -> Option<std::path::PathBuf> {
    which::which(name).ok()
}

pub fn run(argv: &[String], opts: &RunOptions) -> RunResult {
    if argv.is_empty() && opts.shell == Shell::None {
        return RunResult {
            code: None,
            stdout: String::new(),
            stderr: "empty argv".into(),
            timed_out: false,
        };
    }

    // Determine program + args based on shell.
    let (program, args): (String, Vec<String>) = match opts.shell {
        Shell::None => (argv[0].clone(), argv[1..].to_vec()),
        Shell::Cmd => ("cmd".into(), vec!["/C".into(), argv.join(" ")]),
        Shell::Pwsh => (
            "pwsh".into(),
            vec!["-NoProfile".into(), "-Command".into(), argv.join(" ")],
        ),
        Shell::Powershell => (
            "powershell".into(),
            vec!["-NoProfile".into(), "-Command".into(), argv.join(" ")],
        ),
        Shell::Bash => ("bash".into(), vec!["-c".into(), argv.join(" ")]),
        Shell::Sh => ("sh".into(), vec!["-c".into(), argv.join(" ")]),
        Shell::Auto => {
            if cfg!(windows) {
                ("cmd".into(), vec!["/C".into(), argv.join(" ")])
            } else {
                ("sh".into(), vec!["-c".into(), argv.join(" ")])
            }
        }
    };

    let mut wrapped = CommandWrap::with_new(&program, |c| {
        c.args(&args);
        c.stdout(Stdio::piped()).stderr(Stdio::piped());
        for (k, v) in &opts.env {
            c.env(k, v);
        }
    });

    #[cfg(windows)]
    {
        wrapped.wrap(JobObject);
    }
    #[cfg(unix)]
    {
        wrapped.wrap(ProcessGroup::leader());
    }

    let mut child = match wrapped.spawn() {
        Ok(c) => c,
        Err(e) => {
            return RunResult {
                code: None,
                stdout: String::new(),
                stderr: e.to_string(),
                timed_out: false,
            };
        }
    };

    let stdout_pipe = child.stdout().take();
    let stderr_pipe = child.stderr().take();

    let (tx_out, rx_out) = mpsc::channel::<Vec<u8>>();
    let (tx_err, rx_err) = mpsc::channel::<Vec<u8>>();
    if let Some(mut s) = stdout_pipe {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            let _ = tx_out.send(buf);
        });
    } else {
        let _ = tx_out.send(Vec::new());
    }
    if let Some(mut s) = stderr_pipe {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            let _ = tx_err.send(buf);
        });
    } else {
        let _ = tx_err.send(Vec::new());
    }

    let timeout = opts.timeout.unwrap_or(Duration::from_secs(300));
    let start = Instant::now();
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    let _ = child.start_kill();
                    let _ = child.wait();
                    break;
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break,
        }
    }

    let out = rx_out
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_default();
    let err = rx_err
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_default();

    let code = if timed_out {
        None
    } else {
        child
            .try_wait()
            .ok()
            .flatten()
            .and_then(|s: ExitStatus| s.code())
    };

    RunResult {
        code,
        stdout: String::from_utf8_lossy(&out[..out.len().min(opts.cap_bytes)]).to_string(),
        stderr: String::from_utf8_lossy(&err[..err.len().min(opts.cap_bytes)]).to_string(),
        timed_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_echo() {
        let r = run(
            &["echo".into(), "hi".into()],
            &RunOptions {
                shell: Shell::Auto,
                ..Default::default()
            },
        );
        assert!(r.stdout.contains("hi") || r.code == Some(0));
    }

    #[test]
    fn resolve_bin_finds_git() {
        assert!(resolve_bin("git").is_some());
    }
}
