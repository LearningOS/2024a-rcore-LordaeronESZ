use core::cmp::max;
use core::iter;

use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());

    // set mutex_need to current 
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.mutex_need = mutex_id;
    drop(task_inner);
    drop(task);
    
    if process_inner.dlcheck_option {
        // initialize data structure for Banker's Algorithm:
        // Avavilable Vector, Allocation Matrix, Need Matrix
        let m = process_inner.mutex_list.len();
        let mut available: Vec<usize> = iter::repeat(1).take(m).collect();
        let mut allocation: Vec<Vec<usize>> = Vec::new();
        for (i, task_opt) in process_inner.tasks.iter().enumerate() {
            allocation.push(iter::repeat(0).take(m).collect());
            match task_opt {
                Some(task) => {
                    let task_inner = task.inner_exclusive_access();
                    for mid in &task_inner.mutex_allocation {
                        allocation[i][*mid] += 1;
                        available[*mid] -= 1;
                    }
                    drop(task_inner);
                }
                None => {}
            }
        }
        let mut need: Vec<Vec<usize>> = Vec::new();
        for (i, task_opt) in process_inner.tasks.iter().enumerate() {
            need.push(iter::repeat(0).take(m).collect());
            match task_opt {
                Some(task) => {
                    let task_inner = task.inner_exclusive_access();
                    let nid = task_inner.mutex_need;
                    if nid != usize::MAX {
                        need[i][nid] += 1;
                    }
                    drop(task_inner);
                }
                None => {}
            }
        }

        // banker_debug(available.clone(), allocation.clone(), need.clone());

        if !deadlock_check(available, allocation, need) {
            return -0xDEAD;
        }
    }

    // println!("[debug] deadlock check passed");

    
    drop(process_inner);
    drop(process);
    mutex.lock();

    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.mutex_allocation.push(mutex_id);
    task_inner.mutex_need = usize::MAX;
    drop(task_inner);
    drop(task);

    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
        
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);

    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    if let Some(index) = task_inner.mutex_allocation.iter().position(|&x| x == mutex_id) {
        task_inner.mutex_allocation.remove(index);
    }
    drop(task_inner);
    drop(task);
    
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    if let Some(index) = task_inner.sem_allocation.iter().position(|&x| x.0 == sem_id) {
        task_inner.sem_allocation.remove(index);
    }
    drop(task_inner);
    drop(task);
    
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());

    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.sem_need = sem_id;
    drop(task_inner);
    drop(task);

    if process_inner.dlcheck_option {
        // initialize data structure for Banker's Algorithm:
        // Avavilable Vector, Allocation Matrix, Need Matrix
        let m = process_inner.semaphore_list.len();
        let mut available:Vec<usize> = Vec::new();
        for sem_opt in &process_inner.semaphore_list {
            match sem_opt {
                Some(sem) => {
                    let sem_inner = sem.inner.exclusive_access();
                    // available.push(sem_inner.count as usize);
                    available.push(max(sem_inner.count, 0) as usize);
                    drop(sem_inner);
                }
                None => available.push(0),
            }
        }
        let mut allocation: Vec<Vec<usize>> = Vec::new();
        for (i, task_opt) in process_inner.tasks.iter().enumerate() {
            allocation.push(iter::repeat(0).take(m).collect());
            match task_opt {
                Some(task) => {
                    let task_inner = task.inner_exclusive_access();
                    for (id, alloc) in &task_inner.sem_allocation {
                        allocation[i][*id] += *alloc;
                    }
                    drop(task_inner);
                }
                None => {}
            }
        }
        let mut need: Vec<Vec<usize>> = Vec::new();
        for (i, task_opt) in process_inner.tasks.iter().enumerate() {
            need.push(iter::repeat(0).take(m).collect());
            match task_opt {
                Some(task) => {
                    let task_inner = task.inner_exclusive_access();
                    let nid = task_inner.sem_need;
                    if nid != usize::MAX {
                        need[i][nid] += 1;
                    }
                    drop(task_inner);
                }
                None => {}
            }
        }

        // banker_debug(available.clone(), allocation.clone(), need.clone());

        if !deadlock_check(available, allocation, need) {
            return -0xDEAD;
        }
    }

    // println!("[debug] deadlock check passed");
    
    drop(process_inner);
    sem.down();

    /*  The code below cannot be placed here,
        because the scheduling is unpridictable,
        so the allocation and need may not be updated in time! */
    
    // let task = current_task().unwrap();
    // let mut task_inner = task.inner_exclusive_access();
    // // task_inner.sem_allocation.push((sem_id, 1));
    // match task_inner.sem_allocation.iter().position(|&x| x.0 == sem_id) {
    //     Some(index) => task_inner.sem_allocation[index].1 += 1,
    //     None => task_inner.sem_allocation.push((sem_id, 1)),
    // }
    // task_inner.sem_need = usize::MAX;
    // drop(task_inner);
    // drop(task);

    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect");
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mut flag = 0;
    match _enabled {
        0 => process_inner.dlcheck_option = false,
        1 => process_inner.dlcheck_option = true,
        _ => flag = -1,
    }
    drop(process_inner);
    flag
}
/// Banker's Algoritm for dead lock check
fn deadlock_check(available: Vec<usize>, allocation: Vec<Vec<usize>>, need: Vec<Vec<usize>>) -> bool {
    // n: thread count  m: resources count
    let (n, m) = (allocation.len(), allocation[0].len());
    assert_eq!(available.len(), m);
    assert_eq!(need.len(), n);
    assert_eq!(need[0].len(), m);
    let mut work = available;
    let mut finish: Vec<bool> = iter::repeat(false).take(n).collect();
    loop {
        let mut idx = usize::MAX;
        for i in 0..n {
            let mut flag = true;
            if finish[i] {
                continue;
            }
            for j in 0..m {
                if need[i][j] > work[j] {
                    flag = false;
                    break;
                }
            }
            if flag {
                idx = i;
                break;
            }
        }
        // has found a thread meet the requirement
        if idx != usize::MAX {
            for j in 0..m {
                work[j] += allocation[idx][j];
            }
            finish[idx] = true;
        } else {
            break;
        }
    }
    finish.iter().all(|&x| x)
}

#[allow(unused)]
/// Banker's Algorithm debug
fn banker_debug(available: Vec<usize>, allocation: Vec<Vec<usize>>, need: Vec<Vec<usize>>) {
    // [debug]: print deadlock check related data structure
    println!("----------------------------------");
    println!("[Available]");
    for av in &available {
        print!("{} ", *av);
    }
    println!("\n[Allocation]");
    for alloc in &allocation {
        for al in alloc {
            print!("{} ", *al);
        }
        println!("");
    }
    println!("[Need]");
    for ned in &need {
        for nd in ned {
            print!("{} ", *nd);
        }
        println!("");
    }
    println!("----------------------------------");
}