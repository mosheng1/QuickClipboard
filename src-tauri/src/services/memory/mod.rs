// 内存清理模块

#[cfg(windows)]
use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};

const HIDE_CLEANUP_DELAY_MS: u64 = 350;

#[cfg(windows)]
static HIDE_CLEANUP_PENDING: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
use windows::Win32::Foundation::CloseHandle;
#[cfg(windows)]
use windows::Win32::System::{
    Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    },
    Memory::{
        GetProcessHeap, HeapCompact, SetProcessWorkingSetSizeEx, QUOTA_LIMITS_HARDWS_MAX_DISABLE,
        QUOTA_LIMITS_HARDWS_MIN_DISABLE,
    },
    Threading::{
        GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
    },
};

#[cfg(windows)]
fn trim_process_working_set(process: windows::Win32::Foundation::HANDLE) {
    unsafe {
        let _ = SetProcessWorkingSetSizeEx(
            process,
            usize::MAX,
            usize::MAX,
            QUOTA_LIMITS_HARDWS_MIN_DISABLE | QUOTA_LIMITS_HARDWS_MAX_DISABLE,
        );
    }
}

#[cfg(windows)]
fn collect_descendant_process_ids(root_pid: u32) -> Vec<u32> {
    let snapshot = match unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
        Ok(snapshot) => snapshot,
        Err(_) => return Vec::new(),
    };

    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut children_by_parent: HashMap<u32, Vec<u32>> = HashMap::new();

    let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok();
    while has_entry {
        children_by_parent
            .entry(entry.th32ParentProcessID)
            .or_default()
            .push(entry.th32ProcessID);

        has_entry = unsafe { Process32NextW(snapshot, &mut entry) }.is_ok();
    }
    let _ = unsafe { CloseHandle(snapshot) };

    let mut descendants = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(root_pid);
    visited.insert(root_pid);

    while let Some(parent_pid) = queue.pop_front() {
        if let Some(children) = children_by_parent.get(&parent_pid) {
            for child_pid in children {
                if *child_pid == 0 || !visited.insert(*child_pid) {
                    continue;
                }
                descendants.push(*child_pid);
                queue.push_back(*child_pid);
            }
        }
    }

    descendants
}

#[cfg(windows)]
fn cleanup_descendant_processes(root_pid: u32) {
    for pid in collect_descendant_process_ids(root_pid) {
        let handle = unsafe {
            OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            )
        };

        if let Ok(process) = handle {
            trim_process_working_set(process);
            let _ = unsafe { CloseHandle(process) };
        }
    }
}

// 内存清理
#[cfg(windows)]
fn compact_process_heap() {
    unsafe {
        if let Ok(heap) = GetProcessHeap() {
            let _ = HeapCompact(heap, windows::Win32::System::Memory::HEAP_FLAGS(0));
        }
    }
}

#[cfg(windows)]
pub fn cleanup_memory() {
    compact_process_heap();
}

#[cfg(windows)]
pub fn cleanup_memory_with_working_set_trim() {
    compact_process_heap();
    unsafe {
        let process = GetCurrentProcess();
        trim_process_working_set(process);
        cleanup_descendant_processes(std::process::id());
    }
}

#[cfg(windows)]
pub fn cleanup_memory_respecting_settings() {
    if crate::get_settings().memory_optimization_enabled {
        cleanup_memory_with_working_set_trim();
    } else {
        cleanup_memory();
    }
}

#[cfg(windows)]
pub fn schedule_cleanup_after_window_inactive() {
    if HIDE_CLEANUP_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(HIDE_CLEANUP_DELAY_MS));
        cleanup_memory_respecting_settings();
        HIDE_CLEANUP_PENDING.store(false, Ordering::SeqCst);
    });
}

#[cfg(windows)]
pub fn schedule_cleanup_after_main_window_hide() {
    schedule_cleanup_after_window_inactive();
}

#[cfg(not(windows))]
pub fn cleanup_memory() {}

#[cfg(not(windows))]
pub fn cleanup_memory_with_working_set_trim() {}

#[cfg(not(windows))]
pub fn cleanup_memory_respecting_settings() {}

#[cfg(not(windows))]
pub fn schedule_cleanup_after_window_inactive() {}

#[cfg(not(windows))]
pub fn schedule_cleanup_after_main_window_hide() {}

pub fn init() {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(3));
        cleanup_memory_respecting_settings();
    });
}
