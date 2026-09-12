use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::ThreadId;

/// Serializes a run while allowing an inline Agent to complete on its caller thread.
#[derive(Default)]
pub(super) struct AutomationExecutionGate {
    runs: Mutex<BTreeMap<String, Weak<RunGate>>>,
}
#[derive(Default)]
struct RunGate {
    owner: Mutex<(Option<ThreadId>, usize)>,
    wake: Condvar,
}
pub(super) struct RunGuard(Arc<RunGate>, std::marker::PhantomData<std::rc::Rc<()>>);
impl AutomationExecutionGate {
    pub(super) fn enter(&self, run: &str) -> RunGuard {
        let gate = {
            let mut runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
            runs.retain(|_, gate| gate.strong_count() > 0);
            if let Some(gate) = runs.get(run).and_then(Weak::upgrade) {
                gate
            } else {
                let gate = Arc::new(RunGate::default());
                runs.insert(run.to_owned(), Arc::downgrade(&gate));
                gate
            }
        };
        let thread = std::thread::current().id();
        let mut owner = gate.owner.lock().unwrap_or_else(|e| e.into_inner());
        while owner.0.is_some_and(|id| id != thread) {
            owner = gate.wake.wait(owner).unwrap_or_else(|e| e.into_inner());
        }
        owner.0 = Some(thread);
        owner.1 += 1;
        drop(owner);
        RunGuard(gate, Default::default())
    }
}
impl Drop for RunGuard {
    fn drop(&mut self) {
        let mut owner = self.0.owner.lock().unwrap_or_else(|e| e.into_inner());
        owner.1 -= 1;
        if owner.1 == 0 {
            owner.0 = None;
            self.0.wake.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_gate_allows_inline_completion_and_serializes_other_threads() {
        let gate = Arc::new(AutomationExecutionGate::default());
        let first = gate.enter("run");
        let nested = gate.enter("run");
        let (entered, observed) = std::sync::mpsc::channel();
        let other = gate.clone();
        let worker = std::thread::spawn(move || {
            let _independent = other.enter("unrelated-run");
            entered.send("independent").unwrap();
            let _same = other.enter("run");
            entered.send("same").unwrap();
        });
        assert_eq!(observed.recv().unwrap(), "independent");
        drop(nested);
        assert!(observed.try_recv().is_err());
        drop(first);
        assert_eq!(observed.recv().unwrap(), "same");
        worker.join().unwrap();
    }
}
