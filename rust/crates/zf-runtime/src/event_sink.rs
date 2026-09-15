//! Bounded observation transport. Durable events await capacity; replaceable tool
//! previews occupy one slot. A compiled node owns one terminal slot so dropping
//! its future can report completion without blocking or losing the event.
use serde_json::Value;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tokio::sync::{AcquireError, Notify, OwnedSemaphorePermit, Semaphore, TryAcquireError, mpsc};

struct Envelope {
    order: u64,
    event: Value,
}
struct PendingTerminal {
    envelope: Envelope,
    _permit: OwnedSemaphorePermit,
}
struct TerminalState {
    permits: Arc<Semaphore>,
    pending: Mutex<Option<PendingTerminal>>,
    rejected: Mutex<Option<Envelope>>,
}
struct Core {
    enqueue: Mutex<()>,
    next: AtomicU64,
    open: AtomicBool,
    preview: Mutex<Option<Envelope>>,
    terminals: Mutex<Vec<Arc<TerminalState>>>,
    changed: Notify,
}
#[derive(Clone)]
pub struct EventSink {
    sender: mpsc::Sender<Envelope>,
    core: Arc<Core>,
}
pub struct EventReceiver {
    receiver: mpsc::Receiver<Envelope>,
    core: Arc<Core>,
    buffer: Option<Envelope>,
}
#[derive(Clone)]
pub struct TerminalSlot {
    state: Arc<TerminalState>,
    core: Weak<Core>,
    sender: mpsc::WeakSender<Envelope>,
}
pub struct TerminalPermit {
    state: Arc<TerminalState>,
    core: Weak<Core>,
    permit: Option<OwnedSemaphorePermit>,
    sender: mpsc::WeakSender<Envelope>,
}

pub fn channel(capacity: usize) -> (EventSink, EventReceiver) {
    let (sender, receiver) = mpsc::channel(capacity.max(1));
    let core = Arc::new(Core {
        enqueue: Mutex::new(()),
        next: AtomicU64::new(0),
        open: AtomicBool::new(true),
        preview: Mutex::new(None),
        terminals: Mutex::new(vec![]),
        changed: Notify::new(),
    });
    (
        EventSink {
            sender,
            core: core.clone(),
        },
        EventReceiver {
            receiver,
            core,
            buffer: None,
        },
    )
}
impl EventSink {
    pub async fn send(&self, event: Value) -> Result<(), mpsc::error::SendError<Value>> {
        let permit = match self.sender.reserve().await {
            Ok(permit) => permit,
            Err(_) => return Err(mpsc::error::SendError(event)),
        };
        let _order = self.core.enqueue.lock().unwrap_or_else(|p| p.into_inner());
        let order = self.core.next.fetch_add(1, Ordering::Relaxed);
        permit.send(Envelope { order, event });
        Ok(())
    }
    /// Only current previews may be coalesced. Completed output is persisted in
    /// CAS and delivered by an awaited tool_result independently of this slot.
    pub fn emit(&self, event: Value) {
        if !self.core.open.load(Ordering::Acquire) {
            return;
        }
        let _order = self.core.enqueue.lock().unwrap_or_else(|p| p.into_inner());
        let order = self.core.next.fetch_add(1, Ordering::Relaxed);
        *self.core.preview.lock().unwrap_or_else(|p| p.into_inner()) =
            Some(Envelope { order, event });
        self.core.changed.notify_one();
    }
    pub fn same_channel(&self, other: &Self) -> bool {
        self.sender.same_channel(&other.sender)
    }
    /// Register once at graph construction, not once per invocation. The number
    /// of fallback slots is bounded by the number of compiled observed nodes.
    pub fn terminal_slot(&self) -> TerminalSlot {
        let state = Arc::new(TerminalState {
            permits: Arc::new(Semaphore::new(1)),
            pending: Mutex::new(None),
            rejected: Mutex::new(None),
        });
        if !self.core.open.load(Ordering::Acquire) {
            state.permits.close();
        }
        let mut slots = self
            .core
            .terminals
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        slots.retain(|slot| {
            Arc::strong_count(slot) > 1
                || slot
                    .pending
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_some()
                || slot
                    .rejected
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_some()
        });
        slots.push(state.clone());
        TerminalSlot {
            state,
            core: Arc::downgrade(&self.core),
            sender: self.sender.downgrade(),
        }
    }
}
impl TerminalSlot {
    pub async fn reserve(&self) -> Result<TerminalPermit, AcquireError> {
        let permit = self.state.permits.clone().acquire_owned().await?;
        Ok(TerminalPermit {
            state: self.state.clone(),
            core: self.core.clone(),
            permit: Some(permit),
            sender: self.sender.clone(),
        })
    }
    /// Admission must not wait inside an ADK timeout before an occurrence has
    /// a terminal destination. The fallback must be free: reserving ordinary
    /// queue capacity here could make running wait for its own terminal slot.
    pub fn try_reserve(&self) -> Result<TerminalPermit, TryAcquireError> {
        let permit = self.state.permits.clone().try_acquire_owned()?;
        Ok(TerminalPermit {
            state: self.state.clone(),
            core: self.core.clone(),
            permit: Some(permit),
            sender: self.sender.clone(),
        })
    }
    /// One bounded diagnostic per compiled node. Further rejected admissions
    /// are counted explicitly until it is consumed; admitted terminals remain
    /// untouched. Even retry-on-any therefore cannot fail silently.
    pub fn reject(&self, mut event: Value) {
        let Some(core) = self
            .core
            .upgrade()
            .filter(|core| core.open.load(Ordering::Acquire))
        else {
            return;
        };
        let _order = core.enqueue.lock().unwrap_or_else(|p| p.into_inner());
        let mut rejected = self
            .state
            .rejected
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(previous) = rejected.as_mut() {
            previous.event["rejectedAttempts"] = serde_json::json!(
                previous.event["rejectedAttempts"]
                    .as_u64()
                    .unwrap_or(1)
                    .saturating_add(1)
            );
            previous.event["lastRejectedOccurrenceId"] = event["occurrenceId"].clone();
            previous.event["endedAt"] = event["endedAt"].clone();
        } else {
            event["rejectedAttempts"] = serde_json::json!(1);
            *rejected = Some(Envelope {
                order: core.next.fetch_add(1, Ordering::Relaxed),
                event,
            });
        }
        core.changed.notify_one();
    }
}
impl TerminalPermit {
    /// Cancellation-safe and synchronous: release the permit after transfer to
    /// the bounded queue, or retain it until the fallback terminal is consumed.
    pub fn send(mut self, event: Value) {
        let Some(core) = self
            .core
            .upgrade()
            .filter(|core| core.open.load(Ordering::Acquire))
        else {
            return;
        };
        let _order = core.enqueue.lock().unwrap_or_else(|p| p.into_inner());
        let order = core.next.fetch_add(1, Ordering::Relaxed);
        // A slow consumer should not monopolize this node's fallback when the
        // normal bounded transport still has room for its completed attempt.
        if let Some(sender) = self.sender.upgrade()
            && let Ok(permit) = sender.try_reserve()
        {
            permit.send(Envelope { order, event });
            return;
        }
        let Some(permit) = self.permit.take() else {
            return;
        };
        *self.state.pending.lock().unwrap_or_else(|p| p.into_inner()) = Some(PendingTerminal {
            envelope: Envelope { order, event },
            _permit: permit,
        });
        core.changed.notify_one();
    }
}
impl Drop for TerminalPermit {
    fn drop(&mut self) {
        self.permit.take();
        if let Some(core) = self.core.upgrade() {
            core.changed.notify_one();
        }
    }
}
impl EventReceiver {
    pub fn try_recv(&mut self) -> Result<Value, mpsc::error::TryRecvError> {
        let _order = self.core.enqueue.lock().unwrap_or_else(|p| p.into_inner());
        if self.buffer.is_none() {
            self.buffer = self.receiver.try_recv().ok();
        }
        let slots: Vec<_> = self
            .core
            .terminals
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .cloned()
            .collect();
        let mut selected = self.buffer.as_ref().map(|event| (event.order, 0usize));
        for (index, slot) in slots.iter().enumerate() {
            let pending = slot.pending.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(pending) = pending.as_ref()
                && selected.is_none_or(|(order, _)| pending.envelope.order < order)
            {
                selected = Some((pending.envelope.order, index * 2 + 1));
            }
            let rejected = slot.rejected.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(rejected) = rejected.as_ref()
                && selected.is_none_or(|(order, _)| rejected.order < order)
            {
                selected = Some((rejected.order, index * 2 + 2));
            }
        }
        let preview_index = slots.len() * 2 + 1;
        let mut preview = self.core.preview.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(event) = preview.as_ref()
            && selected.is_none_or(|(order, _)| event.order < order)
        {
            selected = Some((event.order, preview_index));
        }
        if let Some((_, index)) = selected {
            let event = if index == 0 {
                self.buffer.take().map(|envelope| envelope.event)
            } else if index == preview_index {
                preview.take().map(|envelope| envelope.event)
            } else if index % 2 == 1 {
                slots[(index - 1) / 2]
                    .pending
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                    .map(|pending| pending.envelope.event)
            } else {
                slots[(index - 2) / 2]
                    .rejected
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                    .map(|envelope| envelope.event)
            };
            return event.ok_or(mpsc::error::TryRecvError::Empty);
        }
        let active = slots
            .iter()
            .any(|slot| slot.permits.available_permits() == 0);
        Err(if self.receiver.is_closed() && !active {
            mpsc::error::TryRecvError::Disconnected
        } else {
            mpsc::error::TryRecvError::Empty
        })
    }
    pub async fn recv(&mut self) -> Option<Value> {
        loop {
            let core = self.core.clone();
            let notified = core.changed.notified();
            tokio::pin!(notified);
            // Register before examining queues so a synchronous guard completion
            // cannot be lost between try_recv and awaiting the notification.
            notified.as_mut().enable();
            match self.try_recv() {
                Ok(event) => return Some(event),
                Err(mpsc::error::TryRecvError::Disconnected) => return None,
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
            if self.receiver.is_closed() {
                notified.await;
            } else {
                tokio::select! {
                    value=self.receiver.recv()=> {self.buffer=value;},
                    ()=&mut notified=>{},
                }
            }
        }
    }
}
impl Drop for EventReceiver {
    fn drop(&mut self) {
        self.core.open.store(false, Ordering::Release);
        self.receiver.close();
        for slot in self
            .core
            .terminals
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .cloned()
        {
            slot.permits.close();
            slot.pending
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take();
            slot.rejected
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    async fn durable_events_backpressure_but_drop_terminal_and_latest_preview_survive() {
        let (sink, mut receiver) = channel(1);
        let slot = sink.terminal_slot();
        let permit = slot.reserve().await.unwrap();
        sink.send(json!({"kind":"start"})).await.unwrap();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(10),
                sink.send(json!({"kind":"blocked"}))
            )
            .await
            .is_err()
        );
        for i in 0..1000 {
            sink.emit(json!({"kind":"preview","i":i}));
        }
        permit.send(json!({"kind":"terminal"}));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), slot.reserve())
                .await
                .is_err()
        );
        assert_eq!(receiver.recv().await.unwrap()["kind"], "start");
        assert_eq!(receiver.recv().await.unwrap()["i"], 999);
        assert_eq!(receiver.recv().await.unwrap()["kind"], "terminal");
        let _next = slot.reserve().await.unwrap();
    }
    #[tokio::test]
    async fn terminal_survives_the_compiled_node_being_dropped() {
        let (sink, mut receiver) = channel(1);
        {
            let slot = sink.terminal_slot();
            slot.reserve().await.unwrap().send(json!("last completion"));
        }
        drop(sink);
        assert_eq!(receiver.recv().await, Some(json!("last completion")));
        assert_eq!(receiver.recv().await, None);
    }
    #[tokio::test]
    async fn nested_nodes_do_not_share_a_global_terminal_permit_pool() {
        let (sink, mut receiver) = channel(1);
        let parent = sink.terminal_slot();
        let child = sink.terminal_slot();
        let parent_permit = parent.reserve().await.unwrap();
        let child_permit = child.reserve().await.unwrap();
        child_permit.send(json!("child"));
        parent_permit.send(json!("parent"));
        assert_eq!(receiver.recv().await, Some(json!("child")));
        assert_eq!(receiver.recv().await, Some(json!("parent")));
        drop(receiver);
        assert!(parent.reserve().await.is_err());
        assert!(sink.send(json!("closed")).await.is_err());
    }
    #[tokio::test]
    async fn pending_terminal_never_reserves_the_only_queue_slot_for_its_successor() {
        let (sink, mut receiver) = channel(1);
        let slot = sink.terminal_slot();
        sink.send(json!("earlier event")).await.unwrap();
        slot.try_reserve().unwrap().send(json!("first terminal"));
        assert_eq!(receiver.recv().await, Some(json!("earlier event")));
        // The normal queue is now free, but admission must reject rather than
        // reserve that single place for a terminal and deadlock its own start.
        assert!(matches!(
            slot.try_reserve(),
            Err(TryAcquireError::NoPermits)
        ));
        sink.send(json!("later event")).await.unwrap();
        assert_eq!(receiver.recv().await, Some(json!("first terminal")));
        let _next = slot.try_reserve().unwrap();
        assert_eq!(receiver.recv().await, Some(json!("later event")));
    }
}
