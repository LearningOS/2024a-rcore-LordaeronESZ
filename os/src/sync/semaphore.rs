//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                let mut task_inner = task.inner_exclusive_access();
                let sem_id = task_inner.sem_need;
                match task_inner.sem_allocation.iter().position(|&x| x.0 == sem_id) {
                    Some(index) => task_inner.sem_allocation[index].1 += 1,
                    None => task_inner.sem_allocation.push((sem_id, 1)),
                }
                task_inner.sem_need = usize::MAX;
                drop(task_inner);
                
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    pub fn down(&self) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        } else {
            let task = current_task().unwrap();
            let mut task_inner = task.inner_exclusive_access();
            let sem_id = task_inner.sem_need;
            match task_inner.sem_allocation.iter().position(|&x| x.0 == sem_id) {
                Some(index) => task_inner.sem_allocation[index].1 += 1,
                None => task_inner.sem_allocation.push((sem_id, 1)),
            }
            task_inner.sem_need = usize::MAX;
            drop(task_inner);
            drop(task);
        }
    }
}
