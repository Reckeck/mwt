//! Task graph: dependencies, resource classes, fail-fast, logs, summary.

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Semaphore, watch};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::exec::{Cmd, Ctx, ExecEnd};
use crate::sink::{Sink, fmt_secs};

/// What a task is limited by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// No limit.
    Light,
    /// A cargo build: at most `Opts::heavy` at once.
    Heavy,
    /// Headless browser (layout, links): at most `Opts::browser` at once, after the cargo builds.
    Browser,
}

pub enum Outcome {
    Ok,
    /// Not an error: this machine cannot run the task. The reason goes to the summary.
    Skip(String),
    Fail(String),
    Cancelled,
}

type Action = Box<dyn Fn(Ctx) -> Pin<Box<dyn Future<Output = Outcome> + Send>> + Send + Sync>;

pub struct Task {
    pub name: String,
    pub deps: Vec<String>,
    /// Soft ordering: waits for these when they are in the graph, never pulls them in.
    pub after: Vec<String>,
    pub class: Class,
    action: Action,
}

impl Task {
    pub fn func<F, Fut>(name: &str, class: Class, deps: &[&str], action: F) -> Self
    where
        F: Fn(Ctx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Outcome> + Send + 'static,
    {
        Self {
            name: name.to_string(),
            deps: deps.iter().map(|dep| (*dep).to_string()).collect(),
            after: Vec::new(),
            class,
            action: Box::new(move |ctx| Box::pin(action(ctx))),
        }
    }

    pub fn after(mut self, names: &[&str]) -> Self {
        self.after
            .extend(names.iter().map(|name| (*name).to_string()));
        self
    }

    pub fn cmd(name: &str, class: Class, deps: &[&str], cmd: Cmd) -> Self {
        Self::func(name, class, deps, move |ctx| {
            let cmd = cmd.clone();
            async move { run_cmd(&ctx, &cmd).await }
        })
    }
}

pub async fn run_cmd(ctx: &Ctx, cmd: &Cmd) -> Outcome {
    match ctx.exec(cmd).await {
        Ok(ExecEnd::Done(0)) => Outcome::Ok,
        Ok(ExecEnd::Done(code)) => {
            Outcome::Fail(format!("`{}` exited with {code}", short(&cmd.display())))
        }
        Ok(ExecEnd::Cancelled) => Outcome::Cancelled,
        Err(error) => Outcome::Fail(format!("{error:#}")),
    }
}

/// First line, at most 90 characters: `bash -c '<script>'` would fill the screen.
fn short(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    match line.char_indices().nth(90) {
        Some((end, _)) => format!("{}...", &line[..end]),
        None => line.to_string(),
    }
}

#[derive(Clone, Copy)]
pub struct Opts {
    pub heavy: usize,
    pub browser: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Ok,
    Skip,
    Fail,
    Cancelled,
}

pub struct Report {
    pub name: String,
    pub status: Status,
    pub took: Duration,
    pub note: String,
}

pub fn log_name(name: &str) -> String {
    name.replace([':', '/'], "-")
}

/// Runs the graph. The first FAIL cancels everything running and not started yet.
pub async fn run_graph(
    tasks: Vec<Task>,
    opts: Opts,
    cancel: CancellationToken,
    sink: Arc<Sink>,
    root: &Path,
    log_dir: &Path,
) -> Vec<Report> {
    let _ = std::fs::create_dir_all(log_dir);
    let heavy = Arc::new(Semaphore::new(opts.heavy.max(1)));
    let browser = Arc::new(Semaphore::new(opts.browser.max(1)));
    let done: HashMap<String, watch::Sender<bool>> = tasks
        .iter()
        .map(|task| (task.name.clone(), watch::channel(false).0))
        .collect();
    let order: Vec<String> = tasks.iter().map(|task| task.name.clone()).collect();
    let mut set = JoinSet::new();
    for (index, task) in tasks.into_iter().enumerate() {
        let waits: Vec<watch::Receiver<bool>> = task
            .deps
            .iter()
            .chain(&task.after)
            .filter_map(|dep| done.get(dep))
            .map(watch::Sender::subscribe)
            .collect();
        let finished = done[&task.name].clone();
        let semaphore = match task.class {
            Class::Light => None,
            Class::Heavy => Some(heavy.clone()),
            Class::Browser => Some(browser.clone()),
        };
        let ctx = Ctx {
            root: root.to_path_buf(),
            log: log_dir.join(format!("{}.log", log_name(&task.name))),
            cancel: cancel.clone(),
        };
        let sink = sink.clone();
        set.spawn(async move {
            // Semaphores are fair: staggering the start keeps the declared order (the long tasks first).
            tokio::time::sleep(Duration::from_millis(3 * index as u64)).await;
            let report = run_one(task, ctx, waits, semaphore, &sink).await;
            let _ = finished.send(true);
            report
        });
    }
    let mut reports = HashMap::new();
    while let Some(joined) = set.join_next().await {
        if let Ok((report, failed)) = joined {
            if failed {
                cancel.cancel();
            }
            reports.insert(report.name.clone(), report);
        }
    }
    order
        .into_iter()
        .filter_map(|name| reports.remove(&name))
        .collect()
}

async fn run_one(
    task: Task,
    ctx: Ctx,
    waits: Vec<watch::Receiver<bool>>,
    semaphore: Option<Arc<Semaphore>>,
    sink: &Sink,
) -> (Report, bool) {
    let cancelled = |took| {
        (
            Report {
                name: task.name.clone(),
                status: Status::Cancelled,
                took,
                note: String::new(),
            },
            false,
        )
    };
    let _ = std::fs::remove_file(&ctx.log);
    for mut wait in waits {
        tokio::select! {
            _ = wait.wait_for(|finished| *finished) => {}
            () = ctx.cancel.cancelled() => return cancelled(Duration::ZERO),
        }
    }
    let _permit = match semaphore {
        Some(semaphore) => tokio::select! {
            permit = semaphore.acquire_owned() => permit.ok(),
            () = ctx.cancel.cancelled() => return cancelled(Duration::ZERO),
        },
        None => None,
    };
    if ctx.cancel.is_cancelled() {
        return cancelled(Duration::ZERO);
    }
    sink.line(&format!("start  {}", task.name));
    let began = Instant::now();
    let outcome = (task.action)(ctx.clone()).await;
    let took = began.elapsed();
    let log_hint = format!("{} ({})", task.name, rel(&ctx.root, &ctx.log));
    let (status, note) = match outcome {
        Outcome::Ok => {
            sink.line(&format!("OK     {} {}", task.name, fmt_secs(took)));
            (Status::Ok, String::new())
        }
        Outcome::Skip(reason) => {
            sink.line(&format!("SKIP   {}: {reason}", task.name));
            (Status::Skip, reason)
        }
        Outcome::Fail(reason) => {
            ctx.note(&format!("xtask: {reason}"));
            sink.line(&format!("FAIL   {log_hint} {}: {reason}", fmt_secs(took)));
            sink.raw(&tail(&ctx.log, 40));
            (Status::Fail, reason)
        }
        Outcome::Cancelled => {
            sink.line(&format!("CANCELLED {} {}", task.name, fmt_secs(took)));
            (Status::Cancelled, String::new())
        }
    };
    let failed = status == Status::Fail;
    if failed {
        // At once, not when the collector gets round to it: no task may start in between.
        ctx.cancel.cancel();
    }
    (
        Report {
            name: task.name,
            status,
            took,
            note,
        },
        failed,
    )
}

mod report;
pub use report::{rel, select, summary, tail};

#[cfg(all(test, unix))]
mod tests;
