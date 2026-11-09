//! OS-level console lines printed around a channel's life (port 8085), as the
//! device does. Debuggers key off them: the VS Code BrightScript extension
//! waits for `AppCompileComplete`, `scrpt.ctx.run.enter` and `AppExitComplete`.
//!
//! Format from the raw console of the reference device (see
//! `.oracle/fidelity/**/raw-console.log`):
//!
//! ```text
//! 11-09 03:50:08.080 app  [beacon.signal] |AppCompileComplete --------> Duration(74 ms)
//! ```
//!
//! The exit lines are **provisional** (not recorded yet; probe
//! `fidelity/system/app-exit`): only `AppExitComplete` is emitted, in the
//! shape the extension matches.

use crate::components::civil_from_days;

/// Width of a beacon name plus its dash leader (`|AppLaunchInitiate ---------> `).
const BEACON_FIELD_WIDTH: usize = 26;

/// `MM-DD HH:MM:SS.mmm` in the device's local time.
pub fn device_timestamp(epoch_ms: f64, utc_offset_minutes: i32) -> String {
    let local = epoch_ms as i64 + i64::from(utc_offset_minutes) * 60_000;
    let days = local.div_euclid(86_400_000);
    let rest = local.rem_euclid(86_400_000);
    let (_, month, day) = civil_from_days(days);
    format!(
        "{month:02}-{day:02} {:02}:{:02}:{:02}.{:03}",
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000
    )
}

/// One timestamped OS log line from the `app` source.
pub fn log_line(timestamp: &str, text: &str) -> String {
    format!("{timestamp} app  {text}\n")
}

/// `[beacon.signal] |Name -----> detail` with the device's dash padding.
pub fn beacon(kind: &str, name: &str, detail: &str) -> String {
    let dashes = BEACON_FIELD_WIDTH.saturating_sub(name.len()).max(1);
    format!("[beacon.{kind}] |{name} {}> {detail}", "-".repeat(dashes))
}

/// Lines printed while a dev channel is compiled and started, ending with the
/// `------ Running dev ... main ------` banner.
pub fn launch_lines(timestamp: &str, title: &str, compile_ms: u64) -> String {
    let mut out = String::new();
    out.push_str(&log_line(
        timestamp,
        &beacon("signal", "AppLaunchInitiate", "TimeBase(0 ms)"),
    ));
    out.push_str(&log_line(
        timestamp,
        &beacon("signal", "AppCompileInitiate", "TimeBase(0 ms)"),
    ));
    out.push_str(&log_line(
        timestamp,
        &format!("[scrpt.cmpl] Compiling '{title}', id 'dev'"),
    ));
    out.push_str(&format!("------ Compiling dev '{title}' ------\n"));
    out.push_str(&log_line(
        timestamp,
        &format!("[scrpt.ctx.cmpl.time] Compiled '{title}', id 'dev' in {compile_ms} ms (BCVer:0)"),
    ));
    out.push_str(&log_line(
        timestamp,
        &beacon(
            "signal",
            "AppCompileComplete",
            &format!("Duration({compile_ms} ms)"),
        ),
    ));
    out.push_str(&log_line(
        timestamp,
        &format!("[scrpt.ctx.run.enter] UI: Entering '{title}', id 'dev'"),
    ));
    out.push_str(&format!("\n------ Running dev '{title}' main ------\n"));
    out
}

/// Lines printed when the channel exits (provisional, see module docs).
pub fn exit_lines(timestamp: &str) -> String {
    log_line(
        timestamp,
        &beacon("report", "AppExitComplete", "Duration(0 ms)"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacons_use_the_device_padding() {
        // Exact lines from the reference device's raw console.
        assert_eq!(
            beacon("signal", "AppLaunchInitiate", "TimeBase(0 ms)"),
            "[beacon.signal] |AppLaunchInitiate ---------> TimeBase(0 ms)"
        );
        assert_eq!(
            beacon("signal", "AppCompileComplete", "Duration(74 ms)"),
            "[beacon.signal] |AppCompileComplete --------> Duration(74 ms)"
        );
        assert_eq!(
            beacon(
                "signal",
                "AppLaunchChainInitiate",
                "TimeBase(NotApplicable)"
            ),
            "[beacon.signal] |AppLaunchChainInitiate ----> TimeBase(NotApplicable)"
        );
    }

    #[test]
    fn timestamps_are_local_month_day_time() {
        // 2026-11-09T06:50:08.002Z in UTC-3.
        let epoch = 1_794_207_008_002.0;
        assert_eq!(device_timestamp(epoch, -180), "11-09 03:50:08.002");
    }
}
