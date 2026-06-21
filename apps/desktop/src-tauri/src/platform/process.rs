use crate::platform::command_executable_name;
use crate::platform::types::{
    CommandSpec, PlatformError, PlatformResult, ProcessHandle, ProcessOutput,
};
use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

static CHILDREN: OnceLock<Mutex<HashMap<u32, Child>>> = OnceLock::new();

pub fn spawn_structured_process(spec: CommandSpec) -> PlatformResult<ProcessHandle> {
    validate_structured_process_spec(&spec)?;

    let mut command = command_for_spec(&spec);
    let child = command.spawn()?;
    let pid = child.id();
    children()
        .lock()
        .map_err(|_| PlatformError::Unsupported("process registry lock poisoned".to_string()))?
        .insert(pid, child);

    Ok(ProcessHandle { pid })
}

pub fn run_structured_process(spec: CommandSpec) -> PlatformResult<ProcessOutput> {
    validate_structured_process_spec(&spec)?;

    let mut command = command_for_spec(&spec);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;

    let stdout_reader = child.stdout.take().map(read_pipe_to_string);
    let stderr_reader = child.stderr.take().map(read_pipe_to_string);
    let (status, timed_out) = wait_child(&mut child, spec.timeout_ms)?;

    let stdout = join_reader(stdout_reader)?;
    let mut stderr = join_reader(stderr_reader)?;
    if timed_out {
        let timeout_message = format!(
            "process timed out after {} ms",
            spec.timeout_ms.unwrap_or_default()
        );
        stderr = if stderr.trim().is_empty() {
            timeout_message
        } else {
            format!("{timeout_message}\n{stderr}")
        };
    }

    Ok(ProcessOutput {
        status_code: if timed_out { None } else { status.code() },
        stdout,
        stderr,
    })
}

pub fn kill_process_tree(pid: u32) -> PlatformResult<()> {
    if pid == 0 {
        return Err(PlatformError::Unsupported(
            "cannot kill process tree for pid 0".to_string(),
        ));
    }

    if let Some(mut child) = children()
        .lock()
        .map_err(|_| PlatformError::Unsupported("process registry lock poisoned".to_string()))?
        .remove(&pid)
    {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        let result = kill_platform_process_tree(pid);
        let _ = child.kill();
        let _ = child.wait();
        return result;
    }

    kill_platform_process_tree(pid)
}

fn children() -> &'static Mutex<HashMap<u32, Child>> {
    CHILDREN.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn validate_structured_process_spec(spec: &CommandSpec) -> PlatformResult<()> {
    if spec.executable.as_os_str().is_empty() {
        return Err(PlatformError::InvalidPath(
            "command executable cannot be empty".to_string(),
        ));
    }

    if spec.args.iter().any(|arg| arg == "-g" || arg == "--global") {
        return Err(PlatformError::Unsupported(
            "global package installs are blocked by policy".to_string(),
        ));
    }

    if spec.env.keys().any(|key| key.eq_ignore_ascii_case("PATH"))
        || spec.args.iter().any(|arg| {
            arg.eq_ignore_ascii_case("PATH")
                || arg
                    .split_once('=')
                    .is_some_and(|(key, _)| key.eq_ignore_ascii_case("PATH"))
        })
    {
        return Err(PlatformError::Unsupported(
            "commands that modify PATH are blocked by policy".to_string(),
        ));
    }

    let executable = command_executable_name(&spec.executable).unwrap_or_default();
    let joined_args = spec.args.join(" ").to_ascii_lowercase();
    if executable == "curl"
        || executable == "wget"
        || ((joined_args.contains("curl ")
            || joined_args.contains("wget ")
            || joined_args.contains("irm ")
            || joined_args.contains("iwr "))
            && (joined_args.contains("http://") || joined_args.contains("https://")))
    {
        return Err(PlatformError::Unsupported(
            "remote download scripts are blocked by policy".to_string(),
        ));
    }

    if binds_public_interface(spec) {
        return Err(PlatformError::Unsupported(
            "non-loopback interface binds are blocked by policy".to_string(),
        ));
    }

    Ok(())
}

fn binds_public_interface(spec: &CommandSpec) -> bool {
    if spec
        .args
        .iter()
        .chain(spec.env.values())
        .any(|value| value.contains("0.0.0.0") || value == "::" || value == "[::]")
    {
        return true;
    }

    for (index, arg) in spec.args.iter().enumerate() {
        if matches!(arg.as_str(), "--host" | "--hostname" | "-H") {
            if let Some(host) = spec.args.get(index + 1) {
                if !is_allowed_loopback_bind(host) {
                    return true;
                }
            }
        }
        for prefix in ["--host=", "--hostname="] {
            if let Some(host) = arg.strip_prefix(prefix) {
                if !is_allowed_loopback_bind(host) {
                    return true;
                }
            }
        }
    }

    spec.env
        .iter()
        .any(|(key, value)| is_host_env_key(key) && !is_allowed_loopback_bind(value))
}

fn is_host_env_key(key: &str) -> bool {
    key.eq_ignore_ascii_case("HOST") || key.to_ascii_uppercase().ends_with("_HOST")
}

fn is_allowed_loopback_bind(value: &str) -> bool {
    value.trim().trim_matches(|ch| ch == '"' || ch == '\'') == "127.0.0.1"
}

fn command_for_spec(spec: &CommandSpec) -> Command {
    let mut command = Command::new(&spec.executable);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .envs(&spec.env);
    configure_process_tree_boundary(&mut command);
    command
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

#[cfg(windows)]
fn kill_platform_process_tree(pid: u32) -> PlatformResult<()> {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(PlatformError::Unsupported(format!(
            "failed to kill process tree for pid {pid}: taskkill exited with {status}"
        )))
    }
}

#[cfg(unix)]
fn kill_platform_process_tree(pid: u32) -> PlatformResult<()> {
    let pid = pid as libc::pid_t;
    let target_group = unsafe { libc::getpgid(pid) };
    if target_group < 0 {
        return Ok(());
    }

    // Negative PID signals are only safe when the spawned child owns its
    // process group. GitHub hosted runners can share long-lived shell process
    // groups; blindly signaling -pid can terminate the runner step.
    let term_result = if target_group == pid {
        send_unix_signal(-pid, libc::SIGTERM)
    } else {
        send_unix_signal(pid, libc::SIGTERM)
    };
    thread::sleep(Duration::from_millis(200));
    let kill_result = if target_group == pid {
        send_unix_signal(-pid, libc::SIGKILL)
    } else {
        send_unix_signal(pid, libc::SIGKILL)
    };

    if term_result.is_ok() || kill_result.is_ok() {
        Ok(())
    } else {
        Err(PlatformError::Unsupported(format!(
            "failed to kill process tree for pid {pid}: TERM={term_result:?}, KILL={kill_result:?}"
        )))
    }
}

#[cfg(unix)]
fn send_unix_signal(pid_or_group: libc::pid_t, signal: libc::c_int) -> std::io::Result<()> {
    if unsafe { libc::kill(pid_or_group, signal) } == 0 {
        Ok(())
    } else {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error)
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn kill_platform_process_tree(pid: u32) -> PlatformResult<()> {
    Err(PlatformError::Unsupported(format!(
        "process-tree killing is not implemented for pid {pid} on this platform"
    )))
}

fn wait_child(child: &mut Child, timeout_ms: Option<u64>) -> PlatformResult<(ExitStatus, bool)> {
    let Some(timeout_ms) = timeout_ms else {
        return Ok((child.wait()?, false));
    };

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((status, false));
        }

        if Instant::now() >= deadline {
            let _ = kill_platform_process_tree(child.id());
            let _ = child.kill();
            return Ok((child.wait()?, true));
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn read_pipe_to_string<R>(mut pipe: R) -> thread::JoinHandle<std::io::Result<String>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut output = String::new();
        pipe.read_to_string(&mut output)?;
        Ok(output)
    })
}

fn join_reader(
    reader: Option<thread::JoinHandle<std::io::Result<String>>>,
) -> PlatformResult<String> {
    let Some(reader) = reader else {
        return Ok(String::new());
    };

    reader
        .join()
        .map_err(|_| PlatformError::Unsupported("process output reader panicked".to_string()))?
        .map_err(PlatformError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[cfg(unix)]
    #[test]
    fn run_structured_process_respects_timeout() {
        let output = run_structured_process(CommandSpec {
            executable: PathBuf::from("sh"),
            args: vec!["-c".to_string(), "sleep 2".to_string()],
            cwd: std::env::current_dir().expect("cwd"),
            env: HashMap::new(),
            allowed_network: false,
            timeout_ms: Some(50),
            kill_on_drop: true,
        })
        .expect("process output");

        assert_eq!(output.status_code, None);
        assert!(output.stderr.contains("process timed out after 50 ms"));
    }

    #[test]
    fn validate_structured_process_allows_policy_approved_network_marker() {
        let result = validate_structured_process_spec(&CommandSpec {
            executable: PathBuf::from("pnpm"),
            args: vec![
                "install".to_string(),
                "--ignore-scripts".to_string(),
                "--prefer-offline".to_string(),
            ],
            cwd: std::env::current_dir().expect("cwd"),
            env: HashMap::new(),
            allowed_network: true,
            timeout_ms: Some(50),
            kill_on_drop: true,
        });

        assert!(result.is_ok());
    }

    #[test]
    fn run_structured_process_rejects_path_and_public_bind_forms() {
        let mut env = HashMap::new();
        env.insert("Path".to_string(), "/tmp/bin".to_string());

        let path_result = run_structured_process(CommandSpec {
            executable: PathBuf::from("pnpm"),
            args: Vec::new(),
            cwd: std::env::current_dir().expect("cwd"),
            env,
            allowed_network: false,
            timeout_ms: Some(50),
            kill_on_drop: true,
        });
        assert!(
            matches!(path_result, Err(PlatformError::Unsupported(message)) if message.contains("PATH"))
        );

        let bind_result = run_structured_process(CommandSpec {
            executable: PathBuf::from("pnpm"),
            args: vec![
                "exec".to_string(),
                "vite".to_string(),
                "--host=0.0.0.0".to_string(),
            ],
            cwd: std::env::current_dir().expect("cwd"),
            env: HashMap::new(),
            allowed_network: false,
            timeout_ms: Some(50),
            kill_on_drop: true,
        });
        assert!(
            matches!(bind_result, Err(PlatformError::Unsupported(message)) if message.contains("non-loopback"))
        );

        let lan_bind_result = run_structured_process(CommandSpec {
            executable: PathBuf::from("pnpm"),
            args: vec![
                "exec".to_string(),
                "vite".to_string(),
                "--host".to_string(),
                "192.168.1.10".to_string(),
            ],
            cwd: std::env::current_dir().expect("cwd"),
            env: HashMap::new(),
            allowed_network: false,
            timeout_ms: Some(50),
            kill_on_drop: true,
        });
        assert!(
            matches!(lan_bind_result, Err(PlatformError::Unsupported(message)) if message.contains("non-loopback"))
        );
    }

    #[test]
    fn validate_structured_process_normalizes_windows_command_extensions() {
        let result = validate_structured_process_spec(&CommandSpec {
            executable: PathBuf::from("C:\\Sofvary\\sidecars\\windows-x64\\CURL.CMD"),
            args: Vec::new(),
            cwd: std::env::current_dir().expect("cwd"),
            env: HashMap::new(),
            allowed_network: false,
            timeout_ms: Some(50),
            kill_on_drop: true,
        });

        assert!(
            matches!(result, Err(PlatformError::Unsupported(message)) if message.contains("remote download"))
        );
    }
}
