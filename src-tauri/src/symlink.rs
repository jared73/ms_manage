//! 跨平台 symlink / junction 工具
//!
//! Windows 上直接调 Win32 `CreateSymbolicLinkW`（directory 标志位 = junction 语义），
//! 避免再走 `cmd.exe /C mklink` 时的 shell 解析（即便参数是 Rust 控制的，过一道 cmd 也增加被
//! log/审计链锁定的负担，且 `CREATE_NO_WINDOW` 不是根治方案）。
//! Unix 上用 `std::os::unix::fs::symlink`。
//!
//! 注：junction / symlink 只能指向**绝对路径**，调用方需把 target 转为绝对路径。

use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SymlinkError {
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("link already exists: {0}")] AlreadyExists(String),
    #[error("link command failed: {0}")] Cmd(String),
}

pub fn create_dir_link(link: &Path, target: &Path) -> Result<(), SymlinkError> {
    if link.exists() {
        return Err(SymlinkError::AlreadyExists(link.display().to_string()));
    }
    let parent = link.parent().ok_or_else(|| {
        SymlinkError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "link has no parent"))
    })?;
    std::fs::create_dir_all(parent)?;

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        const SYMBOLIC_LINK_FLAG_DIRECTORY: u32 = 0x0001;
        const SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE: u32 = 0x0002; // Win10 1703+,免 SeCreateSymbolicLinkPrivilege

        let link_w: Vec<u16> = link.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let target_w: Vec<u16> = target.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let flags = SYMBOLIC_LINK_FLAG_DIRECTORY | SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE;
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::CreateSymbolicLinkW(
                link_w.as_ptr(),
                target_w.as_ptr(),
                flags,
            )
        };
        if ok != 0 {
            return Ok(());
        }
        let err = std::io::Error::last_os_error();
        // ERROR_ALREADY_EXISTS (183):reconcile 同步竞争产生,作为 AlreadyExists 返回便于上游决策
        if err.raw_os_error() == Some(183) {
            return Err(SymlinkError::AlreadyExists(link.display().to_string()));
        }
        // symlink 需要 SeCreateSymbolicLinkPrivilege（管理员或开发者模式）。
        // 没有特权时回退到 junction：junction 同样在目录间建立链接，且无需任何特权。
        eprintln!(
            "[symlink] CreateSymbolicLinkW {:?} -> {:?} failed: {err}; falling back to junction",
            link, target
        );
        return create_junction(link, target).map_err(SymlinkError::Io);
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
        Ok(())
    }
}

/// Windows:用 reparse point 原生创建 junction（`mklink /J` 语义）。
///
/// junction 不需要 SeCreateSymbolicLinkPrivilege，普通用户即可创建，
/// 是 symlink 权限不足时的可靠回退。substitute name 使用 `\??\` 前缀，
/// target 必须是绝对路径。
#[cfg(windows)]
fn create_junction(link: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::FSCTL_SET_REPARSE_POINT;
    use windows_sys::Win32::System::SystemServices::IO_REPARSE_TAG_MOUNT_POINT;

    // junction 必须先创建目标目录（CreateFileW 打开目录句柄需要它存在）
    std::fs::create_dir_all(link)?;
    // 记住目录是我们创建的（失败时要回滚，避免空目录残留阻塞后续 reconcile）
    let dir_created = link.exists() && link.read_dir().map(|mut r| r.next().is_none()).unwrap_or(false);

    // target 转成 NT namespace 绝对路径：\??\C:\...\target
    // 注意：Rust canonicalize 在 Windows 上可能返回 \\?\ verbatim 前缀，
    // 构造 substitute name 前必须剥掉，否则 \??\\?\C:\... 无效。
    let mut target_abs = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir()?.join(target)
    };
    {
        let s = target_abs.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            target_abs = PathBuf::from(stripped);
        }
    }
    let target_nt = format!(r"\??\{}", target_abs.to_string_lossy());
    let target_print = target_abs.to_string_lossy().to_string();

    // 布局:REPARSE_DATA_BUFFER(变长 MountPointReparseBuffer):
    //   tag(4) + data_len(2) + reserved(2) + Offset(2)+Len(2)+POffset(2)+PLen(2) = 16 字节头,
    //   PathBuffer 从 16 开始:[substitute utf16][null][print utf16][null]
    //   length 字段均不含 null 终止符;print name 起点 = substitute 长度 + 2(跳过 null)。
    let sub_bytes = target_nt.encode_utf16().count() * 2;
    let print_bytes = target_print.encode_utf16().count() * 2;
    if sub_bytes > 0xFFFE || print_bytes > 0xFFFE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "junction path too long",
        ));
    }

    const HEADER: usize = 16;
    let buffer_size = HEADER + sub_bytes + 2 + print_bytes + 2;
    let mut buf = vec![0u8; buffer_size];

    let write_u16 = |buf: &mut [u8], off: usize, v: u16| {
        buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
    };
    let write_u32 = |buf: &mut [u8], off: usize, v: u32| {
        buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
    };

    write_u32(&mut buf, 0, IO_REPARSE_TAG_MOUNT_POINT);
    // ReparseDataLength = MountPointReparseBuffer 部分字节数
    // (Offset/Len 4 字段 8 字节 + substitute + null + print + null)
    let data_len = (8 + sub_bytes + 2 + print_bytes + 2) as u16;
    write_u16(&mut buf, 4, data_len);
    write_u16(&mut buf, 6, 0); // reserved

    write_u16(&mut buf, 8, 0); // SubstituteNameOffset（相对 PathBuffer）
    write_u16(&mut buf, 10, sub_bytes as u16);
    write_u16(&mut buf, 12, (sub_bytes + 2) as u16); // PrintNameOffset
    write_u16(&mut buf, 14, print_bytes as u16);

    for (i, unit) in target_nt.encode_utf16().enumerate() {
        buf[HEADER + i * 2..HEADER + i * 2 + 2].copy_from_slice(&unit.to_le_bytes());
    }
    for (i, unit) in target_print.encode_utf16().enumerate() {
        buf[HEADER + sub_bytes + 2 + i * 2..HEADER + sub_bytes + 2 + i * 2 + 2]
            .copy_from_slice(&unit.to_le_bytes());
    }

    let link_w: Vec<u16> = link.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            link_w.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        let err = std::io::Error::last_os_error();
        rollback_empty_dir(link, dir_created);
        return Err(err);
    }

    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_SET_REPARSE_POINT,
            buf.as_ptr() as *const _,
            buffer_size as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    unsafe { CloseHandle(handle) };

    if ok == 0 {
        let err = std::io::Error::last_os_error();
        rollback_empty_dir(link, dir_created);
        return Err(err);
    }
    Ok(())
}

/// junction 创建失败时删除刚建的空目录，避免残留阻塞下次 reconcile
#[cfg(windows)]
fn rollback_empty_dir(link: &Path, dir_created: bool) {
    if !dir_created { return; }
    let _ = std::fs::remove_dir(link);
}

pub fn remove_link(link: &Path) -> Result<(), SymlinkError> {
    if !link.exists() && !is_link(link) {
        return Ok(());
    }
    #[cfg(windows)]
    {
        // rmdir 对 junction / symlink 都能移除链接本身（不删除 target）
        std::fs::remove_dir(link)?;
        Ok(())
    }
    #[cfg(unix)]
    {
        std::fs::remove_file(link)?;
        Ok(())
    }
}

/// 判断一个路径是否是 junction / symlink。
///
/// Windows 上 junction 是 reparse point，但 Rust 标准库的 `file_type().is_symlink()`
/// 对部分文件系统（NTFS junction）返回 false，导致 orphan 清理漏掉。
/// 这里用 Win32 GetFileAttributesW 检查 FILE_ATTRIBUTE_REPARSE_POINT 标志位。
#[cfg(windows)]
pub fn is_link(p: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

    let path_w: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let attrs = unsafe { windows_sys::Win32::Storage::FileSystem::GetFileAttributesW(path_w.as_ptr()) };
    if attrs == u32::MAX { // INVALID_FILE_ATTRIBUTES
        return false;
    }
    (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0
}

#[cfg(unix)]
pub fn is_link(p: &Path) -> bool {
    p.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn junction_fallback_creates_working_link() {
        let tmp = std::env::temp_dir().join(format!("ms-symlink-test-{}", std::process::id()));
        let target = tmp.join("target");
        let link = tmp.join("link");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("marker.txt"), "hello").unwrap();

        let result = create_dir_link(&link, &target);
        assert!(result.is_ok(), "create_dir_link failed: {result:?}");
        assert!(is_link(&link), "link should be a reparse point");
        assert!(
            link.join("marker.txt").exists(),
            "junction should resolve into target"
        );
        assert_eq!(
            std::fs::read_to_string(link.join("marker.txt")).unwrap(),
            "hello"
        );

        remove_link(&link).unwrap();
        assert!(!is_link(&link), "link should be removed");
        assert!(target.exists(), "target must survive link removal");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn junction_handles_verbatim_target() {
        // canonicalize 在 Windows 返回 \\?\C:\... verbatim 路径，substitute name 必须剥掉前缀
        let tmp = std::env::temp_dir().join(format!("ms-symlink-vtest-{}", std::process::id()));
        let target = tmp.join("target");
        let link = tmp.join("link");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("marker.txt"), "hi").unwrap();

        let verbatim = std::fs::canonicalize(&target).unwrap();
        assert!(
            verbatim.to_string_lossy().starts_with(r"\\?\") || verbatim.to_string_lossy().starts_with("\\\\?\\"),
            "canonicalize should yield verbatim path on Windows, got {:?}",
            verbatim
        );

        let result = create_dir_link(&link, &verbatim);
        assert!(result.is_ok(), "create_dir_link(verbatim) failed: {result:?}");
        assert_eq!(
            std::fs::read_to_string(link.join("marker.txt")).unwrap(),
            "hi"
        );

        remove_link(&link).unwrap();
        std::fs::remove_dir_all(&tmp).ok();
    }
}
