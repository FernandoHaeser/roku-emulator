//! `Video` node playback.
//!
//! Device behavior (fidelity/video/lifecycle, H302X Roku OS 16.0.4):
//! - `control = "play"`: `buffering`, then `playing` once the stream is
//!   ready; `duration` is set before `playing` is reported.
//! - `pause` / `resume`: `paused` / `playing`.
//! - `seek`: `buffering`, then `playing` (autoPlayAfterSeek).
//! - End of stream: `finished`.
//! - `stop`: `stopped`, reported during the `control` assignment.
//! - HTTP errors: `errorCode` -1, `errorMsg`/`errorStr` set, then `error`
//!   followed by `finished`.
//!
//! Two backends:
//! - Simulated (default): the engine fetches only what it needs to know the
//!   duration (MP4 `mvhd`, HLS playlists, through the host's asynchronous
//!   HTTP or the package) and advances the position with its own clock. Used
//!   by the CLI, the VS Code runtime and headless hosts.
//! - External: a host with a real player (the web shell) receives commands
//!   from [`take_commands`] and reports events with [`host_event`].

use std::collections::{HashMap, VecDeque};
use std::rc::{Rc, Weak};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value as Json};

use super::runtime::{create_node, set_field};
use super::{node_ref, with_node};
use crate::error::RuntimeError;
use crate::value::{AssocArray, Object, ObjectRef, Value};
use crate::vm::Vm;

type Result<T> = std::result::Result<T, RuntimeError>;

/// Delay between a seek and playback resuming in the simulated backend.
const SEEK_BUFFERING_MS: f64 = 100.0;
/// First bytes requested for a remote MP4: enough for a fast-start `moov`.
const MP4_PROBE_BYTES: u32 = 1024 * 1024;

const HTTP_ERROR_MSG: &str = "There was an error in the HTTP response.  This could mean that malformed HTTP headers or an HTTP error code was returned.";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    /// Waiting for metadata (simulated) or for the host player (external).
    Loading,
    Playing,
    Paused,
    /// Buffering after a seek until the given time (simulated).
    Seeking(f64),
    Done,
}

struct Player {
    id: u32,
    node: Weak<std::cell::RefCell<Object>>,
    phase: Phase,
    duration: f64,
    /// Position at `base_ms` (seconds).
    base_position: f64,
    base_ms: f64,
    /// Last position reported through the `position` field.
    notified_ms: f64,
}

/// Result of a metadata request: HTTP status (negative = transport failure),
/// body, failure text and the URL it answers.
struct Fetched {
    player: u32,
    url: String,
    status: i64,
    body: Vec<u8>,
    failure: String,
}

#[derive(Default)]
pub struct Videos {
    players: Vec<Player>,
    next_id: u32,
    /// Hosts with a real player take commands instead of the simulation.
    external: bool,
    commands: Vec<String>,
    /// Players whose `play` still has to report `buffering`.
    starting: Vec<u32>,
    events: VecDeque<Json>,
    fetched: VecDeque<Fetched>,
    /// HTTP token -> (player id, URL).
    requests: HashMap<u32, (u32, String)>,
}

impl Videos {
    /// Pending host work: metadata requests or playing streams.
    pub fn busy(&self) -> bool {
        !self.requests.is_empty()
            || !self.starting.is_empty()
            || !self.fetched.is_empty()
            || !self.events.is_empty()
            || self
                .players
                .iter()
                .any(|p| matches!(p.phase, Phase::Playing | Phase::Seeking(_) | Phase::Loading))
    }

    /// Earliest time the render thread has to run for playback.
    pub fn next_wake(&self, now: f64, interval_ms: f64) -> Option<f64> {
        if !self.fetched.is_empty() || !self.events.is_empty() || !self.starting.is_empty() {
            return Some(now);
        }
        if self.external {
            return None;
        }
        self.players
            .iter()
            .filter_map(|p| match p.phase {
                Phase::Playing => Some(p.notified_ms + interval_ms),
                Phase::Seeking(until) => Some(until),
                _ => None,
            })
            .reduce(f64::min)
    }
}

/// Switches to a host player (web shell). Call before the channel starts.
pub fn set_external(vm: &mut Vm, external: bool) {
    vm.sg.videos.external = external;
}

/// Commands for the host player since the last call (JSON objects).
pub fn take_commands(vm: &mut Vm) -> Vec<String> {
    std::mem::take(&mut vm.sg.videos.commands)
}

/// Event from the host player: `{"id":1,"type":"playing"}`; types are
/// `buffering`, `playing`, `paused`, `ended`, `duration` (`value`),
/// `position` (`value`) and `error` (`status`, `message`).
pub fn host_event(vm: &mut Vm, event: &str) {
    if let Ok(event) = serde_json::from_str::<Json>(event) {
        vm.sg.videos.events.push_back(event);
    }
}

/// Creates the node-valued parts and native agents a device Video carries.
pub fn initialize(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    if !with_node(node, |n| n.ancestry.iter().any(|t| &**t == "Video")) {
        return Ok(());
    }
    for (field, kind) in [
        ("bifDisplay", "Node"),
        ("bufferingBar", "Node"),
        ("retrievingBar", "Node"),
        ("trickPlayBar", "Node"),
    ] {
        if let Some(part) = create_node(vm, kind)? {
            set_field(vm, node, field, Value::Object(part))?;
        }
    }
    for (field, class) in [
        ("drmHttpAgent", "roHttpAgent"),
        ("mediaPlayerAgent", "roMediaPlayerAgent"),
    ] {
        let agent = Value::object(Object::Native {
            class,
            state: AssocArray::new(),
        });
        set_field(vm, node, field, agent)?;
    }
    Ok(())
}

/// Player id of a Video node that has started playback.
pub fn player_id(vm: &Vm, node: &ObjectRef) -> Option<u32> {
    player_index(vm, node).map(|index| vm.sg.videos.players[index].id)
}

fn player_index(vm: &Vm, node: &ObjectRef) -> Option<usize> {
    vm.sg
        .videos
        .players
        .iter()
        .position(|p| p.node.upgrade().is_some_and(|n| Rc::ptr_eq(&n, node)))
}

fn player_for(vm: &mut Vm, node: &ObjectRef) -> usize {
    if let Some(index) = player_index(vm, node) {
        return index;
    }
    vm.sg.videos.players.retain(|p| p.node.strong_count() > 0);
    vm.sg.videos.next_id += 1;
    vm.sg.videos.players.push(Player {
        id: vm.sg.videos.next_id,
        node: Rc::downgrade(node),
        phase: Phase::Idle,
        duration: 0.0,
        base_position: 0.0,
        base_ms: 0.0,
        notified_ms: 0.0,
    });
    vm.sg.videos.players.len() - 1
}

fn by_id(vm: &Vm, id: u32) -> Option<(usize, ObjectRef)> {
    let index = vm.sg.videos.players.iter().position(|p| p.id == id)?;
    Some((index, vm.sg.videos.players[index].node.upgrade()?))
}

fn set_state(vm: &mut Vm, node: &ObjectRef, state: &str) -> Result<()> {
    set_field(vm, node, "state", Value::string(state))
}

fn command(vm: &mut Vm, id: u32, op: &str, extra: Json) {
    let mut message = json!({"id": id, "op": op});
    if let (Some(target), Some(fields)) = (message.as_object_mut(), extra.as_object()) {
        target.extend(fields.clone());
    }
    vm.sg.videos.commands.push(message.to_string());
}

fn position_now(player: &Player, now: f64) -> f64 {
    match player.phase {
        Phase::Playing => player.base_position + (now - player.base_ms) / 1000.0,
        _ => player.base_position,
    }
}

/// `control` field side effect.
pub fn control(vm: &mut Vm, node: &ObjectRef, control: &str) -> Result<()> {
    let index = player_for(vm, node);
    let now = vm.now_ms();
    let id = vm.sg.videos.players[index].id;
    let external = vm.sg.videos.external;
    let phase = vm.sg.videos.players[index].phase;
    match control {
        "play" => play(vm, node, index),
        "pause" if phase == Phase::Playing => {
            let player = &mut vm.sg.videos.players[index];
            player.base_position = position_now(player, now);
            player.phase = Phase::Paused;
            if external {
                command(vm, id, "pause", json!({}));
            }
            set_state(vm, node, "paused")
        }
        "resume" if phase == Phase::Paused => {
            let player = &mut vm.sg.videos.players[index];
            player.base_ms = now;
            player.notified_ms = now;
            player.phase = Phase::Playing;
            if external {
                command(vm, id, "resume", json!({}));
            }
            set_state(vm, node, "playing")
        }
        "stop" if phase != Phase::Idle => {
            stop_player(vm, index);
            set_state(vm, node, "stopped")
        }
        _ => Ok(()),
    }
}

fn stop_player(vm: &mut Vm, index: usize) {
    let id = vm.sg.videos.players[index].id;
    vm.sg.videos.players[index].phase = Phase::Done;
    vm.sg.videos.requests.retain(|_, (player, _)| *player != id);
    if vm.sg.videos.external {
        command(vm, id, "stop", json!({}));
    }
}

fn content_source(node: &ObjectRef) -> Option<(String, String)> {
    let content = node_ref(&with_node(node, |n| n.raw("content")))?;
    let (url, format) = with_node(&content, |n| (n.text("url"), n.text("streamFormat")));
    (!url.is_empty()).then_some((url, format))
}

fn play(vm: &mut Vm, node: &ObjectRef, index: usize) -> Result<()> {
    let Some((url, format)) = content_source(node) else {
        return Ok(());
    };
    let id = vm.sg.videos.players[index].id;
    vm.sg.videos.requests.retain(|_, (player, _)| *player != id);
    {
        let player = &mut vm.sg.videos.players[index];
        player.phase = Phase::Loading;
        player.duration = 0.0;
        player.base_position = 0.0;
    }
    set_field(vm, node, "errorCode", Value::Integer(0))?;
    set_field(vm, node, "errorMsg", Value::string(""))?;
    set_field(vm, node, "errorStr", Value::string(""))?;
    // The device reports `buffering` after the assignment returns
    // (video/lifecycle: state still reads "none" right after `play`).
    vm.sg.videos.starting.push(id);
    if vm.sg.videos.external {
        command(vm, id, "load", json!({"url": url, "format": format}));
        return Ok(());
    }
    fetch(vm, id, &url, &format);
    Ok(())
}

fn is_mp4(url: &str, format: &str) -> bool {
    format.eq_ignore_ascii_case("mp4")
        || url
            .split(['?', '#'])
            .next()
            .unwrap_or(url)
            .to_ascii_lowercase()
            .ends_with(".mp4")
}

/// Starts a metadata request; the answer arrives in `fetched`.
fn fetch(vm: &mut Vm, player: u32, url: &str, format: &str) {
    if url.starts_with("pkg:/") || url.starts_with("tmp:/") {
        let body = vm.env.read_file(url);
        vm.sg.videos.fetched.push_back(Fetched {
            player,
            url: url.to_string(),
            status: if body.is_some() { 200 } else { 404 },
            body: body.unwrap_or_default(),
            failure: String::new(),
        });
        return;
    }
    let mut headers = json!({});
    if is_mp4(url, format) {
        headers["Range"] = json!(format!("bytes=0-{}", MP4_PROBE_BYTES - 1));
    }
    let request = json!({
        "url": url, "method": "GET", "headers": headers, "body": "",
        "peerVerify": true, "hostVerify": true, "timeout": 30000
    })
    .to_string();
    vm.env.next_identity += 1;
    let token = vm.env.next_identity;
    vm.sg
        .videos
        .requests
        .insert(token, (player, url.to_string()));
    if !vm.host_http_start(token, &request) {
        vm.sg.videos.requests.remove(&token);
        vm.sg.videos.fetched.push_back(Fetched {
            player,
            url: url.to_string(),
            status: -7,
            body: Vec::new(),
            failure: "HTTP host unavailable".into(),
        });
    }
}

/// Host answer for an HTTP token; returns false when it is not a Video one.
pub fn complete_http(vm: &mut Vm, token: u32, response: &str) -> bool {
    let Some((player, url)) = vm.sg.videos.requests.remove(&token) else {
        return false;
    };
    let result: Json = serde_json::from_str(response).unwrap_or(json!({"status": -7}));
    vm.sg.videos.fetched.push_back(Fetched {
        player,
        url,
        status: result["status"].as_i64().unwrap_or(-7),
        body: STANDARD
            .decode(result["body"].as_str().unwrap_or(""))
            .unwrap_or_default(),
        failure: result["failure"].as_str().unwrap_or("").to_string(),
    });
    true
}

/// Render-thread work: metadata answers, host events and the clock.
pub fn service(vm: &mut Vm) -> Result<()> {
    // Video nodes that were garbage collected: release their host players.
    let dropped: Vec<u32> = vm
        .sg
        .videos
        .players
        .iter()
        .filter(|p| p.node.strong_count() == 0)
        .map(|p| p.id)
        .collect();
    if !dropped.is_empty() {
        vm.sg.videos.players.retain(|p| p.node.strong_count() > 0);
        vm.sg
            .videos
            .requests
            .retain(|_, (player, _)| !dropped.contains(player));
        if vm.sg.videos.external {
            for id in dropped {
                command(vm, id, "stop", json!({}));
            }
        }
    }
    for id in std::mem::take(&mut vm.sg.videos.starting) {
        if let Some((index, node)) = by_id(vm, id) {
            if vm.sg.videos.players[index].phase == Phase::Loading {
                set_state(vm, &node, "buffering")?;
            }
        }
    }
    while let Some(fetched) = vm.sg.videos.fetched.pop_front() {
        on_fetched(vm, fetched)?;
    }
    while let Some(event) = vm.sg.videos.events.pop_front() {
        on_host_event(vm, &event)?;
    }
    if !vm.sg.videos.external {
        tick(vm)?;
    }
    Ok(())
}

fn fail(vm: &mut Vm, index: usize, node: &ObjectRef, status: i64, failure: &str) -> Result<()> {
    vm.sg.videos.players[index].phase = Phase::Done;
    let detail = if status > 0 {
        format!(
            "reader pick stream error:HTTP error:Transfer error: HTTP response code said error response code:({status}):{status}:extra:content_id:0:err_clip_idx:0:etype:buffer:program_id:"
        )
    } else {
        format!("reader pick stream error:HTTP error:Transfer error: {failure}")
    };
    set_field(vm, node, "errorCode", Value::Integer(-1))?;
    set_field(vm, node, "errorMsg", Value::string(HTTP_ERROR_MSG))?;
    set_field(vm, node, "errorStr", Value::string(detail.as_str()))?;
    set_state(vm, node, "error")?;
    set_state(vm, node, "finished")
}

fn on_fetched(vm: &mut Vm, fetched: Fetched) -> Result<()> {
    let Some((index, node)) = by_id(vm, fetched.player) else {
        return Ok(());
    };
    if vm.sg.videos.players[index].phase != Phase::Loading {
        return Ok(());
    }
    if !(200..300).contains(&fetched.status) {
        return fail(vm, index, &node, fetched.status, &fetched.failure);
    }
    let text = String::from_utf8_lossy(&fetched.body);
    if text.trim_start().starts_with("#EXTM3U") {
        if let Some(variant) = hls_variant(&text) {
            let url = resolve(&fetched.url, &variant);
            fetch(vm, fetched.player, &url, "hls");
            return Ok(());
        }
        let duration = hls_duration(&text);
        return start(vm, index, &node, duration);
    }
    let duration = mp4_duration(&fetched.body).unwrap_or(0.0);
    start(vm, index, &node, duration)
}

fn start(vm: &mut Vm, index: usize, node: &ObjectRef, duration: f64) -> Result<()> {
    let now = vm.now_ms();
    {
        let player = &mut vm.sg.videos.players[index];
        player.duration = duration;
        player.phase = Phase::Playing;
        player.base_ms = now;
        player.notified_ms = now;
    }
    set_field(vm, node, "duration", Value::Double(duration))?;
    set_state(vm, node, "playing")
}

fn interval_ms(node: &ObjectRef) -> f64 {
    let seconds = with_node(node, |n| n.number("notificationInterval"));
    if seconds > 0.0 {
        seconds * 1000.0
    } else {
        500.0
    }
}

/// Shortest notification interval among playing Videos (for VM wake-ups).
pub fn wake_interval_ms(vm: &Vm) -> f64 {
    vm.sg
        .videos
        .players
        .iter()
        .filter_map(|p| p.node.upgrade())
        .map(|n| interval_ms(&n))
        .reduce(f64::min)
        .unwrap_or(500.0)
}

fn tick(vm: &mut Vm) -> Result<()> {
    let now = vm.now_ms();
    for index in 0..vm.sg.videos.players.len() {
        let Some(node) = vm.sg.videos.players[index].node.upgrade() else {
            continue;
        };
        let player = &vm.sg.videos.players[index];
        match player.phase {
            Phase::Seeking(until) if now >= until => {
                let player = &mut vm.sg.videos.players[index];
                player.phase = Phase::Playing;
                player.base_ms = now;
                player.notified_ms = now;
                set_state(vm, &node, "playing")?;
            }
            Phase::Playing => {
                let position = position_now(player, now);
                let (duration, notified) = (player.duration, player.notified_ms);
                if duration > 0.0 && position >= duration {
                    vm.sg.videos.players[index].phase = Phase::Done;
                    vm.sg.videos.players[index].base_position = duration;
                    set_field(vm, &node, "position", Value::Double(duration))?;
                    set_state(vm, &node, "finished")?;
                } else if now - notified >= interval_ms(&node) {
                    vm.sg.videos.players[index].notified_ms = now;
                    set_field(vm, &node, "position", Value::Double(position))?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// `seek` field side effect.
pub fn seek(vm: &mut Vm, node: &ObjectRef) -> Result<()> {
    let Some(index) = player_index(vm, node) else {
        return Ok(());
    };
    let phase = vm.sg.videos.players[index].phase;
    if !matches!(phase, Phase::Playing | Phase::Paused | Phase::Seeking(_)) {
        return Ok(());
    }
    let target = with_node(node, |n| n.number("seek")).max(0.0);
    let now = vm.now_ms();
    let id = vm.sg.videos.players[index].id;
    {
        let player = &mut vm.sg.videos.players[index];
        player.base_position = if player.duration > 0.0 {
            target.min(player.duration)
        } else {
            target
        };
        player.phase = if vm.sg.videos.external {
            Phase::Loading
        } else {
            Phase::Seeking(now + SEEK_BUFFERING_MS)
        };
    }
    if vm.sg.videos.external {
        command(vm, id, "seek", json!({"position": target}));
    }
    set_state(vm, node, "buffering")
}

fn on_host_event(vm: &mut Vm, event: &Json) -> Result<()> {
    let id = event["id"].as_u64().unwrap_or(0) as u32;
    let Some((index, node)) = by_id(vm, id) else {
        return Ok(());
    };
    let phase = vm.sg.videos.players[index].phase;
    if phase == Phase::Done || phase == Phase::Idle {
        return Ok(());
    }
    let now = vm.now_ms();
    match event["type"].as_str().unwrap_or("") {
        "duration" => {
            let duration = event["value"].as_f64().unwrap_or(0.0);
            vm.sg.videos.players[index].duration = duration;
            set_field(vm, &node, "duration", Value::Double(duration))?;
        }
        "buffering" => set_state(vm, &node, "buffering")?,
        "playing" if phase != Phase::Paused => {
            let player = &mut vm.sg.videos.players[index];
            player.phase = Phase::Playing;
            player.notified_ms = now;
            set_state(vm, &node, "playing")?;
        }
        "position" => {
            let position = event["value"].as_f64().unwrap_or(0.0);
            let player = &mut vm.sg.videos.players[index];
            player.base_position = position;
            player.base_ms = now;
            if now - player.notified_ms >= interval_ms(&node) {
                vm.sg.videos.players[index].notified_ms = now;
                set_field(vm, &node, "position", Value::Double(position))?;
            }
        }
        "ended" => {
            vm.sg.videos.players[index].phase = Phase::Done;
            set_state(vm, &node, "finished")?;
        }
        "error" => {
            let status = event["status"].as_i64().unwrap_or(-7);
            let message = event["message"].as_str().unwrap_or("").to_string();
            fail(vm, index, &node, status, &message)?;
        }
        _ => {}
    }
    Ok(())
}

// ---- metadata parsing ----------------------------------------------------------------

/// First variant URI of an HLS master playlist.
fn hls_variant(playlist: &str) -> Option<String> {
    let mut lines = playlist.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if line.starts_with("#EXT-X-STREAM-INF") {
            return lines
                .find(|l| !l.is_empty() && !l.starts_with('#'))
                .map(String::from);
        }
    }
    None
}

/// Sum of segment durations of an HLS media playlist.
fn hls_duration(playlist: &str) -> f64 {
    playlist
        .lines()
        .filter_map(|line| line.trim().strip_prefix("#EXTINF:"))
        .filter_map(|rest| rest.split(',').next()?.trim().parse::<f64>().ok())
        .sum()
}

/// Resolves a playlist reference against the playlist URL.
fn resolve(base: &str, reference: &str) -> String {
    if reference.contains("://") {
        return reference.to_string();
    }
    if let Some(path) = reference.strip_prefix('/') {
        let origin_end = base
            .find("://")
            .and_then(|scheme| base[scheme + 3..].find('/').map(|i| scheme + 3 + i))
            .unwrap_or(base.len());
        return format!("{}/{path}", &base[..origin_end]);
    }
    let directory = base
        .split(['?', '#'])
        .next()
        .unwrap_or(base)
        .rsplit_once('/')
        .map_or(base, |(dir, _)| dir);
    format!("{directory}/{reference}")
}

/// Duration from the MP4 movie header (`moov/mvhd`), in seconds.
fn mp4_duration(bytes: &[u8]) -> Option<f64> {
    let moov = find_box(bytes, b"moov")?;
    let mvhd = find_box(moov, b"mvhd")?;
    let read32 = |at: usize| Some(u32::from_be_bytes(mvhd.get(at..at + 4)?.try_into().ok()?));
    let read64 = |at: usize| Some(u64::from_be_bytes(mvhd.get(at..at + 8)?.try_into().ok()?));
    let (timescale, duration) = if *mvhd.first()? == 1 {
        (read32(20)?, read64(24)?)
    } else {
        (read32(12)?, u64::from(read32(16)?))
    };
    (timescale > 0).then(|| duration as f64 / f64::from(timescale))
}

/// Payload of the first box named `kind` among `bytes`' top-level boxes.
fn find_box<'a>(bytes: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    let mut at = 0;
    while at + 8 <= bytes.len() {
        let size = u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?) as usize;
        let (header, size) = match size {
            1 => (
                16,
                u64::from_be_bytes(bytes.get(at + 8..at + 16)?.try_into().ok()?) as usize,
            ),
            0 => (8, bytes.len() - at),
            size => (8, size),
        };
        if size < header {
            return None;
        }
        if &bytes[at + 4..at + 8] == kind {
            return bytes.get(at + header..(at + size).min(bytes.len()));
        }
        at += size;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_mp4_duration() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fidelity/video/lifecycle/media/clip.mp4"
        );
        let duration = mp4_duration(&std::fs::read(path).unwrap()).unwrap();
        assert!((duration - 6.0).abs() < 0.05, "{duration}");
    }

    #[test]
    fn parses_hls_playlists() {
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nlow/index.m3u8\n";
        assert_eq!(hls_variant(master).as_deref(), Some("low/index.m3u8"));
        let media = "#EXTM3U\n#EXTINF:10.0,\na.ts\n#EXTINF:4.5,\nb.ts\n#EXT-X-ENDLIST\n";
        assert_eq!(hls_variant(media), None);
        assert_eq!(hls_duration(media), 14.5);
    }

    #[test]
    fn resolves_playlist_references() {
        let base = "https://host/a/b/master.m3u8?x=1";
        assert_eq!(resolve(base, "v/1.m3u8"), "https://host/a/b/v/1.m3u8");
        assert_eq!(resolve(base, "/root.m3u8"), "https://host/root.m3u8");
        assert_eq!(resolve(base, "http://other/x.m3u8"), "http://other/x.m3u8");
    }
}
