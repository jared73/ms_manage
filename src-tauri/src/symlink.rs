//! 跨平台 symlink / junction 工具
//!
//! Windows 上优先用 `mklink /J`（junction），无需 SeCreateSymbolicLinkPrivilege，
//! 普通用户即可创建。Unix 上用 `std::os::unix::fs::symlink`。
//!
//! 注：junction 只能指向**绝对路径**，调用方需把 target 转为绝对路径。

use std::path::Path;
use thiserror::Error;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

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
        let link_s = link.to_string_lossy().to_string();
        let target_s = target.to_string_lossy().to_string();
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J", &link_s, &target_s])
            .creation_flags(0x0800_0000)  // CREATE_NO_WINDOW — 防止 mklink 弹黑窗
            .output()?;
        if !out.status.success() {
            return Err(SymlinkError::Cmd(
                String::from_utf8_lossy(&out.stderr).to_string(),
            ));
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
