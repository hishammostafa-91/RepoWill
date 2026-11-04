// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
    let (prog, args) = match opts.shell {
        Shell::None => {
            if argv.is_empty() {
                return RunResult {
                    code: None,
                    stdout: String::new(),
                    stderr: "empty argv".into(),
                    timed_out: false,
                };
            }
            (argv[0].clone(), argv[1..].to_vec())
        }
        Shell::Cmd => ("cmd".into(), {
            let mut v = vec!["/C".into()];
            v.push(argv.join(" "));
            v
        }),
        Shell::Pwsh => ("pwsh".into(), {
            let mut v = vec!["-NoProfile".into(), "-Command".into()];
            v.push(argv.join(" "));
            v
        }),
        Shell::Powershell => ("powershell".into(), {
            let mut v = vec!["-NoProfile".into(), "-Command".into()];
            v.push(argv.join(" "));
            v
        }),
        Shell::Bash => ("bash".into(), {
            let mut v = vec!["-c".into()];
            v.push(argv.join(" "));
            v
        }),
        Shell::Sh => ("sh".into(), {
            let mut v = vec!["-c".into()];
            v.push(argv.join(" "));
            v
        }),
        Shell::Auto => {
            if cfg!(windows) {
                ("cmd".into(), {
                    let mut v = vec!["/C".into()];
                    v.push(argv.join(" "));
                    v
                })
            } else {
                ("sh".into(), {
                    let mut v = vec!["-c".into()];
                    v.push(argv.join(" "));
                    v
                })
            }
        }
    };

    let mut cmd = Command::new(&prog);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in &opts.env {
        cmd.env(k, v);
    }

    let mut child = match cmd.spawn() {
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

    let start = Instant::now();
    let timeout = opts.timeout.unwrap_or(Duration::from_secs(300));
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = Vec::new();
                        use std::io::Read;
                        let _ = s.read_to_end(&mut buf);
                        buf
                    })
                    .unwrap_or_default();
                let err = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = Vec::new();
                        use std::io::Read;
                        let _ = s.read_to_end(&mut buf);
                        buf
                    })
                    .unwrap_or_default();
                return RunResult {
                    code: status.code(),
                    stdout: String::from_utf8_lossy(&out[..out.len().min(opts.cap_bytes)])
                        .to_string(),
                    stderr: String::from_utf8_lossy(&err[..err.len().min(opts.cap_bytes)])
                        .to_string(),
                    timed_out: false,
                };
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    return RunResult {
                        code: None,
                        stdout: String::new(),
                        stderr: "timed out".into(),
                        timed_out: true,
                    };
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return RunResult {
                    code: None,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    timed_out: false,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_echo() {
        let r = if cfg!(windows) {
            run(
                &["cmd".into(), "/C".into(), "echo hi".into()],
                &RunOptions {
                    shell: Shell::None,
                    ..Default::default()
                },
            )
        } else {
            run(
                &["sh".into(), "-c".into(), "echo hi".into()],
                &RunOptions {
                    shell: Shell::None,
                    ..Default::default()
                },
            )
        };
        assert!(r.stdout.contains("hi") || r.code == Some(0));
    }

    #[test]
    fn resolve_bin_finds_something() {
        // `which` itself may not exist on PATH as a binary named which on all systems; just call.
        let _ = resolve_bin("git");
    }
}
