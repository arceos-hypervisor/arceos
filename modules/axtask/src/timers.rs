use alloc::sync::Arc;

use kspin::SpinNoIrq;
use lazyinit::LazyInit;
use timer_list::{TimeValue, TimerEvent, TimerList};

use axhal::time::wall_time;

use crate::{current_run_queue, AxTaskRef};

def_static!(
    TIMER_LIST,
    LazyInit<SpinNoIrq<TimerList<TaskWakeupEvent>>>,
    LazyInit::new()
);

struct TaskWakeupEvent(AxTaskRef);

impl TimerEvent for TaskWakeupEvent {
    fn callback(self, _now: TimeValue) {
        // Originally, irq and preempt are disabled by SpinNoIrq lock hold by RUN_QUEUE.
        // But, we can't use RUN_QUEUE here, so we need to disable irq and preempt manually.
        // Todo: figure out if `NoPreempt` is needed here.
        let _guard = kernel_guard::NoPreemptIrqSave::new();
        self.0.set_in_timer_list(false);
        current_run_queue().unblock_task(self.0, true);
    }
}

pub fn set_alarm_wakeup(deadline: TimeValue, task: AxTaskRef) {
    TIMER_LIST.with_current(|timer_list| {
        let mut timers = timer_list.lock();
        task.set_in_timer_list(true);
        timers.set(deadline, TaskWakeupEvent(task));
    })
}

pub fn cancel_alarm(task: &AxTaskRef) {
    TIMER_LIST.with_current(|timer_list| {
        let mut timers = timer_list.lock();
        task.set_in_timer_list(false);
        timers.cancel(|t| Arc::ptr_eq(&t.0, task));
    })
}

pub fn check_events() {
    loop {
        let now = wall_time();
        let event = TIMER_LIST.with_current(|timers| timers.lock().expire_one(now));
        if let Some((_deadline, event)) = event {
            event.callback(now);
        } else {
            break;
        }
    }
}

pub fn init() {
    let timer_list = SpinNoIrq::new(TimerList::new());
    unsafe {
        TIMER_LIST.current_ref_mut_raw().init_once(timer_list);
    }
}
