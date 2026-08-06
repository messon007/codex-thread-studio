use std::ffi::OsString;
use std::io;
use std::process::{ExitStatus, Stdio};

use tokio::process::Command;

use crate::command_runtime::{command_spec, spawn_managed, ManagedChild};

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub struct WslSettings {
    pub distribution: Option<String>,
    pub user: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BackendRuntime {
    path: OsString,
}

impl BackendRuntime {
    pub fn new(path: OsString, wsl: WslSettings) -> Self {
        let _ = (wsl.distribution, wsl.user);
        Self { path }
    }

    pub fn environment(&self) -> &'static str {
        "local"
    }

    pub fn wsl_distribution(&self) -> Option<&str> {
        None
    }

    pub fn command(
        &self,
        binary: &str,
        args: &[&str],
        environment: &[(&str, &str)],
    ) -> RuntimeCommand {
        let command = command_spec(binary, args)
            .command(&self.path)
            .map(|mut command| {
                for (key, value) in environment {
                    command.env(key, value);
                }
                command
            });
        RuntimeCommand { command }
    }
}

pub struct RuntimeCommand {
    command: io::Result<Command>,
}

impl RuntimeCommand {
    pub fn stdin(&mut self, configuration: Stdio) -> &mut Self {
        if let Ok(command) = self.command.as_mut() {
            command.stdin(configuration);
        }
        self
    }

    pub fn stdout(&mut self, configuration: Stdio) -> &mut Self {
        if let Ok(command) = self.command.as_mut() {
            command.stdout(configuration);
        }
        self
    }

    pub fn stderr(&mut self, configuration: Stdio) -> &mut Self {
        if let Ok(command) = self.command.as_mut() {
            command.stderr(configuration);
        }
        self
    }

    pub fn spawn(self) -> io::Result<RuntimeChild> {
        let child = spawn_managed(self.command?)?;
        Ok(RuntimeChild { child })
    }
}

pub struct RuntimeChild {
    child: ManagedChild,
}

impl RuntimeChild {
    pub fn take_stdin(&mut self) -> Option<tokio::process::ChildStdin> {
        self.child.take_stdin()
    }

    pub fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.child.take_stdout()
    }

    pub fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.child.take_stderr()
    }

    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait().await
    }

    pub async fn kill_tree(&mut self) -> io::Result<()> {
        self.child.kill_tree().await
    }
}
