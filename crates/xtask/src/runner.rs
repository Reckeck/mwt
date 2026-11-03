//! Running a selected graph: locks, signals, the live block, the summary.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::graph::{self, Opts, Report, Status, Task};
use crate::sink::Sink;
use crate::{guard, sccache};

pub struct Finished {
    pub reports: Vec<Report>,
    pub total: Duration,
    pub stats_text: Option<String>,
}

impl Finished {
    pub fn failed(&self) -> bool {
        self.reports
            .iter()
            .any(|report| report.status == Status::Fail)
    }
}

pub async fn run_tasks(
    tasks: Vec<Task>,
    opts: Opts,
    root: &std::path::Path,
    sink: &Arc<Sink>,
    cancel: &CancellationToken,
) -> Finished {
    sccache::prepare(sink);
    let schemas = guard::SchemasGuard::take(root);
    sink.start_live();
    let reports = graph::run_graph(
        tasks,
        opts,
        cancel.clone(),
        sink.clone(),
        root,
        &root.join(".cache/xtask"),
    )
    .await;
    sink.stop_live();
    schemas.restore();
    let total = sink.elapsed();
    graph::summary(&reports, total, sink);
    let stats_text = sccache::stats_text();
    if let Some(text) = &stats_text {
        sink.line(&sccache::format_stats(text));
    }
    Finished {
        reports,
        total,
        stats_text,
    }
}

pub async fn selftest(sink: &Sink, cancel: &CancellationToken, seconds: u64, code: i32) -> i32 {
    sink.line("start  sleep");
    tokio::select! {
        () = tokio::time::sleep(Duration::from_secs(seconds)) => sink.line("OK     sleep"),
        () = cancel.cancelled() => sink.line("CANCELLED sleep"),
    }
    code
}

/// Ctrl+C and SIGTERM cancel the run like a failure does. The returned token is set when that happened.
pub fn watch_signals(cancel: CancellationToken) -> CancellationToken {
    let interrupted = CancellationToken::new();
    let flag = interrupted.clone();
    tokio::spawn(async move {
        wait_for_signal().await;
        flag.cancel();
        cancel.cancel();
    });
    interrupted
}

#[cfg(unix)]
async fn wait_for_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut term), Ok(mut hup)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::hangup()),
    ) else {
        let _ = tokio::signal::ctrl_c().await;
        return;
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
        _ = hup.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
