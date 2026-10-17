//! Native close interception. Event-loop callbacks only prevent exit and enqueue work.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};
use voids3_app::App;

const IDLE: u8 = 0;
const CHECKING: u8 = 1;
const CANCELLING: u8 = 2;
const READY: u8 = 3;
#[derive(Default)]
pub struct Shutdown(AtomicU8);
impl Shutdown {
    fn request(&self) -> bool {
        self.0
            .compare_exchange(IDLE, CHECKING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn ready(&self) -> bool {
        self.0.load(Ordering::Acquire) == READY
    }
}

pub fn on_event(handle: &AppHandle, event: RunEvent) {
    let shutdown = handle.state::<Shutdown>();
    if shutdown.ready() {
        return;
    }
    match event {
        RunEvent::WindowEvent {
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } => api.prevent_close(),
        RunEvent::ExitRequested { api, .. } => api.prevent_exit(),
        _ => return,
    }
    if shutdown.request() {
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            close(handle).await;
        });
    }
}

async fn dialog(handle: &AppHandle, title: &str, message: String, action: Option<&str>) -> bool {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut builder = handle
        .dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning);
    if let Some(window) = handle.get_webview_window("main") {
        builder = builder.parent(&window);
    }
    builder = match action {
        // Reserve the native cancel slot for dismissal/Escape on every OS.
        // Keep open is first/default; the destructive action is the No button.
        Some(action) => builder.buttons(MessageDialogButtons::YesNoCancelCustom(
            "Keep open".into(),
            action.into(),
            "Cancel".into(),
        )),
        None => builder.buttons(MessageDialogButtons::OkCustom("Keep open".into())),
    };
    let label = action.map(str::to_owned);
    builder.show_with_result(move |result| {
        let confirmed = confirms_close(result, label.as_deref());
        let _ = sender.send(confirmed);
    });
    receiver.await.unwrap_or(false)
}

fn confirms_close(result: MessageDialogResult, action: Option<&str>) -> bool {
    matches!((result, action), (MessageDialogResult::Custom(result), Some(action)) if result == action)
}

fn title(handle: &AppHandle, title: &str) {
    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.set_title(title);
    }
}

async fn close(handle: AppHandle) {
    let app = handle.state::<Arc<App>>().inner().clone();
    let engine = app.engine.clone();
    let check = tokio::task::spawn_blocking(move || engine.begin_close()).await;
    let (guard, summary) = match check {
        Ok(Ok(check)) => check,
        error => {
            let message = match error {
                Ok(Err(error)) => error.to_string(),
                Err(error) => format!("Could not check transfers: {error}"),
                _ => unreachable!(),
            };
            dialog(&handle, "Cannot close VoidS3 yet", message, None).await;
            handle.state::<Shutdown>().0.store(IDLE, Ordering::Release);
            return;
        }
    };
    if summary.needs_confirmation() && !dialog(&handle, "Transfers are still pending", format!(
        "{} transfer(s) and {} upload batch(es) are still running, queued, paused, or waiting for a decision. Background preparation or cleanup may also be running.\n\nClosing will cancel all pending transfers and stop discovery. VoidS3 will attempt to abort unfinished multipart uploads and remove its partial downloads before exiting. Completed files stay in place.\n\nDo you want to cancel transfers and close?",
        summary.transfers, summary.batches), Some("Cancel transfers and close")).await {
        drop(guard);
        handle.state::<Shutdown>().0.store(IDLE, Ordering::Release);
        return;
    }
    handle
        .state::<Shutdown>()
        .0
        .store(CANCELLING, Ordering::Release);
    title(&handle, "VoidS3 — Cancelling transfers and cleaning up…");
    let cleanup = tokio::time::timeout(Duration::from_secs(30), app.cancel_and_drain()).await;
    let problem = match cleanup {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error.to_string()),
        Err(_) => Some("Cleanup is taking longer than 30 seconds. The provider or filesystem may be unavailable. Some cleanup may still be running.".into()),
    };
    if let Some(problem) = problem && !dialog(&handle, "Transfer cleanup is incomplete", format!(
        "{problem}\n\nKeep the app open to inspect the queue and retry cancellation. If you close anyway, unfinished multipart uploads or partial downloads may remain. Saved queue records are retained for recovery."), Some("Close anyway")).await {
        title(&handle, "VoidS3");
        drop(guard);
        handle.state::<Shutdown>().0.store(IDLE, Ordering::Release);
        return;
    }
    // Keep admission closed until the process exits; this callback's programmatic
    // ExitRequested event is the only one allowed through the interceptor.
    handle.state::<Shutdown>().0.store(READY, Ordering::Release);
    handle.exit(0);
    // AppHandle::exit queues an event rather than terminating synchronously.
    // Retain the guard in managed state until the event loop actually exits.
    handle.manage(guard);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escape_dismissal_and_default_keep_open_never_confirm_exit() {
        let action = Some("Cancel transfers and close");
        for result in [
            MessageDialogResult::Cancel,
            MessageDialogResult::Ok,
            MessageDialogResult::No,
            MessageDialogResult::Custom("Keep open".into()),
            MessageDialogResult::Custom("Cancel".into()),
        ] {
            assert!(!confirms_close(result, action));
        }
        assert!(confirms_close(
            MessageDialogResult::Custom("Cancel transfers and close".into()),
            action
        ));
        assert!(!confirms_close(
            MessageDialogResult::Custom("Close anyway".into()),
            action
        ));
        assert!(!confirms_close(
            MessageDialogResult::Custom("Close anyway".into()),
            None
        ));
    }
    #[test]
    fn repeated_close_requests_do_not_open_duplicate_dialogs_or_bypass_cleanup() {
        let shutdown = Shutdown::default();
        assert!(shutdown.request());
        assert!(!shutdown.request());
        assert!(!shutdown.ready());
        shutdown.0.store(CANCELLING, Ordering::Release);
        assert!(!shutdown.request());
        assert!(!shutdown.ready());
        shutdown.0.store(IDLE, Ordering::Release);
        assert!(shutdown.request());
        shutdown.0.store(READY, Ordering::Release);
        assert!(shutdown.ready());
    }
}
