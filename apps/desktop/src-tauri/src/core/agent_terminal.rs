use crate::core::agent_config::AgentCommandConfig;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AgentTerminalError {
    #[error("terminal session not found: {0}")]
    NotFound(String),
    #[error("terminal spawn failed: {0}")]
    Spawn(String),
    #[error("terminal io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("terminal lock poisoned")]
    Poisoned,
}

pub type AgentTerminalResult<T> = Result<T, AgentTerminalError>;

#[derive(Debug, Clone)]
pub struct AgentTerminalStartSpec {
    pub thread_id: String,
    pub agent_id: String,
    pub command: AgentCommandConfig,
    pub cwd: PathBuf,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTerminalOutputEvent {
    pub session_id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTerminalSessionSummary {
    pub session_id: String,
    pub thread_id: String,
    pub agent_id: String,
}

#[derive(Default)]
pub struct AgentTerminalManager {
    sessions: Mutex<HashMap<String, AgentTerminalSession>>,
}

impl AgentTerminalManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start<F>(
        &self,
        spec: AgentTerminalStartSpec,
        on_output: F,
    ) -> AgentTerminalResult<AgentTerminalSessionSummary>
    where
        F: Fn(AgentTerminalOutputEvent) + Send + 'static,
    {
        let session_id = format!("term_{}", Uuid::new_v4());
        self.stop_thread(&spec.thread_id)?;

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: spec.rows.max(8),
                cols: spec.cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| AgentTerminalError::Spawn(error.to_string()))?;
        let mut command = CommandBuilder::new(spec.command.executable.as_os_str());
        command.args(spec.command.args.iter().map(String::as_str));
        command.cwd(&spec.cwd);
        for (key, value) in &spec.command.env {
            command.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| AgentTerminalError::Spawn(error.to_string()))?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| AgentTerminalError::Spawn(error.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| AgentTerminalError::Spawn(error.to_string()))?;
        let thread_id = spec.thread_id.clone();
        let agent_id = spec.agent_id.clone();
        let output_session_id = session_id.clone();
        let reader_thread = thread::spawn(move || {
            let mut buffer = [0_u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(bytes) => {
                        on_output(AgentTerminalOutputEvent {
                            session_id: output_session_id.clone(),
                            thread_id: thread_id.clone(),
                            agent_id: agent_id.clone(),
                            text: String::from_utf8_lossy(&buffer[..bytes]).to_string(),
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        let summary = AgentTerminalSessionSummary {
            session_id: session_id.clone(),
            thread_id: spec.thread_id.clone(),
            agent_id: spec.agent_id.clone(),
        };
        let session = AgentTerminalSession {
            thread_id: spec.thread_id,
            master: pair.master,
            writer,
            child,
            reader_thread: Some(reader_thread),
        };
        self.sessions
            .lock()
            .map_err(|_| AgentTerminalError::Poisoned)?
            .insert(session_id, session);
        Ok(summary)
    }

    pub fn write(&self, session_id: &str, data: &str) -> AgentTerminalResult<()> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| AgentTerminalError::Poisoned)?;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AgentTerminalError::NotFound(session_id.to_string()))?;
        session.writer.write_all(data.as_bytes())?;
        session.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, session_id: &str, rows: u16, cols: u16) -> AgentTerminalResult<()> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| AgentTerminalError::Poisoned)?;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AgentTerminalError::NotFound(session_id.to_string()))?;
        session
            .master
            .resize(PtySize {
                rows: rows.max(8),
                cols: cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| AgentTerminalError::Spawn(error.to_string()))
    }

    pub fn stop(&self, session_id: &str) -> AgentTerminalResult<()> {
        let session = self
            .sessions
            .lock()
            .map_err(|_| AgentTerminalError::Poisoned)?
            .remove(session_id)
            .ok_or_else(|| AgentTerminalError::NotFound(session_id.to_string()))?;
        drop(session);
        Ok(())
    }

    pub fn stop_thread(&self, thread_id: &str) -> AgentTerminalResult<()> {
        let session_ids = self
            .sessions
            .lock()
            .map_err(|_| AgentTerminalError::Poisoned)?
            .iter()
            .filter_map(|(session_id, session)| {
                (session.thread_id == thread_id).then(|| session_id.clone())
            })
            .collect::<Vec<_>>();
        for session_id in session_ids {
            let _ = self.stop(&session_id);
        }
        Ok(())
    }
}

struct AgentTerminalSession {
    thread_id: String,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send>,
    reader_thread: Option<JoinHandle<()>>,
}

impl Drop for AgentTerminalSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.reader_thread.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agent_config::AgentInstallSource;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    #[test]
    fn terminal_session_streams_output_and_accepts_input() {
        let manager = AgentTerminalManager::new();
        let output = Arc::new(Mutex::new(String::new()));
        let output_for_callback = output.clone();
        let summary = manager
            .start(
                AgentTerminalStartSpec {
                    thread_id: "thread-a".to_string(),
                    agent_id: "codex".to_string(),
                    command: AgentCommandConfig {
                        executable: PathBuf::from("/bin/sh"),
                        args: vec![
                            "-lc".to_string(),
                            "read line; printf 'terminal:%s' \"$line\"".to_string(),
                        ],
                        env: HashMap::new(),
                        source: AgentInstallSource::ExternalPath,
                    },
                    cwd: std::env::temp_dir(),
                    rows: 24,
                    cols: 80,
                },
                move |event| {
                    output_for_callback
                        .lock()
                        .expect("output lock")
                        .push_str(&event.text);
                },
            )
            .expect("start terminal");

        manager
            .write(&summary.session_id, "ready\n")
            .expect("write terminal input");

        let started_at = Instant::now();
        loop {
            if output
                .lock()
                .expect("output lock")
                .contains("terminal:ready")
            {
                break;
            }
            assert!(
                started_at.elapsed() < Duration::from_secs(5),
                "terminal output was not received"
            );
            std::thread::sleep(Duration::from_millis(25));
        }

        manager.stop(&summary.session_id).expect("stop terminal");
    }
}
