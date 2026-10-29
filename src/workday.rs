//! 使用 holiday-cn 的年度数据判断中国大陆法定工作日和休息日。

use crate::cache;
use anyhow::{Context, Result, bail};
use chrono::{Datelike, NaiveDate, Weekday};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

pub const BASE_URL: &str =
    "https://raw.githubusercontent.com/NateScarlet/holiday-cn/refs/heads/master";

#[derive(Debug, Clone)]
pub struct YearData {
    pub year: i32,
    pub source: String,
    days: HashMap<NaiveDate, DayInfo>,
}

#[derive(Debug, Clone)]
struct DayInfo {
    name: String,
    is_off_day: bool,
}

#[derive(Debug, Deserialize)]
struct RawDoc {
    year: Option<i32>,
    days: Vec<RawDay>,
}

#[derive(Debug, Deserialize)]
struct RawDay {
    #[serde(default)]
    name: String,
    date: NaiveDate,
    #[serde(rename = "isOffDay")]
    is_off_day: bool,
}

impl YearData {
    pub fn is_workday(&self, date: NaiveDate) -> bool {
        match self.days.get(&date) {
            Some(day) => !day.is_off_day,
            None => !matches!(date.weekday(), Weekday::Sat | Weekday::Sun),
        }
    }

    pub fn day_name(&self, date: NaiveDate) -> Option<&str> {
        self.days
            .get(&date)
            .map(|day| day.name.as_str())
            .filter(|name| !name.is_empty())
    }

    pub fn is_makeup_workday(&self, date: NaiveDate) -> bool {
        self.days.get(&date).is_some_and(|day| !day.is_off_day)
    }
}

/// 获取指定年份的节假日数据：优先使用本地缓存，无缓存时下载并写入缓存。
pub async fn fetch_year(year: i32) -> Result<YearData> {
    fetch_year_from(year, &cache::year_file(year), &year_url(year)).await
}

/// 节假日数据的获取逻辑，缓存路径与数据源地址由参数指定，便于测试。
async fn fetch_year_from(year: i32, cached_file: &Path, source: &str) -> Result<YearData> {
    match read_cached(year, cached_file) {
        Some(Ok(data)) => return Ok(data),
        Some(Err(_)) => {
            let _ = fs::remove_file(cached_file);
        }
        None => {}
    }

    let body = download_year(source).await?;
    save_to_cache(cached_file, &body);
    parse_year_body(year, source, &body)
}

/// 读取缓存文件；返回 None 表示无缓存或读取失败，调用方应转而下载。
fn read_cached(year: i32, path: &Path) -> Option<Result<YearData>> {
    if !path.exists() {
        return None;
    }
    match fs::read_to_string(path) {
        Ok(body) => {
            let source = format!("本地缓存：{}", path.display());
            Some(parse_year_body(year, &source, &body).map_err(|error| {
                eprintln!(
                    "警告：缓存文件 {} 无法解析（{error}），将重新下载。",
                    path.display()
                );
                error
            }))
        }
        Err(error) => {
            eprintln!(
                "警告：无法读取缓存文件 {}：{error}，将重新下载。",
                path.display()
            );
            None
        }
    }
}

async fn download_year(source: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .context("初始化 HTTP 客户端失败")?;
    let response = client
        .get(source)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .with_context(|| format!("请求节假日数据失败：{source}"))?;
    let status = response.status();
    let body = response.text().await.context("读取节假日数据失败")?;
    if !status.is_success() {
        let detail: String = body.chars().take(200).collect();
        bail!("节假日数据请求返回 HTTP {status}：{detail}");
    }
    Ok(body)
}

/// 写入缓存文件；失败只警告，不影响主流程。
fn save_to_cache(path: &Path, body: &str) {
    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        eprintln!("警告：无法创建缓存目录 {}：{error}", parent.display());
        return;
    }
    if let Err(error) = fs::write(path, body) {
        eprintln!("警告：无法写入缓存文件 {}：{error}", path.display());
    }
}

pub fn year_url(year: i32) -> String {
    format!("{BASE_URL}/{year}.json")
}

pub(crate) fn parse_year_body(year: i32, source: &str, body: &str) -> Result<YearData> {
    let document: RawDoc =
        serde_json::from_str(body).with_context(|| format!("解析节假日数据失败：{source}"))?;
    if document.year.is_some_and(|actual| actual != year) {
        bail!(
            "节假日数据年份不匹配：请求 {year} 年，返回 {:?} 年",
            document.year
        );
    }
    if document.days.is_empty() {
        bail!("{year} 年节假日数据为空，可能该年度安排尚未公布");
    }

    let days = document
        .days
        .into_iter()
        .map(|day| {
            (
                day.date,
                DayInfo {
                    name: day.name,
                    is_off_day: day.is_off_day,
                },
            )
        })
        .collect();
    Ok(YearData {
        year,
        source: source.to_string(),
        days,
    })
}

pub fn describe_date(data: &YearData, date: NaiveDate) -> String {
    let weekday = match date.weekday() {
        Weekday::Mon => "周一",
        Weekday::Tue => "周二",
        Weekday::Wed => "周三",
        Weekday::Thu => "周四",
        Weekday::Fri => "周五",
        Weekday::Sat => "周六",
        Weekday::Sun => "周日",
    };
    let kind = if data.is_workday(date) {
        if data.is_makeup_workday(date) {
            "法定工作日（调休上班）"
        } else {
            "工作日"
        }
    } else {
        "休息日"
    };
    let name = data
        .day_name(date)
        .map(|name| format!("，{name}"))
        .unwrap_or_default();
    format!(
        "{}（{}）是{}{}",
        date.format("%Y-%m-%d"),
        weekday,
        kind,
        name
    )
}

pub fn today_weekday() -> u32 {
    chrono::Local::now().weekday().number_from_monday()
}

/// 测试用节假日数据样例。
#[cfg(test)]
pub(crate) const SAMPLE: &str = r#"{
    "year": 2026,
    "days": [
        {"name":"元旦","date":"2026-01-01","isOffDay":true},
        {"name":"元旦","date":"2026-01-04","isOffDay":false}
    ]
}"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 生成一个唯一的临时目录路径（不创建目录）。
    fn temp_dir_path(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "{}-test-{}-{tag}-{id}",
            cache::APP_NAME,
            std::process::id()
        ))
    }

    /// 启动一个一次性本地 HTTP 服务，返回指定状态行与响应体，返回其地址。
    fn spawn_http_server(status_line: &'static str, body: &'static str) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0u8; 1024];
                let _ = stream.read(&mut request);
                let response = format!(
                    "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn honors_holidays_and_makeup_workdays() {
        let data = parse_year_body(2026, "test", SAMPLE).unwrap();
        assert!(!data.is_workday(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()));
        assert!(data.is_workday(NaiveDate::from_ymd_opt(2026, 1, 4).unwrap()));
        assert!(data.is_workday(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()));
        assert!(!data.is_workday(NaiveDate::from_ymd_opt(2026, 1, 3).unwrap()));
    }

    #[test]
    fn validates_year_and_nonempty_data() {
        assert!(parse_year_body(2025, "test", SAMPLE).is_err());
        assert!(parse_year_body(2026, "test", r#"{"year":2026,"days":[]}"#).is_err());
    }

    #[test]
    fn builds_current_year_url() {
        assert_eq!(
            year_url(2026),
            "https://raw.githubusercontent.com/NateScarlet/holiday-cn/refs/heads/master/2026.json"
        );
    }

    #[test]
    fn read_cached_reports_missing_valid_and_corrupted_files() {
        let dir = temp_dir_path("read-cached");
        fs::create_dir_all(&dir).unwrap();

        let missing = dir.join("missing.json");
        assert!(read_cached(2026, &missing).is_none(), "无缓存应返回 None");

        let corrupted = dir.join("corrupted.json");
        fs::write(&corrupted, "这不是合法的 JSON").unwrap();
        assert!(
            read_cached(2026, &corrupted).unwrap().is_err(),
            "损坏缓存应返回错误"
        );

        let valid = dir.join("valid.json");
        fs::write(&valid, SAMPLE).unwrap();
        let data = read_cached(2026, &valid).unwrap().unwrap();
        assert_eq!(data.source, format!("本地缓存：{}", valid.display()));
        assert!(!data.is_workday(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_to_cache_creates_directory_and_writes_file() {
        let dir = temp_dir_path("save-cache");
        let file = dir.join("nested").join("2026.json");
        save_to_cache(&file, SAMPLE);
        assert_eq!(fs::read_to_string(&file).unwrap(), SAMPLE);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn download_year_returns_body_on_success() {
        let url = spawn_http_server("HTTP/1.1 200 OK", SAMPLE);
        assert_eq!(download_year(&url).await.unwrap(), SAMPLE);
    }

    #[tokio::test]
    async fn download_year_rejects_error_status() {
        let url = spawn_http_server("HTTP/1.1 500 Internal Server Error", "服务端错误");
        assert!(download_year(&url).await.is_err());
    }

    #[tokio::test]
    async fn download_year_fails_when_unreachable() {
        assert!(download_year("http://127.0.0.1:9").await.is_err());
    }

    #[tokio::test]
    async fn fetch_year_from_downloads_and_caches_when_missing() {
        let url = spawn_http_server("HTTP/1.1 200 OK", SAMPLE);
        let dir = temp_dir_path("download-cache");
        let file = dir.join("2026.json");

        let data = fetch_year_from(2026, &file, &url).await.unwrap();
        assert!(data.is_workday(NaiveDate::from_ymd_opt(2026, 1, 4).unwrap()));
        assert_eq!(fs::read_to_string(&file).unwrap(), SAMPLE, "应写入缓存");

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn fetch_year_from_prefers_cache_without_downloading() {
        let dir = temp_dir_path("cache-hit");
        let file = dir.join("2026.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&file, SAMPLE).unwrap();

        // 传入必然失败的下载地址：若实现错误地发起下载，本测试就会失败。
        let data = fetch_year_from(2026, &file, "http://127.0.0.1:9")
            .await
            .unwrap();
        assert_eq!(data.source, format!("本地缓存：{}", file.display()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn fetch_year_from_replaces_corrupted_cache_with_download() {
        let url = spawn_http_server("HTTP/1.1 200 OK", SAMPLE);
        let dir = temp_dir_path("corrupt-cache");
        let file = dir.join("2026.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&file, "这不是合法的 JSON").unwrap();

        let data = fetch_year_from(2026, &file, &url).await.unwrap();
        assert!(data.is_workday(NaiveDate::from_ymd_opt(2026, 1, 4).unwrap()));
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            SAMPLE,
            "损坏缓存应被新下载内容替换"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn fetch_year_from_propagates_download_failure() {
        let dir = temp_dir_path("download-failure");
        let file = dir.join("2026.json");

        let result = fetch_year_from(2026, &file, "http://127.0.0.1:9").await;
        assert!(result.is_err());
        assert!(!file.exists(), "下载失败不应留下缓存文件");

        let _ = fs::remove_dir_all(&dir);
    }
}
