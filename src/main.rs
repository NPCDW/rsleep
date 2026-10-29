mod cache;
mod cli;
mod duration;
mod workday;

use anyhow::{Context, Result, anyhow};
use chrono::{Datelike, Local, NaiveDate};
use clap::Parser;
use cli::{Cli, parse_weekday_specs};
use duration::{DurationRange, format_duration};
use std::io::Write;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let duration = select_duration(&cli).await?;
    let seconds = duration.resolve();

    if duration.min_seconds != duration.max_seconds {
        println!(
            "随机范围 {} 秒到 {} 秒，本次选取 {}。",
            duration.min_seconds,
            duration.max_seconds,
            format_duration(seconds)
        );
    } else {
        println!("准备睡眠 {}。", format_duration(seconds));
    }
    std::io::stdout().flush().context("刷新标准输出失败")?;

    tokio::select! {
        () = tokio::time::sleep(Duration::from_secs(seconds)) => {
            println!("睡眠结束。");
        }
        result = tokio::signal::ctrl_c() => {
            result.context("监听 Ctrl+C 失败")?;
            println!("\n收到 Ctrl+C，提前结束睡眠。");
        }
    }
    Ok(())
}

async fn select_duration(cli: &Cli) -> Result<DurationRange> {
    if let Some(duration) = cli.duration {
        return Ok(duration);
    }

    if let (Some(workday), Some(holiday)) = (cli.workday, cli.holiday) {
        let today = Local::now().date_naive();
        let data = workday::fetch_year(today.year()).await;
        return choose_by_holiday_data(data, workday, holiday, today);
    }

    let specs = parse_weekday_specs(&cli.week_day)?;
    let weekday = workday::today_weekday();
    specs
        .iter()
        .find(|spec| spec.matches(weekday))
        .map(|spec| spec.duration)
        .ok_or_else(|| anyhow!("今天是星期 {weekday}，但没有配置对应的 --week-day 睡眠时长"))
}

/// 根据节假日数据选择睡眠时长；数据不可用时按工作日处理。
fn choose_by_holiday_data(
    data: Result<workday::YearData>,
    workday: DurationRange,
    holiday: DurationRange,
    today: NaiveDate,
) -> Result<DurationRange> {
    match data {
        Ok(data) => {
            println!("{} 年节假日数据来源：{}", data.year, data.source);
            println!("{}", workday::describe_date(&data, today));
            Ok(if data.is_workday(today) {
                workday
            } else {
                holiday
            })
        }
        Err(error) => {
            eprintln!("警告：获取节假日数据失败（{error:#}），今天将按工作日处理。");
            Ok(workday)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn selects_plain_duration_without_network() {
        let cli = Cli::try_parse_from(["rsleep", "1h2m3s"]).unwrap();
        assert_eq!(select_duration(&cli).await.unwrap().min_seconds, 3723);
    }

    #[tokio::test]
    async fn selects_matching_weekday() {
        let cli = Cli::try_parse_from(["rsleep", "--week-day", "1-7", "2h"]).unwrap();
        assert_eq!(select_duration(&cli).await.unwrap().min_seconds, 7200);
    }

    #[test]
    fn falls_back_to_workday_when_holiday_data_unavailable() {
        // 2026-01-03 是周六，若数据可用本应选休息日时长。
        let saturday = NaiveDate::from_ymd_opt(2026, 1, 3).unwrap();
        let duration = choose_by_holiday_data(
            Err(anyhow!("网络不可用")),
            DurationRange {
                min_seconds: 7200,
                max_seconds: 7200,
            },
            DurationRange {
                min_seconds: 43_183,
                max_seconds: 43_183,
            },
            saturday,
        )
        .unwrap();
        assert_eq!(duration.min_seconds, 7200);
        assert_eq!(duration.max_seconds, 7200);
    }

    #[test]
    fn selects_duration_by_holiday_data() {
        let workday = DurationRange {
            min_seconds: 7200,
            max_seconds: 7200,
        };
        let holiday = DurationRange {
            min_seconds: 43_183,
            max_seconds: 43_183,
        };
        let data = workday::parse_year_body(2026, "test", workday::SAMPLE).unwrap();

        // 元旦当天 -> 休息日时长。
        let new_year = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(
            choose_by_holiday_data(Ok(data.clone()), workday, holiday, new_year)
                .unwrap()
                .min_seconds,
            43_183
        );
        // 调休上班日 -> 工作日时长。
        let makeup = NaiveDate::from_ymd_opt(2026, 1, 4).unwrap();
        assert_eq!(
            choose_by_holiday_data(Ok(data), workday, holiday, makeup)
                .unwrap()
                .min_seconds,
            7200
        );
    }
}
