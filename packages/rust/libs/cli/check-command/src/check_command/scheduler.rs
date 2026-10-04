use super::interrupt::{CancellationToken, InterruptFlag};
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub enum ScheduledState<T> {
    Completed(T),
    Canceled,
    NotRun,
}

pub struct ScheduledResult<T> {
    pub id: String,
    pub state: ScheduledState<T>,
}

pub struct ScheduleReport<T> {
    pub results: Vec<ScheduledResult<T>>,
}

/// Tracks worker completion so a caller-thread progress consumer knows when no
/// further events can be produced. Workers hold a guard that decrements the
/// count as their last action.
pub(super) struct WorkerTracker {
    state: Arc<(Mutex<usize>, Condvar)>,
}

impl WorkerTracker {
    fn new(count: usize) -> Self {
        Self {
            state: Arc::new((Mutex::new(count), Condvar::new())),
        }
    }

    fn guard(&self) -> WorkerGuard {
        WorkerGuard {
            state: self.state.clone(),
        }
    }

    /// Wait up to `timeout` for all workers to finish. Returns `true` once idle.
    pub(super) fn wait_timeout(&self, timeout: Duration) -> bool {
        let (lock, condvar) = &*self.state;
        let guard = lock.lock().expect("worker tracker lock");
        if *guard == 0 {
            return true;
        }
        let (guard, _timeout_result) = condvar
            .wait_timeout(guard, timeout)
            .expect("worker tracker wait");
        *guard == 0
    }
}

struct WorkerGuard {
    state: Arc<(Mutex<usize>, Condvar)>,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        let (lock, condvar) = &*self.state;
        let mut count = lock.lock().expect("worker tracker lock");
        *count = count.saturating_sub(1);
        condvar.notify_all();
    }
}

pub(super) type Task<'scope, T> = Box<dyn FnOnce(CancellationToken) -> T + Send + 'scope>;

struct Scheduler<'scope, T> {
    ids: Vec<String>,
    queue: VecDeque<(usize, Task<'scope, T>)>,
    slots: Vec<Slot<T>>,
    admission_stopped: bool,
}
enum Slot<T> {
    Queued,
    Running,
    Completed(T),
}

impl<T> Scheduler<'_, T> {
    /// Permanently stop admission and return the IDs whose queued work can now
    /// never run. Called once per scheduling run under the state lock.
    fn stop_admission(&mut self) -> Vec<String> {
        if self.admission_stopped {
            return Vec::new();
        }
        self.admission_stopped = true;
        let queued = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| matches!(slot, Slot::Queued))
            .map(|(index, _)| self.ids[index].clone())
            .collect();
        self.queue.clear();
        queued
    }
}

#[cfg(test)]
pub fn run_bounded<'scope, T, F, IsFailure>(
    tasks: Vec<(String, F)>,
    jobs: usize,
    fail_fast: bool,
    interrupt: InterruptFlag,
    is_failure: IsFailure,
) -> ScheduleReport<T>
where
    T: Send + 'scope,
    F: FnOnce(CancellationToken) -> T + Send + 'scope,
    IsFailure: Fn(&T) -> bool + Sync + 'scope,
{
    run_with_followups(
        tasks
            .into_iter()
            .map(|(id, task)| (id, Box::new(task) as Task<'scope, T>))
            .collect(),
        jobs,
        interrupt,
        |result| fail_fast && is_failure(result),
        |_| Vec::new(),
        |_id, _result| {},
        |_queued| {},
        |_tracker| {},
    )
}

/// Admit newly ready dependencies ahead of queued independent work. Admission
/// and failure-triggered cancellation share a lock, so no task is admitted
/// after the scheduler observes a stopping failure.
///
/// `on_complete` runs once per completed task and is where live outcome events
/// are published. `on_stopped` runs once with the queued task IDs when failure
/// or interruption permanently stops admission, so callers can publish their
/// final not_run outcomes without waiting for active workers to join.
/// `on_spawned` runs on the calling thread after every worker is started; it is
/// the single-owner drain point for progress events.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_with_followups<'scope, T, Stop, Followups, OnComplete, OnStopped, OnSpawned>(
    tasks: Vec<(String, Task<'scope, T>)>,
    jobs: usize,
    interrupt: InterruptFlag,
    should_stop: Stop,
    followups: Followups,
    on_complete: OnComplete,
    on_stopped: OnStopped,
    on_spawned: OnSpawned,
) -> ScheduleReport<T>
where
    T: Send + 'scope,
    Stop: Fn(&T) -> bool + Sync + 'scope,
    Followups: Fn(&mut T) -> Vec<(String, Task<'scope, T>)> + Sync + 'scope,
    OnComplete: Fn(&str, &T) + Sync + 'scope,
    OnStopped: Fn(&[String]) + Sync + 'scope,
    OnSpawned: FnOnce(&WorkerTracker),
{
    let count = tasks.len();
    if count == 0 {
        return ScheduleReport {
            results: Vec::new(),
        };
    }

    let mut ids = Vec::with_capacity(count);
    let mut queue: VecDeque<(usize, Task<'scope, T>)> = VecDeque::with_capacity(count);
    for (index, (id, task)) in tasks.into_iter().enumerate() {
        ids.push(id);
        queue.push_back((index, task));
    }
    let scheduler = Mutex::new(Scheduler {
        ids,
        queue,
        slots: (0..count).map(|_| Slot::Queued).collect(),
        admission_stopped: false,
    });
    let token = CancellationToken::new(interrupt);
    let worker_count = jobs.max(1).min(count);
    let tracker = WorkerTracker::new(worker_count);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let token = token.clone();
            let scheduler = &scheduler;
            let should_stop = &should_stop;
            let followups = &followups;
            let on_complete = &on_complete;
            let on_stopped = &on_stopped;
            let guard = tracker.guard();
            scope.spawn(move || {
                let _guard = guard;
                loop {
                    if token.interrupted() {
                        token.cancel();
                    }
                    if token.is_canceled() {
                        let queued = {
                            let mut scheduler = scheduler.lock().expect("scheduler state lock");
                            scheduler.stop_admission()
                        };
                        if !queued.is_empty() {
                            on_stopped(&queued);
                        }
                        break;
                    }

                    let next = {
                        let mut scheduler = scheduler.lock().expect("scheduler state lock");
                        if token.is_canceled() {
                            let queued = scheduler.stop_admission();
                            drop(scheduler);
                            if !queued.is_empty() {
                                on_stopped(&queued);
                            }
                            break;
                        }
                        let next = scheduler.queue.pop_front();
                        if let Some((index, _)) = &next {
                            scheduler.slots[*index] = Slot::Running;
                        }
                        next
                    };
                    let Some((index, work)) = next else {
                        break;
                    };

                    let mut result = work(token.clone());
                    let mut scheduler = scheduler.lock().expect("scheduler state lock");
                    if should_stop(&result) {
                        token.cancel();
                    }
                    if !token.is_canceled() {
                        for (id, task) in followups(&mut result).into_iter().rev() {
                            let index = scheduler.slots.len();
                            scheduler.ids.push(id);
                            scheduler.slots.push(Slot::Queued);
                            scheduler.queue.push_front((index, task));
                        }
                    }
                    let completed_id = scheduler.ids[index].clone();
                    on_complete(&completed_id, &result);
                    scheduler.slots[index] = Slot::Completed(result);
                }
            });
        }
        on_spawned(&tracker);
    });

    let scheduler = scheduler.into_inner().expect("scheduler state lock");
    let results = scheduler
        .ids
        .into_iter()
        .zip(scheduler.slots)
        .map(|(id, slot)| {
            let state = match slot {
                Slot::Completed(result) => ScheduledState::Completed(result),
                Slot::Running if token.is_canceled() => ScheduledState::Canceled,
                Slot::Running | Slot::Queued => ScheduledState::NotRun,
            };
            ScheduledResult { id, state }
        })
        .collect();

    ScheduleReport { results }
}

#[cfg(test)]
mod tests {
    use super::{ScheduledState, Task, run_bounded, run_with_followups};
    use crate::check_command::interrupt::{CancellationToken, InterruptFlag};
    use bloomery_test_macros::bloomery;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::time::Duration;

    #[derive(Debug, PartialEq, Eq)]
    enum TaskResult {
        Passed,
        Failed,
        Canceled,
    }

    type TestTask = Box<dyn FnOnce(CancellationToken) -> TaskResult + Send>;

    #[test]
    #[bloomery("CLI-CHECK-RUN-018")]
    fn ready_followups_cancel_running_work_and_leave_queued_work_unstarted() {
        let barrier = Arc::new(Barrier::new(2));
        let structure_barrier = barrier.clone();
        let nix_barrier = barrier.clone();
        let tasks: Vec<(String, Task<'_, TaskResult>)> = vec![
            (
                "structure".to_owned(),
                Box::new(move |_| {
                    structure_barrier.wait();
                    TaskResult::Passed
                }),
            ),
            (
                "running-nix".to_owned(),
                Box::new(move |cancel| {
                    nix_barrier.wait();
                    let deadline = std::time::Instant::now() + Duration::from_secs(3);
                    while !cancel.is_canceled() && std::time::Instant::now() < deadline {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    assert!(cancel.is_canceled(), "followup did not cancel running work");
                    TaskResult::Canceled
                }),
            ),
            (
                "queued-nix".to_owned(),
                Box::new(|_| panic!("queued work admitted before followup")),
            ),
        ];
        let queued_not_run = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let observed_queued = queued_not_run.clone();
        let report = run_with_followups(
            tasks,
            2,
            InterruptFlag::for_test(),
            |result| *result == TaskResult::Failed,
            |result| {
                if *result == TaskResult::Passed {
                    let task: Task<'_, TaskResult> = Box::new(|_| TaskResult::Failed);
                    vec![("traceability".to_owned(), task)]
                } else {
                    Vec::new()
                }
            },
            |_id, _result| {},
            |queued| {
                observed_queued
                    .lock()
                    .expect("queued outcome lock")
                    .extend_from_slice(queued);
            },
            |_tracker| {},
        );
        assert_eq!(
            queued_not_run
                .lock()
                .expect("queued outcome lock")
                .as_slice(),
            &["queued-nix".to_owned()]
        );
        assert!(matches!(
            report.results[1].state,
            ScheduledState::Completed(TaskResult::Canceled)
        ));
        assert!(matches!(report.results[2].state, ScheduledState::NotRun));
        assert_eq!(report.results[3].id, "traceability");
        assert!(matches!(
            report.results[3].state,
            ScheduledState::Completed(TaskResult::Failed)
        ));
    }

    #[test]
    #[bloomery("CLI-CHECK-RUN-001")]
    #[bloomery("CLI-CHECK-RUN-002")]
    #[bloomery("CLI-CHECK-RUN-003")]
    fn bounded_workers_run_independent_tasks_concurrently() {
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(2));
        let mut tasks = Vec::new();
        for id in ["static", "nix"] {
            let active = active.clone();
            let maximum = maximum.clone();
            let barrier = barrier.clone();
            tasks.push((id.to_owned(), move |_cancel: CancellationToken| {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(now, Ordering::SeqCst);
                barrier.wait();
                std::thread::sleep(Duration::from_millis(10));
                active.fetch_sub(1, Ordering::SeqCst);
                false
            }));
        }
        let report = run_bounded(tasks, 2, false, InterruptFlag::for_test(), |failed| *failed);
        assert_eq!(maximum.load(Ordering::SeqCst), 2);
        assert!(
            report
                .results
                .iter()
                .all(|item| matches!(item.state, ScheduledState::Completed(false)))
        );
    }

    #[test]
    #[bloomery("CLI-CHECK-RUN-003")]
    fn the_task_limit_bounds_active_work() {
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let tasks = (0..12)
            .map(|index| {
                let active = active.clone();
                let maximum = maximum.clone();
                (
                    format!("task-{index}"),
                    move |_cancel: CancellationToken| {
                        let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                        maximum.fetch_max(running, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(5));
                        active.fetch_sub(1, Ordering::SeqCst);
                        false
                    },
                )
            })
            .collect();
        let report = run_bounded(tasks, 3, false, InterruptFlag::for_test(), |failed| *failed);
        assert!(
            report
                .results
                .iter()
                .all(|result| matches!(result.state, ScheduledState::Completed(false)))
        );
        assert!(maximum.load(Ordering::SeqCst) <= 3);
        assert!(maximum.load(Ordering::SeqCst) > 1);
    }

    #[test]
    #[bloomery("CLI-CHECK-RUN-005")]
    #[bloomery("CLI-CHECK-RUN-007")]
    #[bloomery("CLI-CHECK-RUN-010")]
    fn fail_fast_stops_admission_and_marks_queued_tasks_not_run() {
        let admitted = Arc::new(AtomicUsize::new(0));
        let tasks = ["first", "queued-1", "queued-2"]
            .into_iter()
            .map(|id| {
                let admitted = admitted.clone();
                (id.to_owned(), move |_cancel: CancellationToken| {
                    admitted.fetch_add(1, Ordering::SeqCst);
                    id == "first"
                })
            })
            .collect();
        let report = run_bounded(tasks, 1, true, InterruptFlag::for_test(), |failed| *failed);

        assert_eq!(admitted.load(Ordering::SeqCst), 1);
        assert!(matches!(
            report.results[0].state,
            ScheduledState::Completed(true)
        ));
        assert!(matches!(report.results[1].state, ScheduledState::NotRun));
        assert!(matches!(report.results[2].state, ScheduledState::NotRun));
    }

    #[test]
    #[bloomery("CLI-CHECK-RUN-008")]
    fn fail_fast_cancels_only_its_running_task_work() {
        let barrier = Arc::new(Barrier::new(2));
        let first_barrier = barrier.clone();
        let running_barrier = barrier.clone();
        let tasks: Vec<(String, TestTask)> = vec![
            (
                "failure".to_owned(),
                Box::new(move |_cancel: CancellationToken| {
                    first_barrier.wait();
                    TaskResult::Failed
                }),
            ),
            (
                "running".to_owned(),
                Box::new(move |cancel: CancellationToken| {
                    running_barrier.wait();
                    while !cancel.is_canceled() {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    TaskResult::Canceled
                }),
            ),
        ];
        let report = run_bounded(tasks, 2, true, InterruptFlag::for_test(), |result| {
            *result == TaskResult::Failed
        });

        assert!(matches!(
            report.results[0].state,
            ScheduledState::Completed(TaskResult::Failed)
        ));
        assert!(matches!(
            report.results[1].state,
            ScheduledState::Completed(TaskResult::Canceled)
        ));
    }

    #[test]
    #[bloomery("CLI-CHECK-RUN-012")]
    #[bloomery("CLI-CHECK-RUN-013")]
    fn interruption_requests_cancellation_and_preserves_finished_work() {
        let interrupt = InterruptFlag::for_test();
        let for_task = interrupt.clone();
        let task_interrupt = interrupt.clone();
        let tasks: Vec<(String, TestTask)> = vec![
            (
                "finished".to_owned(),
                Box::new(|_cancel: CancellationToken| TaskResult::Passed),
            ),
            (
                "waiting".to_owned(),
                Box::new(move |cancel: CancellationToken| {
                    for_task.interrupt_for_test();
                    while !cancel.is_canceled() {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    TaskResult::Canceled
                }),
            ),
        ];
        let report = run_bounded(tasks, 1, false, task_interrupt, |result| {
            *result == TaskResult::Failed
        });

        assert!(interrupt.is_set());
        assert!(matches!(
            report.results[0].state,
            ScheduledState::Completed(TaskResult::Passed)
        ));
        assert!(matches!(
            report.results[1].state,
            ScheduledState::Completed(TaskResult::Canceled)
        ));
    }
}
