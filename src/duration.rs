use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurationRange {
    pub min_seconds: u64,
    pub max_seconds: u64,
}

impl DurationRange {
    pub fn resolve(self) -> u64 {
        if self.min_seconds == self.max_seconds {
            self.min_seconds
        } else {
            rand::random_range(self.min_seconds..=self.max_seconds)
        }
    }
}

impl FromStr for DurationRange {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        if input.is_empty() {
            return Err("睡眠时长不能为空".to_string());
        }

        let mut parts = input.split('-');
        let min = parse_duration(parts.next().unwrap())?;
        let max = match parts.next() {
            Some(value) => parse_duration(value)?,
            None => min,
        };
        if parts.next().is_some() {
            return Err(format!("无效的时长范围：{input}"));
        }
        if min > max {
            return Err(format!("范围起点不能大于终点：{input}"));
        }

        Ok(Self {
            min_seconds: min,
            max_seconds: max,
        })
    }
}

fn parse_duration(input: &str) -> Result<u64, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("时长范围两侧都必须有值".to_string());
    }
    if input.bytes().all(|byte| byte.is_ascii_digit()) {
        return input
            .parse::<u64>()
            .map_err(|_| format!("秒数超出支持范围：{input}"));
    }

    let mut rest = input;
    let mut total = 0u64;
    let mut previous_rank = None;

    while !rest.is_empty() {
        let digits = rest
            .bytes()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if digits == 0 {
            return Err(format!("无效时长：{input}，应为数字加 s/m/h 单位"));
        }
        let amount = rest[..digits]
            .parse::<u64>()
            .map_err(|_| format!("数值超出支持范围：{input}"))?;
        let unit = rest[digits..]
            .chars()
            .next()
            .ok_or_else(|| format!("缺少时间单位：{input}"))?;
        let (multiplier, rank) = match unit {
            'h' => (3600, 0),
            'm' => (60, 1),
            's' => (1, 2),
            _ => return Err(format!("不支持的时间单位：{unit}，仅支持 s/m/h")),
        };
        if previous_rank.is_some_and(|previous| rank <= previous) {
            return Err(format!("时间单位必须按 h、m、s 顺序且不能重复：{input}"));
        }
        previous_rank = Some(rank);

        let component = amount
            .checked_mul(multiplier)
            .ok_or_else(|| format!("时长超出支持范围：{input}"))?;
        total = total
            .checked_add(component)
            .ok_or_else(|| format!("时长超出支持范围：{input}"))?;
        rest = &rest[digits + unit.len_utf8()..];
    }

    Ok(total)
}

pub fn format_duration(seconds: u64) -> String {
    if seconds == 0 {
        return "0秒".to_string();
    }

    let hours = seconds / 3600;
    let minutes = seconds % 3600 / 60;
    let seconds = seconds % 60;
    let mut output = String::new();
    if hours > 0 {
        output.push_str(&format!("{hours}小时"));
    }
    if minutes > 0 {
        output.push_str(&format!("{minutes}分钟"));
    }
    if seconds > 0 {
        output.push_str(&format!("{seconds}秒"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(input: &str) -> u64 {
        input.parse::<DurationRange>().unwrap().min_seconds
    }

    #[test]
    fn parses_readme_examples() {
        assert_eq!(seconds("3600"), 3600);
        assert_eq!(seconds("20m"), 1200);
        assert_eq!(seconds("8h"), 28_800);
        assert_eq!(seconds("11h59m43s"), 43_183);
    }

    #[test]
    fn parses_ranges() {
        let range = "3600-7200".parse::<DurationRange>().unwrap();
        assert_eq!((range.min_seconds, range.max_seconds), (3600, 7200));
        let range = "20m-1h".parse::<DurationRange>().unwrap();
        assert_eq!((range.min_seconds, range.max_seconds), (1200, 3600));
        for _ in 0..20 {
            assert!((1200..=3600).contains(&range.resolve()));
        }
    }

    #[test]
    fn rejects_invalid_durations() {
        for input in ["", "abc", "1d", "1h30", "2h-1h", "1-2-3", "1m1h", "1ss"] {
            assert!(input.parse::<DurationRange>().is_err(), "应拒绝 {input:?}");
        }
    }

    #[test]
    fn formats_duration() {
        assert_eq!(format_duration(0), "0秒");
        assert_eq!(format_duration(43_183), "11小时59分钟43秒");
        assert_eq!(format_duration(3600), "1小时");
    }
}
