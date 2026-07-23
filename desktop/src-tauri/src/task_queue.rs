use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskKind {
    Smoke,
    Refresh,
    BatchScrape,
    Scrape,
    Rescrape,
    ManualMatch,
    Rename,
    Organize,
    Cleanup,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub completed: u32,
    pub total: u32,
    pub current: String,
    pub stage_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub id: String,
    pub title: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub progress: Option<TaskProgress>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

struct TaskRecord {
    snapshot: TaskSnapshot,
    cancel: Arc<AtomicBool>,
    work: Option<TaskWork>,
}

type TaskWork = Box<dyn FnOnce(TaskHandle) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>> + Send>;

#[derive(Clone)]
pub struct TaskHandle {
    pub id: String,
    cancel: Arc<AtomicBool>,
    queue: Arc<TaskQueueInner>,
}

impl TaskHandle {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub async fn update_progress(&self, progress: TaskProgress) {
        self.queue
            .update(&self.id, |snap| {
                snap.progress = Some(progress);
                snap.updated_at = Utc::now();
            })
            .await;
    }
}

struct TaskQueueInner {
    tasks: Mutex<Vec<TaskRecord>>,
    wake: Notify,
    processing: AtomicBool,
}

pub struct TaskQueue {
    inner: Arc<TaskQueueInner>,
}

impl TaskQueue {
    pub fn new() -> Self {
        let inner = Arc::new(TaskQueueInner {
            tasks: Mutex::new(Vec::new()),
            wake: Notify::new(),
            processing: AtomicBool::new(false),
        });
        let worker = Arc::clone(&inner);
        tauri::async_runtime::spawn(async move {
            worker_loop(worker).await;
        });
        Self { inner }
    }

    pub async fn list(&self) -> Vec<TaskSnapshot> {
        self.inner
            .tasks
            .lock()
            .await
            .iter()
            .map(|t| t.snapshot.clone())
            .collect()
    }

    pub async fn enqueue_smoke(&self, title: impl Into<String>) -> TaskSnapshot {
        let title = title.into();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let cancel = Arc::new(AtomicBool::new(false));
        let snapshot = TaskSnapshot {
            id: id.clone(),
            title: title.clone(),
            kind: TaskKind::Smoke,
            status: TaskStatus::Pending,
            progress: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };

        let work: TaskWork = Box::new(|handle| {
            Box::pin(async move {
                for step in 1..=5u32 {
                    if handle.is_cancelled() {
                        return Err("cancelled".into());
                    }
                    handle
                        .update_progress(TaskProgress {
                            completed: step,
                            total: 5,
                            current: format!("smoke step {step}/5"),
                            stage_key: Some("smoke".into()),
                        })
                        .await;
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                Ok(())
            })
        });

        {
            let mut tasks = self.inner.tasks.lock().await;
            tasks.push(TaskRecord {
                snapshot: snapshot.clone(),
                cancel,
                work: Some(work),
            });
        }
        self.inner.wake.notify_one();
        snapshot
    }

    pub async fn cancel(&self, id: &str) -> bool {
        let mut tasks = self.inner.tasks.lock().await;
        if let Some(task) = tasks.iter_mut().find(|t| t.snapshot.id == id) {
            task.cancel.store(true, Ordering::SeqCst);
            if task.snapshot.status == TaskStatus::Pending {
                task.snapshot.status = TaskStatus::Cancelled;
                task.snapshot.updated_at = Utc::now();
                task.work = None;
            }
            return true;
        }
        false
    }
}

impl TaskQueueInner {
    async fn update(&self, id: &str, f: impl FnOnce(&mut TaskSnapshot)) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.iter_mut().find(|t| t.snapshot.id == id) {
            f(&mut task.snapshot);
        }
    }
}

async fn worker_loop(inner: Arc<TaskQueueInner>) {
    loop {
        let next = {
            let mut tasks = inner.tasks.lock().await;
            tasks
                .iter_mut()
                .find(|t| t.snapshot.status == TaskStatus::Pending && t.work.is_some())
                .map(|t| {
                    t.snapshot.status = TaskStatus::Running;
                    t.snapshot.updated_at = Utc::now();
                    let work = t.work.take().expect("work present");
                    let handle = TaskHandle {
                        id: t.snapshot.id.clone(),
                        cancel: Arc::clone(&t.cancel),
                        queue: Arc::clone(&inner),
                    };
                    (work, handle)
                })
        };

        let Some((work, handle)) = next else {
            inner.wake.notified().await;
            continue;
        };

        if inner.processing.swap(true, Ordering::SeqCst) {
            // Should not happen with single worker; keep semantics explicit.
        }

        let result = work(handle.clone()).await;
        {
            let mut tasks = inner.tasks.lock().await;
            if let Some(task) = tasks.iter_mut().find(|t| t.snapshot.id == handle.id) {
                task.snapshot.updated_at = Utc::now();
                if handle.is_cancelled() {
                    task.snapshot.status = TaskStatus::Cancelled;
                } else if let Err(err) = result {
                    task.snapshot.status = TaskStatus::Failed;
                    task.snapshot.error_message = Some(err);
                } else {
                    task.snapshot.status = TaskStatus::Completed;
                }
            }
        }
        inner.processing.store(false, Ordering::SeqCst);
    }
}
