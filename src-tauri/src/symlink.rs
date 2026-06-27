//! 跨平台 symlink / junction 工具
//!
//! Windows 上优先用 `mklink /J`（junction），无需 SeCreateSymbolicLinkPrivilege，
//! 普通用户即可创建。Unix 上用 `std::os::unix::fs::symlink`。
//!
//! 注：junction 只能指向**绝对路径**，调用方需把 target 转为绝对路径。

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
        let link_s = link.to_string_lossy().to_string();
        let target_s = target.to_string_lossy().to_string();
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J", &link_s, &target_s])
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
    if !link.exists() && !junction_point(link) {
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

/// 判断一个路径是否是 junction / symlink
pub fn is_link(p: &Path) -> bool {
    p.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(windows)]
fn junction_point(p: &Path) -> bool {
    // Windows 上 junction 没有稳定的 std API，但 symlink_metadata 返回的 file_type
    // 对 junction 也标为 symlink。直接复用 is_link 即可。
    is_link(p)
}

#[cfg(unix)]
fn junction_point(_p: &Path) -> bool { false }
