use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use amane::Service;

// the mixed sink desktop and mic sound both loop into, for wf-recorder's one audio device
const MIX_SINK: &str = "kajitsu_rec";

#[derive(Clone, Copy, PartialEq)]
pub enum Audio {
    None,
    Desktop,
    DesktopMic,
    Mic,
}

pub const AUDIO: [Audio; 4] = [Audio::None, Audio::Desktop, Audio::Mic, Audio::DesktopMic];

impl Audio {
    pub fn label(self) -> &'static str {
        match self {
            Audio::None => "No sound",
            Audio::Desktop => "Desktop sound",
            Audio::DesktopMic => "Desktop + mic",
            Audio::Mic => "Mic only",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Audio::None => "󰝟",
            Audio::Desktop => "󰕾",
            Audio::DesktopMic => "󰋋",
            Audio::Mic => "󰍬",
        }
    }

    fn flag(self) -> String {
        match self {
            Audio::None => String::new(),
            Audio::Desktop => String::from("--audio=@DEFAULT_MONITOR@"),
            Audio::DesktopMic => format!("--audio={MIX_SINK}.monitor"),
            Audio::Mic => String::from("--audio=@DEFAULT_SOURCE@"),
        }
    }
}

pub struct Recorder {
    // wf-recorder, only while recording
    child: Option<Child>,

    started: Instant,

    // set once stop is asked for, so an exit before that counts as a failure
    stopping: bool,

    // pactl modules loaded for the desktop and mic mix, unloaded when recording ends
    modules: Vec<String>,

    pub audio: Audio,
    pub error: Option<String>,
    starting: bool,
}

impl Recorder {
    pub fn starting(&self) -> bool {
        self.starting
    }

    pub fn select_audio(&mut self, audio: Audio) {
        if !self.starting && self.child.is_none() {
            self.audio = audio;
        }
    }

    pub fn elapsed(&self) -> Option<String> {
        self.child.as_ref()?;
        Some(time(self.started.elapsed()))
    }
}

impl Service for Recorder {
    fn new() -> Self {
        Self {
            child: None,
            started: Instant::now(),
            stopping: false,
            modules: Vec::new(),
            audio: Audio::None,
            error: None,
            starting: false,
        }
    }

    // ticks the elapsed time, and notices wf-recorder exiting
    fn update(&mut self) -> bool {
        let Some(child) = self.child.as_mut() else {
            return false;
        };

        if child.try_wait().is_ok_and(|exited| exited.is_none()) {
            return true;
        }

        self.child = None;
        if !self.stopping {
            self.error = Some("Recording failed; the output may have disappeared or wf-recorder could not capture it".into());
        }

        for module in self.modules.drain(..) {
            amane::spawn(&format!("pactl unload-module {module}"));
        }

        let message = if self.stopping {
            "Recording saved to ~/Videos"
        } else {
            "Recording failed, is wf-recorder installed?"
        };

        amane::spawn(&format!("notify-send -a Recorder Recorder '{message}'"));

        true
    }
}

pub fn toggle() {
    let mut recorder = Recorder::write();

    if let Some(child) = recorder.child.as_ref() {
        // SIGINT lets wf-recorder finish writing the file; update() reaps it
        amane::spawn(&format!("kill -INT {}", child.id()));

        recorder.stopping = true;

        return;
    }

    if recorder.starting {
        return;
    }
    recorder.starting = true;
    recorder.error = None;
    let audio = recorder.audio;
    drop(recorder);
    // Mango queries and child startup run outside the service lock and UI thread.
    std::thread::spawn(move || {
        let mut modules = Vec::new();
        let result = (|| -> Result<Child, String> {
            let output = crate::mango::focused_output()?;
            let folder =
                std::path::PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?)
                    .join("Videos");
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            let stamp = Command::new("date")
                .arg("+%F_%H-%M-%S-%N")
                .output()
                .map_err(|e| e.to_string())?;
            let file = folder.join(format!(
                "{}.mp4",
                String::from_utf8_lossy(&stamp.stdout).trim()
            ));
            if audio == Audio::DesktopMic {
                modules = mix();
            }
            let mut command = Command::new("wf-recorder");
            command.arg("-o").arg(output).arg("-f").arg(file);
            let flag = audio.flag();
            if !flag.is_empty() {
                command.arg(flag);
            }
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Cannot start wf-recorder: {e}"))
        })();
        let mut recorder = Recorder::write();
        recorder.starting = false;
        match result {
            Ok(child) => {
                recorder.child = Some(child);
                recorder.modules = modules;
                recorder.started = Instant::now();
                recorder.stopping = false;
            }
            Err(error) => {
                for module in modules {
                    let _ = Command::new("pactl")
                        .args(["unload-module", &module])
                        .spawn();
                }
                recorder.error = Some(error);
            }
        }
    });
}

// ponytail: pactl loopback mix, switch to gpu-screen-recorder if the two sources drift apart
fn mix() -> Vec<String> {
    let sink = format!("sink_name={MIX_SINK}");
    let into = format!("sink={MIX_SINK}");

    let loads: [&[&str]; 3] = [
        &["module-null-sink", &sink],
        &["module-loopback", "source=@DEFAULT_MONITOR@", &into],
        &["module-loopback", "source=@DEFAULT_SOURCE@", &into],
    ];

    let mut modules = Vec::new();

    for arguments in loads {
        let output = Command::new("pactl")
            .arg("load-module")
            .args(arguments)
            .output();

        if let Ok(output) = output
            && output.status.success()
        {
            modules.push(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }

    modules
}

// like "01:23" while recording, for the bar tray and the record button
pub fn status() -> Option<String> {
    Recorder::read().elapsed()
}

fn time(gone: Duration) -> String {
    let seconds = gone.as_secs();

    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

pub fn ipc(_arguments: &[String]) -> String {
    toggle();

    String::from("ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_startup_keeps_captured_audio_until_it_finishes() {
        let mut recorder = Recorder::new();
        recorder.audio = Audio::Mic;
        recorder.starting = true;
        recorder.select_audio(Audio::None);
        assert!(
            recorder.audio == Audio::Mic,
            "startup changed the captured audio mode"
        );
        assert!(recorder.starting());
        recorder.starting = false;
        recorder.select_audio(Audio::None);
        assert!(
            recorder.audio == Audio::None,
            "finished startup kept the controls locked"
        );
    }
}
