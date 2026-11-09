//! `brs run <channel>`: run a channel (directory or zip) natively.
//! `brs conformance <dir>`: run every recorded probe and diff against goldens.

mod http;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};
use std::{env, fs};

use std::collections::VecDeque;

use brs_core::sg::bitmap;
use brs_core::sg::runtime::queue_key;
use brs_core::{launch, Channel, Host, Object, Status, Value};

const PROBE_BEGIN: &str = "@@PROBE_BEGIN\n";
const PROBE_END: &str = "@@PROBE_END\n";
/// Guards conformance runs against probes that never finish.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

struct CaptureHost {
    output: Rc<RefCell<String>>,
    echo: bool,
    started: Instant,
    http_sender: std::sync::mpsc::Sender<(u32, String)>,
    http_receiver: std::sync::mpsc::Receiver<(u32, String)>,
}

impl Host for CaptureHost {
    fn write(&mut self, text: &str) {
        if self.echo {
            print!("{text}");
        }
        self.output.borrow_mut().push_str(text);
    }

    fn now_ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    fn http(&mut self, request: &str) -> String {
        http::request(request)
    }
    fn http_start(&mut self, id: u32, request: &str) -> bool {
        let sender = self.http_sender.clone();
        let request = request.to_string();
        std::thread::spawn(move || {
            let _ = sender.send((id, http::request(&request)));
        });
        true
    }
    fn http_poll(&mut self) -> Vec<(u32, String)> {
        self.http_receiver.try_iter().collect()
    }
    fn epoch_ms(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    }
}

fn load_channel(path: &Path, package_md5: Option<String>) -> Result<Channel, String> {
    if path.is_file() {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        return Channel::from_zip(&bytes).map_err(|e| e.to_string());
    }
    let mut files = BTreeMap::new();
    collect_files(path, path, &mut files)?;
    let mut channel = Channel::from_files(files).map_err(|e| e.to_string())?;
    channel.package_md5 = package_md5;
    Ok(channel)
}

fn collect_files(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if dir == root && (name == "golden" || name == "probe.json") || name.starts_with('.') {
            continue;
        }
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        if file_type.is_symlink() {
            return Err(format!(
                "Symlinks are not allowed in channels: {}",
                path.display()
            ));
        }
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else {
            let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
            files.insert(
                relative.to_string_lossy().replace('\\', "/"),
                fs::read(&path).map_err(|e| e.to_string())?,
            );
        }
    }
    Ok(())
}

/// Remote key steps from probe.json, sent after the BEGIN marker.
#[derive(Default)]
struct KeyScript {
    start_delay: Duration,
    steps: VecDeque<(String, Duration)>,
}

fn key_script(probe: &Path) -> KeyScript {
    let mut script = KeyScript {
        start_delay: Duration::from_millis(1000),
        steps: VecDeque::new(),
    };
    let Ok(text) = fs::read_to_string(probe.join("probe.json")) else {
        return script;
    };
    let Ok(Value::Object(spec)) = brs_core::json::parse(&text) else {
        return script;
    };
    let Object::AssocArray(spec) = &*spec.borrow() else {
        return script;
    };
    if let Some(delay) = spec.get("startDelayMs").and_then(Value::as_f64) {
        script.start_delay = Duration::from_millis(delay as u64);
    }
    if let Some(Value::Object(steps)) = spec.get("steps") {
        if let Object::Array(steps) = &*steps.borrow() {
            for step in steps {
                let Value::Object(step) = step else { continue };
                let Object::AssocArray(step) = &*step.borrow() else {
                    continue;
                };
                let key = step.get("key").and_then(|k| k.as_str().map(str::to_string));
                let wait = step.get("waitMs").and_then(Value::as_f64).unwrap_or(300.0);
                if let Some(key) = key {
                    script
                        .steps
                        .push_back((key, Duration::from_millis(wait as u64)));
                }
            }
        }
    }
    script
}

/// Runs a channel until it ends, crashes, times out or (with `stop_at_end`)
/// prints the probe END marker. Returns its console output.
fn execute(
    channel: &Channel,
    echo: bool,
    timeout: Option<Duration>,
    mut keys: KeyScript,
    stop_at_end: bool,
) -> (String, Status, Duration) {
    let output = Rc::new(RefCell::new(String::new()));
    let (http_sender, http_receiver) = std::sync::mpsc::channel();
    let host = CaptureHost {
        output: output.clone(),
        echo,
        started: Instant::now(),
        http_sender,
        http_receiver,
    };
    let started = Instant::now();
    let mut next_key_at: Option<Instant> = None;
    // Remote Poster images: fetched on threads, answered between slices.
    let (image_sender, image_receiver) = std::sync::mpsc::channel();
    let mut images_in_flight = 0usize;
    let status = match launch(channel, Box::new(host)) {
        Ok(mut vm) => loop {
            let status = vm.run(200_000);
            for uri in bitmap::take_requests(&mut vm) {
                let sender = image_sender.clone();
                images_in_flight += 1;
                std::thread::spawn(move || {
                    let size = http::image_size(&uri);
                    let _ = sender.send((uri, size));
                });
            }
            for (uri, size) in image_receiver.try_iter() {
                images_in_flight -= 1;
                bitmap::complete(&mut vm, &uri, size);
            }
            let (begun, ended) = {
                let text = output.borrow();
                let begin = text.rfind(PROBE_BEGIN);
                (
                    begin.is_some(),
                    begin.is_some_and(|b| text[b..].contains(PROBE_END)),
                )
            };
            if stop_at_end && ended {
                break Status::Finished;
            }
            if begun && !keys.steps.is_empty() {
                let due = *next_key_at.get_or_insert_with(|| Instant::now() + keys.start_delay);
                if Instant::now() >= due {
                    let (key, wait) = keys.steps.pop_front().expect("step");
                    queue_key(&mut vm, &key, true);
                    queue_key(&mut vm, &key, false);
                    next_key_at = Some(Instant::now() + wait);
                    continue;
                }
            }
            let until_key = next_key_at
                .filter(|_| !keys.steps.is_empty())
                .map(|at| at.saturating_duration_since(Instant::now()));
            match status {
                Status::Running => {}
                Status::Sleeping { until_ms } => {
                    let now = started.elapsed().as_secs_f64() * 1000.0;
                    let sleep = Duration::from_secs_f64(((until_ms - now).max(0.0)) / 1000.0);
                    std::thread::sleep(
                        until_key
                            .map_or(sleep, |k| k.min(sleep))
                            .min(Duration::from_millis(50)),
                    );
                }
                Status::Waiting => match until_key {
                    Some(wait) => std::thread::sleep(wait.min(Duration::from_millis(50))),
                    None if begun && !keys.steps.is_empty() => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    None if !vm.env.http_pending.is_empty() || images_in_flight > 0 => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    None => break Status::Waiting,
                },
                other => break other,
            }
            if timeout.is_some_and(|limit| started.elapsed() > limit) {
                break Status::Running;
            }
        },
        Err(message) => {
            output.borrow_mut().push_str(&format!("{message}\n"));
            if echo {
                println!("{message}");
            }
            Status::Crashed
        }
    };
    let text = output.borrow().clone();
    (text, status, started.elapsed())
}

/// Same extraction rule as the oracle (packages/oracle/src/probe.ts).
fn extract_probe_output(console: &str) -> (&'static str, String) {
    let Some(begin) = console.rfind(PROBE_BEGIN) else {
        return ("incomplete", console.to_string());
    };
    let rest = &console[begin + PROBE_BEGIN.len()..];
    match rest.find(PROBE_END) {
        Some(end) => ("ok", rest[..end].to_string()),
        None if rest.contains("BrightScript Micro Debugger.") => ("crashed", rest.to_string()),
        None => ("incomplete", rest.to_string()),
    }
}

fn json_string_field(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let after = &json[json.find(&key)? + key.len()..];
    let start = after.find('"')? + 1;
    let end = start + after[start..].find('"')?;
    Some(after[start..end].to_string())
}

fn find_probes(dir: &Path, probes: &mut Vec<PathBuf>) {
    if dir.join("manifest").is_file() {
        probes.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut children: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    children.sort();
    for child in children {
        find_probes(&child, probes);
    }
}

fn first_difference(expected: &str, actual: &str) -> Option<(usize, String, String)> {
    let expected_lines: Vec<&str> = expected.split('\n').collect();
    let actual_lines: Vec<&str> = actual.split('\n').collect();
    (0..expected_lines.len().max(actual_lines.len())).find_map(|i| {
        let (e, a) = (expected_lines.get(i), actual_lines.get(i));
        (e != a).then(|| {
            (
                i + 1,
                e.map_or("<none>".into(), |s| format!("{s:?}")),
                a.map_or("<none>".into(), |s| format!("{s:?}")),
            )
        })
    })
}

fn conformance(root: &Path) -> ExitCode {
    let mut probes = Vec::new();
    find_probes(root, &mut probes);
    let (mut passed, mut failed, mut skipped) = (0, 0, 0);
    for probe in probes {
        let name = probe
            .strip_prefix(root)
            .unwrap_or(&probe)
            .display()
            .to_string();
        let golden = probe.join("golden");
        let Ok(meta) = fs::read_to_string(golden.join("meta.json")) else {
            println!("SKIP  {name} (no golden)");
            skipped += 1;
            continue;
        };
        let channel = match load_channel(&probe, json_string_field(&meta, "packageMd5")) {
            Ok(channel) => channel,
            Err(error) => {
                println!("FAIL  {name}: {error}");
                failed += 1;
                continue;
            }
        };
        let (console, _, elapsed) = execute(
            &channel,
            false,
            Some(PROBE_TIMEOUT),
            key_script(&probe),
            true,
        );
        let (status, output) = extract_probe_output(&console);
        if json_string_field(&meta, "kind").as_deref() == Some("bench") {
            println!("BENCH {name} ({} ms)", elapsed.as_millis());
            let device = fs::read_to_string(golden.join("console.txt")).unwrap_or_default();
            for (emulator_line, device_line) in output.lines().zip(device.lines()) {
                println!("      emulator {emulator_line:<28} device {device_line}");
            }
            continue;
        }
        let expected = fs::read_to_string(golden.join("console.txt")).unwrap_or_default();
        if json_string_field(&meta, "kind").as_deref() == Some("calibration") {
            let total = expected.lines().count();
            let exact = expected
                .lines()
                .zip(output.lines())
                .filter(|(device, emulator)| device == emulator)
                .count();
            println!("CALIB {name} ({exact}/{total} lines exact)");
            continue;
        }
        if output == expected {
            println!("PASS  {name}");
            passed += 1;
        } else {
            failed += 1;
            println!("FAIL  {name} ({status})");
            if let Some((line, expected_line, actual_line)) = first_difference(&expected, &output) {
                println!("      line {line}\n      device:   {expected_line}\n      emulator: {actual_line}");
            }
        }
    }
    println!("\n{passed} passed, {failed} failed, {skipped} skipped");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [command, path] if command == "run" => match load_channel(Path::new(path), None) {
            Ok(channel) => {
                let (_, status, _) = execute(&channel, true, None, KeyScript::default(), false);
                if status == Status::Finished {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        [command, path] if command == "conformance" => conformance(Path::new(path)),
        _ => {
            eprintln!("Usage:\n  brs run <channel dir or zip>\n  brs conformance <fidelity dir>");
            ExitCode::from(2)
        }
    }
}
