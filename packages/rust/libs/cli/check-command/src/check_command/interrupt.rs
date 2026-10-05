use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

static USER_INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
pub struct InterruptFlag {
    local: Arc<AtomicBool>,
    include_user_signal: bool,
}

impl InterruptFlag {
    pub fn install() -> Self {
        USER_INTERRUPTED.store(false, Ordering::SeqCst);
        #[cfg(unix)]
        unsafe {
            let _ = signal(2, Some(handle_sigint));
        }
        Self {
            local: Arc::new(AtomicBool::new(false)),
            include_user_signal: true,
        }
    }

    #[cfg(test)]
    pub(super) fn for_test() -> Self {
        Self {
            local: Arc::new(AtomicBool::new(false)),
            include_user_signal: false,
        }
    }

    #[cfg(test)]
    pub(super) fn interrupt_for_test(&self) {
        self.local.store(true, Ordering::SeqCst);
    }

    pub fn is_set(&self) -> bool {
        self.local.load(Ordering::SeqCst)
            || (self.include_user_signal && USER_INTERRUPTED.load(Ordering::SeqCst))
    }
}

#[cfg(unix)]
type SignalHandler = Option<extern "C" fn(i32)>;

#[cfg(unix)]
unsafe extern "C" {
    fn signal(signal: i32, handler: SignalHandler) -> SignalHandler;
}

#[cfg(unix)]
extern "C" fn handle_sigint(_signal: i32) {
    USER_INTERRUPTED.store(true, Ordering::SeqCst);
}

#[derive(Clone)]
pub struct CancellationToken {
    requested: Arc<AtomicBool>,
    interrupt: InterruptFlag,
}

impl CancellationToken {
    pub fn new(interrupt: InterruptFlag) -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            interrupt,
        }
    }

    pub fn is_canceled(&self) -> bool {
        self.requested.load(Ordering::SeqCst) || self.interrupt.is_set()
    }

    pub fn cancel(&self) {
        self.requested.store(true, Ordering::SeqCst);
    }

    pub fn interrupted(&self) -> bool {
        self.interrupt.is_set()
    }
}

#[cfg(test)]
mod tests {
    use super::{CancellationToken, InterruptFlag};

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-RUN-012"))]
    #[cfg_attr(any(), bloomery("CLI-CHECK-RUN-013"))]
    fn user_interruption_is_distinct_from_task_cancellation() {
        let interrupt = InterruptFlag::for_test();
        let token = CancellationToken::new(interrupt.clone());
        assert!(!token.is_canceled());
        interrupt.interrupt_for_test();
        assert!(token.is_canceled());
        assert!(token.interrupted());
    }
}
