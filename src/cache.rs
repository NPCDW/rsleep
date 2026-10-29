//! 节假日数据文件的本地缓存管理。
//!
//! 缓存目录按平台惯例选择：
//! - Linux 等：`$XDG_CACHE_HOME/rsleep`，默认 `~/.cache/rsleep`
//! - macOS：`~/Library/Caches/rsleep`
//! - Windows：`%LOCALAPPDATA%\rsleep\cache`
//! - 以上均不可用时，退回当前目录下 `.rsleep-cache`

use std::fs;
use std::path::PathBuf;

/// 应用名，用于拼接各平台缓存目录。
pub const APP_NAME: &str = "rsleep";

/// 返回 {year}.json 对应的缓存文件路径（所在目录会自动创建）。
pub fn year_file(year: i32) -> PathBuf {
    cache_dir().join(file_name(year))
}

/// 返回节假日数据缓存目录，不存在时尝试创建。
pub fn cache_dir() -> PathBuf {
    let dir = resolve_cache_dir();
    if let Err(error) = fs::create_dir_all(&dir) {
        eprintln!("警告：无法创建缓存目录 {}：{error}", dir.display());
    }
    dir
}

fn file_name(year: i32) -> String {
    format!("{year}.json")
}

fn resolve_cache_dir() -> PathBuf {
    platform_cache_dir().unwrap_or_else(fallback_cache_dir)
}

/// 读取平台环境变量定位目录；取不到时返回 None。
fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn platform_cache_dir() -> Option<PathBuf> {
    env_dir("HOME").map(|home| home.join("Library").join("Caches").join(APP_NAME))
}

#[cfg(target_os = "windows")]
fn platform_cache_dir() -> Option<PathBuf> {
    env_dir("LOCALAPPDATA").map(|base| base.join(APP_NAME).join("cache"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_cache_dir() -> Option<PathBuf> {
    match env_dir("XDG_CACHE_HOME") {
        Some(xdg) => Some(xdg.join(APP_NAME)),
        None => env_dir("HOME").map(|home| home.join(".cache").join(APP_NAME)),
    }
}

fn fallback_cache_dir() -> PathBuf {
    std::env::current_dir()
        .map(|dir| dir.join(format!(".{APP_NAME}-cache")))
        .unwrap_or_else(|_| PathBuf::from(format!(".{APP_NAME}-cache")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_file_name_is_year_json() {
        assert_eq!(file_name(2026), "2026.json");
        assert_eq!(file_name(1999), "1999.json");
    }
}
