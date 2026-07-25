//! 跨平台 symlink / junction 工具
//!
//! Windows 上直接调 Win32 `CreateSymbolicLinkW`（directory 标志位 = junction 语义），
//! 避免再走 `cmd.exe /C mklink` 时的 shell 解析（即便参数是 Rust 控制的，过一道 cmd 也增加被
//! log/审计链锁定的负担，且 `CREATE_NO_WINDOW` 不是根治方案）。
//! Unix 上用 `std::os::unix::fs::symlink`。
//!
//! 注：junction / symlink 只能指向**绝对路径**，调用方需把 target 转为绝对路径。

use std::path::Path;
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
        // Win32 CreateSymbolicLinkW —— SYMBOLIC_LINK_FLAG_DIRECTORY = 1,等价于 `mklink /J`
        // 不再 spawn cmd.exe,避免把 link / target 路径塞进 shell 解析。
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
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            // ERROR_ALREADY_EXISTS (183):reconcile 同步竞争产生,作为 AlreadyExists 返回便于上游决策
            if err.raw_os_error() == Some(183) {
                return Err(SymlinkError::AlreadyExists(link.display().to_string()));
            }
            return Err(SymlinkError::Io(err));
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
        Ok(())
    }
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
