use crate::platform::process::validate_structured_process_spec;
use crate::platform::types::{CommandSpec, PlatformError, PlatformResult};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

pub struct StdioJsonRpcProcess {
    child: Child,
    stdin: ChildStdin,
    stdout_rx: Receiver<std::io::Result<String>>,
    stderr_rx: Receiver<std::io::Result<String>>,
    stderr_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StdioLine {
    Stdout(String),
    Stderr(String),
}

pub struct StdioLineProcess {
    child: Child,
    line_rx: Receiver<std::io::Result<StdioLine>>,
}

impl StdioJsonRpcProcess {
    pub fn spawn(spec: &CommandSpec) -> PlatformResult<Self> {
        let mut command = Command::new(&spec.executable);
        command
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(&spec.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_process_tree_boundary(&mut command);

        let mut child = command.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| {
            PlatformError::Unsupported("stdio process stdin is unavailable".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            PlatformError::Unsupported("stdio process stdout is unavailable".to_string())
        })?;
        let stderr = child.stderr.take();

        let (stdout_tx, stdout_rx) = mpsc::channel();
        spawn_json_line_reader(stdout, stdout_tx);
        let (stderr_tx, stderr_rx) = mpsc::channel();
        if let Some(stderr) = stderr {
            spawn_json_line_reader(stderr, stderr_tx);
        }

        Ok(Self {
            child,
            stdin,
            stdout_rx,
            stderr_rx,
            stderr_lines: Vec::new(),
        })
    }

    pub fn write_line(&mut self, line: &str) -> PlatformResult<()> {
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    pub fn read_line_timeout(&mut self, timeout: Duration) -> PlatformResult<Option<String>> {
        match self.stdout_rx.recv_timeout(timeout) {
            Ok(Ok(line)) => Ok(Some(line)),
            Ok(Err(error)) => Err(PlatformError::Io(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    pub fn drain_stderr(&mut self) -> PlatformResult<Vec<String>> {
        let mut lines = Vec::new();
        loop {
            match self.stderr_rx.try_recv() {
                Ok(Ok(line)) => {
                    self.stderr_lines.push(line.clone());
                    lines.push(line);
                }
                Ok(Err(error)) => return Err(PlatformError::Io(error)),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        Ok(lines)
    }

    pub fn recent_stderr(&mut self) -> PlatformResult<String> {
        self.drain_stderr()?;
        Ok(self.stderr_lines.join("\n"))
    }

    pub fn try_wait(&mut self) -> PlatformResult<Option<ExitStatus>> {
        Ok(self.child.try_wait()?)
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for StdioJsonRpcProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            self.kill();
        }
    }
}

impl StdioLineProcess {
    #[allow(dead_code)]
    pub fn spawn(spec: &CommandSpec) -> PlatformResult<Self> {
        Self::spawn_with_stdin(spec, None)
    }

    pub fn spawn_with_stdin(spec: &CommandSpec, stdin_text: Option<&str>) -> PlatformResult<Self> {
        validate_structured_process_spec(spec)?;

        let mut command = Command::new(&spec.executable);
        command
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(&spec.env)
            .stdin(if stdin_text.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_process_tree_boundary(&mut command);

        let mut child = command.spawn()?;
        if let Some(stdin_text) = stdin_text {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                PlatformError::Unsupported("stdio line process stdin is unavailable".to_string())
            })?;
            stdin.write_all(stdin_text.as_bytes())?;
            stdin.flush()?;
            drop(stdin);
        }
        let stdout = child.stdout.take().ok_or_else(|| {
            PlatformError::Unsupported("stdio line process stdout is unavailable".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            PlatformError::Unsupported("stdio line process stderr is unavailable".to_string())
        })?;

        let (line_tx, line_rx) = mpsc::channel();
        spawn_line_reader(stdout, line_tx.clone(), StdioLine::Stdout);
        spawn_line_reader(stderr, line_tx, StdioLine::Stderr);

        Ok(Self { child, line_rx })
    }

    pub fn read_line_timeout(&mut self, timeout: Duration) -> PlatformResult<Option<StdioLine>> {
        match self.line_rx.recv_timeout(timeout) {
            Ok(Ok(line)) => Ok(Some(line)),
            Ok(Err(error)) => Err(PlatformError::Io(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    pub fn try_wait(&mut self) -> PlatformResult<Option<ExitStatus>> {
        Ok(self.child.try_wait()?)
    }

    pub fn kill(&mut self) {
        let _ = crate::platform::process::kill_process_tree(self.child.id());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for StdioLineProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            self.kill();
        }
    }
}

fn spawn_line_reader<R>(
    stream: R,
    line_tx: Sender<std::io::Result<StdioLine>>,
    wrap: fn(String) -> StdioLine,
) where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let line = line.trim_end_matches(['\r', '\n']).to_string();
                    if line_tx.send(Ok(wrap(line))).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = line_tx.send(Err(error));
                    break;
                }
            }
        }
    });
}

fn spawn_json_line_reader<R>(stream: R, line_tx: Sender<std::io::Result<String>>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let line = line.trim_end_matches(['\r', '\n']).to_string();
                    if line_tx.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = line_tx.send(Err(error));
                    break;
                }
            }
        }
    });
}

#[cfg(unix)]
fn configure_process_tree_boundary(command: &mut Command) {
    command.process_group(0);
}

#[cfg(windows)]
fn configure_process_tree_boundary(command: &mut Command) {
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn configure_process_tree_boundary(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    #[test]
    fn stdio_line_process_streams_stdout_and_stderr() {
        let spec = CommandSpec {
            executable: test_shell_executable(),
            args: test_shell_args(),
            cwd: std::env::current_dir().expect("cwd"),
            env: HashMap::new(),
            allowed_network: false,
            timeout_ms: Some(5_000),
            kill_on_drop: true,
        };
        let mut process = StdioLineProcess::spawn(&spec).expect("process");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_stdout = false;
        let mut saw_stderr = false;

        while Instant::now() < deadline && (!saw_stdout || !saw_stderr) {
            if let Some(line) = process
                .read_line_timeout(Duration::from_millis(100))
                .expect("line")
            {
                match line {
                    StdioLine::Stdout(line) if line.trim() == "out" => saw_stdout = true,
                    StdioLine::Stderr(line) if line.trim() == "err" => saw_stderr = true,
                    _ => {}
                }
            }
            if process.try_wait().expect("wait").is_some() && saw_stdout && saw_stderr {
                break;
            }
        }

        assert!(saw_stdout);
        assert!(saw_stderr);
    }

    #[test]
    fn stdio_line_process_can_write_stdin() {
        let spec = CommandSpec {
            executable: test_shell_executable(),
            args: test_shell_stdin_args(),
            cwd: std::env::current_dir().expect("cwd"),
            env: HashMap::new(),
            allowed_network: false,
            timeout_ms: Some(5_000),
            kill_on_drop: true,
        };
        let mut process =
            StdioLineProcess::spawn_with_stdin(&spec, Some("hello from stdin\n")).expect("process");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_stdin = false;

        while Instant::now() < deadline && !saw_stdin {
            if let Some(line) = process
                .read_line_timeout(Duration::from_millis(100))
                .expect("line")
            {
                if matches!(line, StdioLine::Stdout(line) if line.trim() == "hello from stdin") {
                    saw_stdin = true;
                }
            }
            if process.try_wait().expect("wait").is_some() && saw_stdin {
                break;
            }
        }

        assert!(saw_stdin);
    }

    #[test]
    fn stdio_json_rpc_process_keeps_stderr_after_exit() {
        let spec = CommandSpec {
            executable: test_shell_executable(),
            args: test_shell_stderr_exit_args(),
            cwd: std::env::current_dir().expect("cwd"),
            env: HashMap::new(),
            allowed_network: false,
            timeout_ms: Some(5_000),
            kill_on_drop: true,
        };
        let mut process = StdioJsonRpcProcess::spawn(&spec).expect("process");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut status = None;

        while Instant::now() < deadline {
            process.drain_stderr().expect("stderr");
            status = process.try_wait().expect("wait");
            if status.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert_eq!(status.and_then(|status| status.code()), Some(7));
        assert!(process
            .recent_stderr()
            .expect("stderr")
            .contains("json-rpc-boom"));
    }

    #[cfg(windows)]
    fn test_shell_executable() -> PathBuf {
        PathBuf::from("cmd")
    }

    #[cfg(windows)]
    fn test_shell_args() -> Vec<String> {
        vec!["/C".to_string(), "echo out && echo err 1>&2".to_string()]
    }

    #[cfg(windows)]
    fn test_shell_stdin_args() -> Vec<String> {
        vec!["/C".to_string(), "more".to_string()]
    }

    #[cfg(windows)]
    fn test_shell_stderr_exit_args() -> Vec<String> {
        vec![
            "/C".to_string(),
            "echo json-rpc-boom 1>&2 && exit /B 7".to_string(),
        ]
    }

    #[cfg(unix)]
    fn test_shell_executable() -> PathBuf {
        PathBuf::from("sh")
    }

    #[cfg(unix)]
    fn test_shell_args() -> Vec<String> {
        vec![
            "-c".to_string(),
            "printf 'out\\n'; printf 'err\\n' >&2".to_string(),
        ]
    }

    #[cfg(unix)]
    fn test_shell_stdin_args() -> Vec<String> {
        vec!["-c".to_string(), "cat".to_string()]
    }

    #[cfg(unix)]
    fn test_shell_stderr_exit_args() -> Vec<String> {
        vec![
            "-c".to_string(),
            "printf 'json-rpc-boom\\n' >&2; exit 7".to_string(),
        ]
    }
}
