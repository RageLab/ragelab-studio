use std::{
    backtrace::Backtrace,
    fs::{self, OpenOptions},
    io::Write,
    panic,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::{json, Value};

struct DiagnosticsState {
    root: PathBuf,
    marker: PathBuf,
    session_id: String,
    session_log: PathBuf,
    lock: Mutex<()>,
}

static STATE: OnceLock<DiagnosticsState> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsSummary {
    pub(crate) directory: String,
    pub(crate) session_id: String,
    pub(crate) session_log: String,
    pub(crate) latest_incident: Option<String>,
    pub(crate) previous_abnormal_exit_detected: bool,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn diagnostics_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("RageLab")
        .join("diagnostics")
}

fn incident_path(root: &Path, prefix: &str) -> PathBuf {
    root.join(format!("{prefix}-{}-{}.json", now_ms(), std::process::id()))
}

fn latest_incident(root: &Path) -> Option<PathBuf> {
    let mut entries = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with("panic-")
                || name.starts_with("fatal-")
                || name.starts_with("abnormal-exit-"))
            {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(modified, _)| *modified);
    entries.pop().map(|(_, path)| path)
}

pub(crate) fn init(core_revision: &'static str) {
    let root = diagnostics_root();
    if let Err(error) = fs::create_dir_all(&root) {
        eprintln!("RageLab diagnostics unavailable: {error}");
        return;
    }

    let marker = root.join("active-session.json");
    let previous_marker = fs::read_to_string(&marker).ok();
    let previous_abnormal_exit_detected = previous_marker.is_some();

    if let Some(previous) = previous_marker {
        let parsed = serde_json::from_str::<Value>(&previous).unwrap_or_else(|_| {
            json!({
                "raw": previous,
            })
        });
        let report = json!({
            "kind": "abnormal_exit",
            "detectedAtUnixMs": now_ms(),
            "detectedByPid": std::process::id(),
            "message": "The previous RageLab Studio session did not clear its active-session marker.",
            "previousSession": parsed,
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
            let _ = fs::write(incident_path(&root, "abnormal-exit"), bytes);
        }
    }

    let started_at = now_ms();
    let session_id = format!("{started_at}-{}", std::process::id());
    let session_log = root.join(format!("session-{session_id}.jsonl"));

    let state = DiagnosticsState {
        root: root.clone(),
        marker: marker.clone(),
        session_id: session_id.clone(),
        session_log: session_log.clone(),
        lock: Mutex::new(()),
    };

    if STATE.set(state).is_err() {
        return;
    }

    let marker_value = json!({
        "sessionId": session_id,
        "pid": std::process::id(),
        "startedAtUnixMs": started_at,
        "studioVersion": env!("CARGO_PKG_VERSION"),
        "coreRevision": core_revision,
        "executable": std::env::current_exe().ok().map(|path| path.display().to_string()),
        "sessionLog": session_log.display().to_string(),
    });
    if let Ok(bytes) = serde_json::to_vec_pretty(&marker_value) {
        let _ = fs::write(&marker, bytes);
    }

    record_event(
        "session",
        "started",
        json!({
            "studioVersion": env!("CARGO_PKG_VERSION"),
            "coreRevision": core_revision,
            "previousAbnormalExitDetected": previous_abnormal_exit_detected,
        }),
    );

    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        record_panic(info);
        previous_hook(info);
    }));
}

pub(crate) fn mark_graceful_exit() {
    record_event("session", "graceful_exit", json!({}));
    if let Some(state) = STATE.get() {
        let _guard = state.lock.lock().ok();
        let _ = fs::remove_file(&state.marker);
    }
}

pub(crate) fn record_event(category: &str, event: &str, details: Value) {
    let Some(state) = STATE.get() else {
        return;
    };
    let current_thread = std::thread::current();
    let record = json!({
        "timestampUnixMs": now_ms(),
        "sessionId": state.session_id,
        "pid": std::process::id(),
        "thread": current_thread.name().unwrap_or("unnamed"),
        "category": category,
        "event": event,
        "details": details,
    });

    let Ok(_guard) = state.lock.lock() else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&state.session_log)
    {
        if serde_json::to_writer(&mut file, &record).is_ok() {
            let _ = file.write_all(b"\n");
            let _ = file.flush();
        }
    }
}

pub(crate) fn record_fatal(category: &str, event: &str, details: Value) {
    record_event(category, event, details.clone());
    let Some(state) = STATE.get() else {
        return;
    };
    let current_thread = std::thread::current();
    let report = json!({
        "kind": "fatal",
        "timestampUnixMs": now_ms(),
        "sessionId": state.session_id,
        "pid": std::process::id(),
        "thread": current_thread.name().unwrap_or("unnamed"),
        "category": category,
        "event": event,
        "details": details,
        "backtrace": Backtrace::force_capture().to_string(),
        "sessionLog": state.session_log.display().to_string(),
    });
    if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
        let _ = fs::write(incident_path(&state.root, "fatal"), bytes);
    }
}

fn record_panic(info: &panic::PanicHookInfo<'_>) {
    let Some(state) = STATE.get() else {
        return;
    };
    let message = if let Some(message) = info.payload().downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = info.payload().downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    };
    let location = info.location().map(|location| {
        json!({
            "file": location.file(),
            "line": location.line(),
            "column": location.column(),
        })
    });
    let current_thread = std::thread::current();
    let report = json!({
        "kind": "panic",
        "timestampUnixMs": now_ms(),
        "sessionId": state.session_id,
        "pid": std::process::id(),
        "thread": current_thread.name().unwrap_or("unnamed"),
        "message": message,
        "location": location,
        "backtrace": Backtrace::force_capture().to_string(),
        "sessionLog": state.session_log.display().to_string(),
    });

    record_event(
        "panic",
        "panic_hook",
        json!({
            "message": report["message"],
            "location": report["location"],
        }),
    );
    if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
        let _ = fs::write(incident_path(&state.root, "panic"), bytes);
    }
}

#[tauri::command]
pub(crate) fn diagnostics_summary() -> DiagnosticsSummary {
    if let Some(state) = STATE.get() {
        DiagnosticsSummary {
            directory: state.root.display().to_string(),
            session_id: state.session_id.clone(),
            session_log: state.session_log.display().to_string(),
            latest_incident: latest_incident(&state.root).map(|path| path.display().to_string()),
            previous_abnormal_exit_detected: latest_incident(&state.root)
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .is_some_and(|name| name.starts_with("abnormal-exit-")),
        }
    } else {
        let root = diagnostics_root();
        DiagnosticsSummary {
            directory: root.display().to_string(),
            session_id: String::new(),
            session_log: String::new(),
            latest_incident: latest_incident(&root).map(|path| path.display().to_string()),
            previous_abnormal_exit_detected: false,
        }
    }
}

#[tauri::command]
pub(crate) fn diagnostics_record_frontend_error(
    message: String,
    stack: Option<String>,
    context: Option<String>,
) {
    record_event(
        "frontend",
        "unhandled_error",
        json!({
            "message": message,
            "stack": stack,
            "context": context,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_root_is_absolute() {
        assert!(diagnostics_root().is_absolute());
    }
}
