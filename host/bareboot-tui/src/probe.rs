use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use crate::worker::UiEvent;
use std::sync::mpsc::Sender;
use std::thread;

pub fn run_probe_command(args: Vec<String>, output: Sender<UiEvent>) {
    thread::spawn(move || {
        let mut command = Command::new("probe-rs");
        command.args(&args).stdout(Stdio::piped()).stderr(Stdio::piped());

        match command.spawn() {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        let _ = output.send(UiEvent::Log { line });
                    }
                }
                if let Some(stderr) = child.stderr.take() {
                    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                        let _ = output.send(UiEvent::Log { line });
                    }
                }
                let _ = child.wait();
            }
            Err(err) => {
                let _ = output.send(UiEvent::Error { message: format!("failed to spawn probe-rs: {err}") });
            }
        }
    });
}
