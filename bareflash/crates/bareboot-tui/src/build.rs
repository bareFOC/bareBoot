use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use crate::worker::UiEvent;
use std::sync::mpsc::Sender;
use std::thread;

#[derive(Debug, Clone)]
pub struct BuildJob {
    pub manifest_path: String,
}

pub fn stream_cargo_build(job: BuildJob, output: Sender<UiEvent>) {
    thread::spawn(move || {
        let mut command = Command::new("cargo");
        command
            .args(["build", "--manifest-path", &job.manifest_path])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
                let _ = output.send(UiEvent::Error { message: format!("failed to spawn cargo: {err}") });
            }
        }
    });
}

pub fn discover_firmware(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    let target = root.join("target");
    if !target.exists() { return None; }
    fn walkdir(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() { walkdir(&p, out); }
                else { out.push(p); }
            }
        }
    }
    let mut files = Vec::new();
    walkdir(&target, &mut files);
    for entry in files {
        let ext = entry.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext == "bin" || ext == "elf" {
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if best.as_ref().map(|(t,_)| mtime > *t).unwrap_or(true) {
                        best = Some((mtime, entry));
                    }
                }
            }
        }
    }
    best.map(|(_,p)| p)
}
