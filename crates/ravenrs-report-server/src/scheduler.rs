//! Runs jobs: what is due on the schedule, or what is asked for by hand, with
//! retries, a history of who received what, and retention of the files.

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use ravenrs_report::Report;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::cron::Schedule;
use crate::deliver::{Mail, Mailer, write_to_folder};
use crate::job::{Delivery, Job};
use crate::render::{Rendered, local_date_time, parameter_values, render, resolve_data};
use crate::store::{Delivered, JobState, RunRecord, RunStatus, Store};

/// Keeps a job from running twice at once.
struct RunningGuard {
    running: Arc<Mutex<HashSet<String>>>,
    id: String,
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(&self.id);
        }
    }
}

/// A delivery still to be made.
#[derive(Clone, Debug)]
enum Pending {
    Email { to: Vec<String>, subject: Option<String>, body: Option<String> },
    Folder { path: String },
}

/// What a run has done so far, kept across its attempts: a retry renders once
/// and does not send again to whoever already received the files.
struct Progress {
    rendered: Option<Rendered>,
    layout_version: Option<u32>,
    pending: Vec<Pending>,
    delivered: Vec<Delivered>,
    errors: Vec<String>,
    skipped_empty: bool,
    /// The attempt died midway, so what was sent is unknown and retrying could send twice.
    stopped: bool,
}

fn pending_deliveries(job: &Job) -> Vec<Pending> {
    let mut pending = Vec::new();
    for delivery in &job.deliveries {
        match delivery {
            Delivery::Email { to, separately: true, subject, body } => pending.extend(to.iter().map(|one| {
                Pending::Email { to: vec![one.clone()], subject: subject.clone(), body: body.clone() }
            })),
            Delivery::Email { to, subject, body, .. } => {
                pending.push(Pending::Email { to: to.clone(), subject: subject.clone(), body: body.clone() });
            }
            Delivery::Folder { path } => pending.push(Pending::Folder { path: path.clone() }),
        }
    }
    pending
}

fn fill(template: &str, report: &str, date: &str) -> String {
    template.replace("{report}", report).replace("{date}", date)
}

/// Runs jobs. Clones share the same limits.
#[derive(Clone)]
pub struct Runner {
    store: Store,
    config: Arc<Config>,
    mailer: Option<Arc<dyn Mailer>>,
    permits: Arc<Semaphore>,
    running: Arc<Mutex<HashSet<String>>>,
}

impl fmt::Debug for Runner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Runner").field("store", &self.store).finish_non_exhaustive()
    }
}

impl Runner {
    /// A runner over `store`. Email deliveries fail without a `mailer`.
    pub fn new(store: Store, config: Arc<Config>, mailer: Option<Arc<dyn Mailer>>) -> Self {
        let permits = Arc::new(Semaphore::new(config.max_concurrent_renders.max(1)));
        Self { store, config, mailer, permits, running: Arc::new(Mutex::new(HashSet::new())) }
    }

    /// The limit on simultaneous renders, shared with on-demand requests.
    pub fn permits(&self) -> Arc<Semaphore> {
        self.permits.clone()
    }

    /// Checks the schedule forever.
    pub async fn run_forever(&self, period: Duration) {
        let mut interval = tokio::time::interval(period);
        loop {
            interval.tick().await;
            self.tick(Timestamp::now());
        }
    }

    /// Starts whatever is due at `now`, and returns the runs it started.
    ///
    /// The next run is saved before a run starts, so a crash cannot repeat one.
    pub fn tick(&self, now: Timestamp) -> Vec<JoinHandle<RunRecord>> {
        let ids = match self.store.job_ids() {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!(error = %e, "could not list jobs");
                return Vec::new();
            }
        };
        let mut started = Vec::new();
        for id in ids {
            match self.check_job(&id, now) {
                Ok(Some(handle)) => started.push(handle),
                Ok(None) => {}
                Err(e) => tracing::error!(job = %id, error = %e, "could not check the job"),
            }
        }
        started
    }

    fn check_job(&self, id: &str, now: Timestamp) -> Result<Option<JoinHandle<RunRecord>>, String> {
        let Some(job) = self.store.job(id).map_err(|e| e.to_string())? else { return Ok(None) };
        if !job.enabled {
            return Ok(None);
        }
        let schedule = Schedule::parse(&job.schedule)?;
        let zone = TimeZone::get(&job.timezone).map_err(|e| e.to_string())?;
        let state = self.store.job_state(id).map_err(|e| e.to_string())?;
        let next = schedule.next_after(&now.to_zoned(zone)).map(|z| z.timestamp());
        let Some(due) = state.next_run else {
            self.store.save_job_state(id, &JobState { next_run: next }).map_err(|e| e.to_string())?;
            return Ok(None);
        };
        if due > now {
            return Ok(None);
        }
        self.store.save_job_state(id, &JobState { next_run: next }).map_err(|e| e.to_string())?;
        let grace = SignedDuration::from_secs(self.config.misfire_grace_minutes.saturating_mul(60) as i64);
        if now.duration_since(due) > grace {
            tracing::warn!(job = %id, due = %due, "the run is too late; recording it as missed");
            self.record_missed(id, due, now, "the server was not running at the scheduled time");
            return Ok(None);
        }
        let runner = self.clone();
        let id = id.to_owned();
        Ok(Some(tokio::spawn(async move { runner.run(&id, &job, due, false).await })))
    }

    fn record_missed(&self, id: &str, due: Timestamp, now: Timestamp, why: &str) -> RunRecord {
        let record = RunRecord {
            id: self.store.new_run_id(now),
            job: id.to_owned(),
            layout_version: None,
            scheduled_for: due,
            started_at: now,
            finished_at: now,
            manual: false,
            status: RunStatus::Missed,
            attempts: 0,
            rows: None,
            delivered: Vec::new(),
            errors: vec![why.to_owned()],
            report_warnings: Vec::new(),
            documents: Vec::new(),
        };
        if let Err(e) = self.store.save_run(&record, self.config.history_limit) {
            tracing::error!(job = %id, error = %e, "could not save the run");
        }
        record
    }

    /// Runs a job now, whatever its schedule, and returns what happened.
    pub async fn run_now(&self, id: &str, job: &Job) -> RunRecord {
        self.run(id, job, Timestamp::now(), true).await
    }

    async fn run(&self, id: &str, job: &Job, scheduled_for: Timestamp, manual: bool) -> RunRecord {
        let started_at = Timestamp::now();
        let free = self.running.lock().is_ok_and(|mut running| running.insert(id.to_owned()));
        if !free {
            return self.record_missed(id, scheduled_for, started_at, "the previous run was still going");
        }
        let guard = RunningGuard { running: self.running.clone(), id: id.to_owned() };
        let mut progress = Progress {
            rendered: None,
            layout_version: None,
            pending: pending_deliveries(job),
            delivered: Vec::new(),
            errors: Vec::new(),
            skipped_empty: false,
            stopped: false,
        };
        let mut attempts = 0;
        loop {
            attempts += 1;
            progress = self.attempt(job, scheduled_for, progress).await;
            let done = progress.skipped_empty || (progress.pending.is_empty() && progress.errors.is_empty());
            if done || progress.stopped || attempts > job.retries {
                break;
            }
            let wait = self.config.retry_delay_seconds.saturating_mul(1 << (attempts - 1).min(16));
            tracing::warn!(job = %id, attempt = attempts, errors = ?progress.errors, wait, "the run failed; trying again");
            tokio::time::sleep(Duration::from_secs(wait)).await;
        }
        drop(guard);
        let record = self.finish(id, job, scheduled_for, started_at, manual, attempts, progress);
        tracing::info!(job = %id, run = %record.id, status = ?record.status, "run finished");
        record
    }

    async fn attempt(&self, job: &Job, scheduled_for: Timestamp, progress: Progress) -> Progress {
        let permit = self.permits.clone().acquire_owned().await;
        let runner = self.clone();
        let task_job = job.clone();
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            runner.attempt_blocking(&task_job, scheduled_for, progress)
        });
        match task.await {
            Ok(progress) => progress,
            // A panic: this attempt's state is gone with it.
            Err(e) => Progress {
                rendered: None,
                layout_version: None,
                pending: Vec::new(),
                delivered: Vec::new(),
                errors: vec![format!("the run stopped unexpectedly: {e}")],
                skipped_empty: false,
                stopped: true,
            },
        }
    }

    fn attempt_blocking(&self, job: &Job, scheduled_for: Timestamp, mut progress: Progress) -> Progress {
        progress.errors.clear();
        let Ok(zone) = TimeZone::get(&job.timezone) else {
            progress.errors.push(format!("unknown time zone '{}'", job.timezone));
            return progress;
        };
        let local = scheduled_for.to_zoned(zone);
        let date = local.strftime("%Y-%m-%d").to_string();
        if progress.rendered.is_none() {
            match self.render_job(job, &local, &date) {
                Ok((version, rendered)) => {
                    progress.layout_version = Some(version);
                    progress.rendered = Some(rendered);
                }
                Err(e) => {
                    progress.errors.push(e);
                    return progress;
                }
            }
        }
        let Some(rendered) = progress.rendered.take() else { return progress };
        if job.skip_if_empty && rendered.rows == 0 {
            progress.skipped_empty = true;
            progress.pending.clear();
            progress.rendered = Some(rendered);
            return progress;
        }
        let mut remaining = Vec::new();
        for pending in std::mem::take(&mut progress.pending) {
            match self.deliver(&pending, &rendered, &date) {
                Ok(delivered) => progress.delivered.push(delivered),
                Err(e) => {
                    progress.errors.push(e);
                    remaining.push(pending);
                }
            }
        }
        progress.pending = remaining;
        progress.rendered = Some(rendered);
        progress
    }

    fn render_job(&self, job: &Job, local: &jiff::Zoned, date: &str) -> Result<(u32, Rendered), String> {
        let (version, text) = self
            .store
            .layout(&job.layout, None)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("the layout '{}' does not exist", job.layout))?;
        let report = Report::from_json(&text).map_err(|e| format!("layout '{}': {e}", job.layout))?;
        let now = local_date_time(local).ok_or("the date is out of range")?;
        let parameters = parameter_values(&job.parameters, now)?;
        let data = resolve_data(&report, &parameters, &self.config)?;
        Ok((version, render(&report, &data, parameters, now, &job.formats, date)?))
    }

    fn deliver(&self, pending: &Pending, rendered: &Rendered, date: &str) -> Result<Delivered, String> {
        let files: Vec<String> = rendered.files.iter().map(|f| f.name.clone()).collect();
        let at = Timestamp::now();
        match pending {
            Pending::Email { to, subject, body } => {
                let mailer = self.mailer.as_ref().ok_or("the server has no mail server configured")?;
                let target = to.join(", ");
                let subject = fill(subject.as_deref().unwrap_or("{report} - {date}"), &rendered.title, date);
                let body = fill(
                    body.as_deref().unwrap_or("{report}, {date}. The report is attached."),
                    &rendered.title,
                    date,
                );
                mailer
                    .send(&Mail { to: to.clone(), subject, body, attachments: rendered.files.clone() })
                    .map_err(|e| format!("email to {target}: {e}"))?;
                Ok(Delivered { kind: "email".to_owned(), target, files, at })
            }
            Pending::Folder { path } => {
                let base = self.config.output_dir.as_ref().ok_or("the server has no outputDir configured")?;
                write_to_folder(base, path, &rendered.files)?;
                Ok(Delivered { kind: "folder".to_owned(), target: path.clone(), files, at })
            }
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one private step of a run")]
    fn finish(
        &self,
        id: &str,
        job: &Job,
        scheduled_for: Timestamp,
        started_at: Timestamp,
        manual: bool,
        attempts: u32,
        progress: Progress,
    ) -> RunRecord {
        let now = Timestamp::now();
        let run_id = self.store.new_run_id(started_at);
        let status = if progress.skipped_empty {
            RunStatus::SkippedEmpty
        } else if progress.errors.is_empty() && progress.pending.is_empty() {
            RunStatus::Success
        } else if progress.delivered.is_empty() {
            RunStatus::Failed
        } else {
            RunStatus::Partial
        };
        let mut documents = Vec::new();
        if job.retention_days > 0
            && !progress.skipped_empty
            && let Some(rendered) = &progress.rendered
        {
            for file in &rendered.files {
                match self.store.save_document(id, &run_id, &file.name, &file.bytes) {
                    Ok(()) => documents.push(file.name.clone()),
                    Err(e) => {
                        tracing::error!(job = %id, file = %file.name, error = %e, "could not keep the file")
                    }
                }
            }
        }
        let record = RunRecord {
            id: run_id,
            job: id.to_owned(),
            layout_version: progress.layout_version,
            scheduled_for,
            started_at,
            finished_at: now,
            manual,
            status,
            attempts,
            rows: progress.rendered.as_ref().map(|r| r.rows),
            delivered: progress.delivered,
            errors: progress.errors,
            report_warnings: progress.rendered.map(|r| r.warnings).unwrap_or_default(),
            documents,
        };
        if let Err(e) = self.store.save_run(&record, self.config.history_limit) {
            tracing::error!(job = %id, error = %e, "could not save the run");
        }
        let keep = SignedDuration::from_hours(i64::from(job.retention_days).saturating_mul(24));
        if let Ok(cutoff) = now.checked_sub(keep)
            && let Err(e) = self.store.purge_documents(id, cutoff)
        {
            tracing::error!(job = %id, error = %e, "could not delete old files");
        }
        record
    }
}
