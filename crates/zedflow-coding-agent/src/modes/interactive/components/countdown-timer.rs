//! Deterministic countdown state; the interactive owner calls `tick` once per second.

pub struct CountdownTimer {
    remaining_seconds: u64,
    active: bool,
    on_tick: Box<dyn FnMut(u64)>,
    on_expire: Box<dyn FnMut()>,
}

impl CountdownTimer {
    pub fn new(
        timeout_ms: u64,
        mut on_tick: impl FnMut(u64) + 'static,
        on_expire: impl FnMut() + 'static,
    ) -> Self {
        let remaining_seconds = timeout_ms.div_ceil(1_000);
        on_tick(remaining_seconds);
        Self {
            remaining_seconds,
            active: true,
            on_tick: Box::new(on_tick),
            on_expire: Box::new(on_expire),
        }
    }

    #[must_use]
    pub fn remaining_seconds(&self) -> u64 {
        self.remaining_seconds
    }
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn tick(&mut self) {
        if !self.active {
            return;
        }
        self.remaining_seconds = self.remaining_seconds.saturating_sub(1);
        (self.on_tick)(self.remaining_seconds);
        if self.remaining_seconds == 0 {
            self.dispose();
            (self.on_expire)();
        }
    }

    pub fn dispose(&mut self) {
        self.active = false;
    }
}
