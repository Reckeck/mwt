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
use crate::sink::{Sink, TaskEnd};

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

/// How often a running task reports itself in CI.
const PROGRESS_EVERY: Duration = Duration::from_secs(60);

type Action = Box<dyn Fn(Ctx) -> Pin<Box<dyn Future<Output = Outcome> + Send>> + Send + Sync>;

pub struct Task {
    pub name: String,
    pub deps: Vec<String>,
    /// Soft ordering: waits for these when they are in the graph, never pulls them in.
    pub after: Vec<String>,
    pub class: Class,
    /// What it runs, for `--dry-run`; `None` for a built-in function.
    pub description: Option<String>,
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
            description: None,
            action: Box::new(move |ctx| Box::pin(action(ctx))),
        }
    }

    pub fn after(mut self, names: &[&str]) -> Self {
        self.after
            .extend(names.iter().map(|name| (*name).to_string()));
        self
    }

    pub fn cmd(name: &str, class: Class, deps: &[&str], cmd: Cmd) -> Self {
        let description = short(&cmd.display());
        let mut task = Self::func(name, class, deps, move |ctx| {
            let cmd = cmd.clone();
            async move { run_cmd(&ctx, &cmd).await }
        });
        task.description = Some(description);
        task
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
    /// A FAIL does not cancel the others; only the tasks that depend on it are not run.
    pub keep_going: bool,
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

/// Runs the graph. The first FAIL cancels everything running and not started yet, unless `Opts::keep_going`.
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
    let done: HashMap<String, watch::Sender<Option<Status>>> = tasks
        .iter()
        .map(|task| (task.name.clone(), watch::channel(None).0))
        .collect();
    let order: Vec<String> = tasks.iter().map(|task| task.name.clone()).collect();
    let mut set = JoinSet::new();
    for (index, task) in tasks.into_iter().enumerate() {
        let waits = Waits {
            deps: task
                .deps
                .iter()
                .filter_map(|dep| {
                    done.get(dep)
                        .map(|sender| (dep.clone(), sender.subscribe()))
                })
                .collect(),
            after: task
                .after
                .iter()
                .filter_map(|dep| done.get(dep))
                .map(watch::Sender::subscribe)
                .collect(),
        };
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
            let report = run_one(task, ctx, waits, semaphore, &sink, opts.keep_going).await;
            let _ = finished.send(Some(report.0.status));
            report
        });
    }
    let mut reports = HashMap::new();
    while let Some(joined) = set.join_next().await {
        if let Ok((report, failed)) = joined {
            if failed && !opts.keep_going {
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

/// In CI the task output stays in its log until the end, and a silent job looks hung:
/// every minute a line with the elapsed time and the last line of the log (`Sink::wants_progress`).
async fn with_progress(
    name: &str,
    ctx: &Ctx,
    action: impl Future<Output = Outcome>,
    sink: &Sink,
) -> Outcome {
    if !sink.wants_progress() {
        return action.await;
    }
    let began = Instant::now();
    let mut ticks =
        tokio::time::interval_at(tokio::time::Instant::now() + PROGRESS_EVERY, PROGRESS_EVERY);
    tokio::pin!(action);
    loop {
        tokio::select! {
            outcome = &mut action => return outcome,
            _ = ticks.tick() => sink.progress(name, began.elapsed(), &last_line(&ctx.log, 160)),
        }
    }
}

/// What a task waits for: `deps` also decide whether it runs, `after` only order it.
struct Waits {
    deps: Vec<(String, watch::Receiver<Option<Status>>)>,
    after: Vec<watch::Receiver<Option<Status>>>,
}

/// Waits until the task reports; `None` when its sender is gone.
async fn finished(wait: &mut watch::Receiver<Option<Status>>) -> Option<Status> {
    wait.wait_for(Option::is_some)
        .await
        .ok()
        .and_then(|status| *status)
}

async fn run_one(
    task: Task,
    ctx: Ctx,
    waits: Waits,
    semaphore: Option<Arc<Semaphore>>,
    sink: &Sink,
    keep_going: bool,
) -> (Report, bool) {
    let cancelled = |took, note: String| {
        (
            Report {
                name: task.name.clone(),
                status: Status::Cancelled,
                took,
                note,
            },
            false,
        )
    };
    let _ = std::fs::remove_file(&ctx.log);
    for mut wait in waits.after {
        tokio::select! {
            _ = finished(&mut wait) => {}
            () = ctx.cancel.cancelled() => return cancelled(Duration::ZERO, String::new()),
        }
    }
    for (dep, mut wait) in waits.deps {
        let status = tokio::select! {
            status = finished(&mut wait) => status,
            () = ctx.cancel.cancelled() => return cancelled(Duration::ZERO, String::new()),
        };
        if matches!(status, Some(Status::Fail | Status::Cancelled)) {
            let note = format!("dependency {dep} failed");
            sink.line(&format!("CANCELLED {} ({note})", task.name));
            return cancelled(Duration::ZERO, note);
        }
    }
    let _permit = match semaphore {
        Some(semaphore) => tokio::select! {
            permit = semaphore.acquire_owned() => permit.ok(),
            () = ctx.cancel.cancelled() => return cancelled(Duration::ZERO, String::new()),
        },
        None => None,
    };
    if ctx.cancel.is_cancelled() {
        return cancelled(Duration::ZERO, String::new());
    }
    sink.task_start(&task.name, &ctx.log);
    let began = Instant::now();
    let outcome = with_progress(&task.name, &ctx, (task.action)(ctx.clone()), sink).await;
    let took = began.elapsed();
    let log_hint = format!("{} ({})", task.name, rel(&ctx.root, &ctx.log));
    let (status, note) = match outcome {
        Outcome::Ok => (Status::Ok, String::new()),
        Outcome::Skip(reason) => (Status::Skip, reason),
        Outcome::Fail(reason) => {
            ctx.note(&format!("xtask: {reason}"));
            (Status::Fail, reason)
        }
        Outcome::Cancelled => (Status::Cancelled, String::new()),
    };
    sink.task_end(&TaskEnd {
        name: &task.name,
        status,
        took,
        reason: &note,
        root: &ctx.root,
        log: &ctx.log,
        log_hint: &log_hint,
    });
    let failed = status == Status::Fail;
    if failed && !keep_going {
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
pub use report::{last_line, list_text, plan_text, rel, select, summary, tail};

#[cfg(all(test, unix))]
mod tests;
