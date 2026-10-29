use crate::duration::DurationRange;
use anyhow::{Result, anyhow, bail};
use clap::Parser;
use std::collections::HashSet;

#[derive(Debug, Parser)]
#[command(
    name = "rsleep",
    version,
    about = "睡眠指定时长，支持随机范围、法定节假日和按星期配置",
    after_help = "示例：\n  rsleep 3600\n  rsleep 3600-7200\n  rsleep 20m-1h\n  rsleep --workday 8h --holiday 11h59m43s\n  rsleep --week-day 1-4 8 --week-day 5 7 --week-day 6-7 12\n\n时长不带单位时按秒计算，仅支持 s（秒）、m（分钟）、h（小时）。"
)]
pub struct Cli {
    #[arg(
        value_name = "时长",
        required_unless_present_any = ["workday", "holiday", "week_day"],
        conflicts_with_all = ["workday", "holiday", "week_day"]
    )]
    pub duration: Option<DurationRange>,

    #[arg(
        long,
        value_name = "时长",
        requires = "holiday",
        conflicts_with_all = ["duration", "week_day"]
    )]
    pub workday: Option<DurationRange>,

    #[arg(
        long,
        value_name = "时长",
        requires = "workday",
        conflicts_with_all = ["duration", "week_day"]
    )]
    pub holiday: Option<DurationRange>,

    #[arg(
        long = "week-day",
        value_names = ["星期", "时长"],
        num_args = 2,
        action = clap::ArgAction::Append,
        conflicts_with_all = ["duration", "workday", "holiday"]
    )]
    pub week_day: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeekdaySpec {
    pub days: Vec<u32>,
    pub duration: DurationRange,
}

impl WeekdaySpec {
    pub fn matches(&self, weekday: u32) -> bool {
        self.days.contains(&weekday)
    }
}

pub fn parse_weekday_specs(values: &[String]) -> Result<Vec<WeekdaySpec>> {
    let mut specs = Vec::new();
    let mut configured_days = HashSet::new();

    for pair in values.as_chunks::<2>().0 {
        let days = parse_weekdays(&pair[0])?;
        for day in &days {
            if !configured_days.insert(*day) {
                bail!("星期 {day} 配置了多次，请合并或调整范围");
            }
        }
        let duration = pair[1]
            .parse()
            .map_err(|error| anyhow!("无效的星期睡眠时长：{error}"))?;
        specs.push(WeekdaySpec { days, duration });
    }

    Ok(specs)
}

fn parse_weekdays(input: &str) -> Result<Vec<u32>> {
    let mut parts = input.trim().split('-');
    let start = parse_weekday(parts.next().unwrap())?;
    let end = match parts.next() {
        Some(value) => parse_weekday(value)?,
        None => start,
    };
    if parts.next().is_some() {
        bail!("无效的星期范围：{input}");
    }
    if start > end {
        bail!("星期范围起点不能大于终点：{input}");
    }
    Ok((start..=end).collect())
}

fn parse_weekday(input: &str) -> Result<u32> {
    let day = input
        .trim()
        .parse::<u32>()
        .map_err(|_| anyhow!("星期必须是 1 到 7 的整数：{input}"))?;
    if !(1..=7).contains(&day) {
        bail!("星期必须是 1（周一）到 7（周日）：{day}");
    }
    Ok(day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_readme_weekday_example() {
        let cli = Cli::try_parse_from([
            "rsleep",
            "--week-day",
            "1-4",
            "8",
            "--week-day",
            "5",
            "7",
            "--week-day",
            "6-7",
            "12",
        ])
        .unwrap();
        let specs = parse_weekday_specs(&cli.week_day).unwrap();
        assert_eq!(specs.len(), 3);
        assert!(specs[0].matches(1));
        assert!(specs[0].matches(4));
        assert_eq!(specs[1].duration.min_seconds, 7);
        assert_eq!(specs[2].duration.min_seconds, 12);
        assert!(specs[2].matches(7));
    }

    #[test]
    fn rejects_overlapping_or_invalid_weekdays() {
        for days in ["0", "8", "4-1", "1-2-3", "Mon"] {
            let values = vec![days.to_string(), "1".to_string()];
            assert!(parse_weekday_specs(&values).is_err(), "应拒绝 {days}");
        }
        let values = vec![
            "1-4".to_string(),
            "8".to_string(),
            "4".to_string(),
            "7".to_string(),
        ];
        assert!(parse_weekday_specs(&values).is_err());
    }

    #[test]
    fn modes_are_mutually_exclusive() {
        assert!(Cli::try_parse_from(["rsleep", "1", "--workday", "2", "--holiday", "3"]).is_err());
        assert!(Cli::try_parse_from(["rsleep", "--workday", "2"]).is_err());
        assert!(Cli::try_parse_from(["rsleep"]).is_err());
    }
}
