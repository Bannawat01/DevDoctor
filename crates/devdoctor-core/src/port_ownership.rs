#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortOwner {
    pub pid: u32,
    pub process_name: Option<String>,
    pub executable_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipResult {
    Owners(Vec<PortOwner>),
    PermissionDenied,
    Unsupported,
    Unavailable,
}

#[cfg(windows)]
pub fn lookup_tcp_owners(port: u16) -> OwnershipResult {
    windows::lookup(port)
}

#[cfg(target_os = "linux")]
pub fn lookup_tcp_owners(port: u16) -> OwnershipResult {
    linux::lookup(port)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn lookup_tcp_owners(port: u16) -> OwnershipResult {
    let _ = port;
    OwnershipResult::Unsupported
}

#[cfg(windows)]
mod windows {
    use super::{OwnershipResult, PortOwner};
    use std::{path::Path, ptr::null_mut};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
        },
        Networking::WinSock::AF_INET,
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
    };

    pub fn lookup(port: u16) -> OwnershipResult {
        let mut bytes = 0u32;
        let status = unsafe {
            GetExtendedTcpTable(
                null_mut(),
                &mut bytes,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if status != 122 || bytes == 0 {
            return OwnershipResult::Unavailable;
        }
        let mut buffer = vec![0u8; bytes as usize];
        let status = unsafe {
            GetExtendedTcpTable(
                buffer.as_mut_ptr().cast(),
                &mut bytes,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if status != 0 {
            return OwnershipResult::Unavailable;
        }
        let table = buffer.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
        let count = unsafe { (*table).dwNumEntries as usize };
        let rows = unsafe { std::slice::from_raw_parts((*table).table.as_ptr(), count) };
        let mut owners = Vec::new();
        for row in rows {
            if u16::from_be(row.dwLocalPort as u16) == port && row.dwState == 2 {
                let pid = row.dwOwningPid;
                let (process_name, executable_path) = process_details(pid);
                owners.push(PortOwner {
                    pid,
                    process_name,
                    executable_path,
                });
            }
        }
        OwnershipResult::Owners(owners)
    }

    pub fn process_details(pid: u32) -> (Option<String>, Option<String>) {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return (None, None);
        }
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        let ok =
            unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) } != 0;
        unsafe { CloseHandle(handle) };
        if !ok {
            return (None, None);
        }
        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        let name = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string);
        (name, Some(path))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{OwnershipResult, PortOwner};
    use std::fs;
    use std::path::Path;

    pub fn lookup(port: u16) -> OwnershipResult {
        let contents = match fs::read_to_string("/proc/net/tcp") {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return OwnershipResult::PermissionDenied;
            }
            Err(_) => return OwnershipResult::Unavailable,
        };
        let inodes: Vec<String> = contents
            .lines()
            .skip(1)
            .filter_map(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                let local = fields.get(1)?.rsplit_once(':')?.1;
                let local_port = u16::from_str_radix(local, 16).ok()?;
                if local_port == port && fields.get(3) == Some(&"0A") {
                    Some(fields.get(9)?.to_string())
                } else {
                    None
                }
            })
            .collect();
        if inodes.is_empty() {
            return OwnershipResult::Owners(Vec::new());
        }
        let mut owners = Vec::new();
        let proc = match fs::read_dir("/proc") {
            Ok(proc) => proc,
            Err(_) => return OwnershipResult::PermissionDenied,
        };
        for entry in proc.flatten() {
            let name = entry.file_name();
            let Some(pid) = name.to_str().and_then(|name| name.parse::<u32>().ok()) else {
                continue;
            };
            let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
                continue;
            };
            for fd in fds.flatten() {
                let Ok(target) = fs::read_link(fd.path()) else {
                    continue;
                };
                let target = target.to_string_lossy();
                if inodes
                    .iter()
                    .any(|inode| target == format!("socket:[{inode}]"))
                {
                    let path = fs::read_link(format!("/proc/{pid}/exe")).ok();
                    let executable_path = path.as_ref().map(|path| path.display().to_string());
                    let process_name = path
                        .as_deref()
                        .and_then(Path::file_name)
                        .and_then(|name| name.to_str())
                        .map(str::to_string);
                    owners.push(PortOwner {
                        pid,
                        process_name,
                        executable_path,
                    });
                    break;
                }
            }
        }
        OwnershipResult::Owners(owners)
    }
}
