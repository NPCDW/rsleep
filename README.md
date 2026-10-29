# rsleep

## 使用示例

不加单位，默认使用秒，如下睡眠 3600 秒
```bash
rsleep 3600
```

可以指定范围，如下睡眠 3600-7200 中的一个随机数秒
```bash
rsleep "3600-7200"
```

如下睡眠 20 分钟到 1 小时之间的一个随机数秒
```bash
rsleep 20m-1h
```

可以指定工作日睡眠 8 小时，休息日睡眠 11 小时 59 分钟 43 秒
```bash
rsleep --workday 8h --holiday 11h59m43s
```

可以指定周 1 到周 4 睡眠 8 小时，周五睡眠 7 小时，周六周天睡眠 12小时
```bash
rslepp --week-day 1-4 8 --week-day 5 7 --week-day 6-7 12 
```

# 说明

睡眠时长的参数可以为 数字 ，默认以 秒 为单位，可以是时间范围，也可以是时分秒组合，单位只包括 s秒 m分钟 h小时。

工作日节假日数据不内置、不硬编码，而是按当前年份从 holiday-cn 下载：
```
https://raw.githubusercontent.com/NateScarlet/holiday-cn/refs/heads/master/<年>.json
```

下载后的数据会缓存到系统缓存目录，之后优先使用缓存、不再重复下载：
- Linux：`~/.cache/rsleep/<年>.json`（遵循 XDG 规范）
- macOS：`~/Library/Caches/rsleep/<年>.json`
- Windows：`%LOCALAPPDATA%\rsleep\cache\<年>.json`

若本地没有缓存且下载失败，将打印警告并按工作日处理，使用 `--workday` 时长。