use std::collections::{BTreeSet, HashSet};
use std::env;
use std::ffi::c_char;
use std::fs;
use std::io;
use std::os::raw::{c_int, c_long};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_CONFIG_PATH: &str = "/etc/daily-poweroff.conf";
const DEFAULT_SHUTDOWN_COMMAND: &str = "systemctl poweroff";
const DEFAULT_WARNING_SECONDS: &[i64] = &[3600, 1800, 900, 600, 300, 180, 120, 60, 30, 10];
const SYSTEM_BIN_PATH: &str = "/usr/local/bin/daily-poweroff";
const SYSTEMD_SERVICE_PATH: &str = "/etc/systemd/system/daily-poweroff.service";
const SYSTEMD_SERVICE_NAME: &str = "daily-poweroff.service";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Language {
    En,
    ZhCn,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Tm {
    tm_sec: c_int,
    tm_min: c_int,
    tm_hour: c_int,
    tm_mday: c_int,
    tm_mon: c_int,
    tm_year: c_int,
    tm_wday: c_int,
    tm_yday: c_int,
    tm_isdst: c_int,
    tm_gmtoff: c_long,
    tm_zone: *const c_char,
}

unsafe extern "C" {
    fn localtime_r(timep: *const c_long, result: *mut Tm) -> *mut Tm;
    fn mktime(tm: *mut Tm) -> c_long;
}

#[derive(Clone, Copy, Debug)]
struct TimeOfDay {
    hour: u32,
    minute: u32,
}

#[derive(Debug)]
struct Config {
    enabled: bool,
    shutdown_time: Option<TimeOfDay>,
    canceled_dates: BTreeSet<String>,
    warning_seconds: Vec<i64>,
    shutdown_command: String,
    dry_run: bool,
    language: Language,
}

#[derive(Debug)]
struct LocalNow {
    epoch: i64,
    date: String,
    time: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("daily-poweroff: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (config_path, args) = parse_global_args(env::args().skip(1).collect())?;
    if args.is_empty() {
        print_usage(config_language_or_default(&config_path));
        return Ok(());
    }

    match args[0].as_str() {
        "set" => cmd_set(&config_path, &args[1..]),
        "cancel" => cmd_cancel(&config_path, &args[1..], true),
        "resume" | "uncancel" => cmd_cancel(&config_path, &args[1..], false),
        "status" => cmd_status(&config_path),
        "enable" => cmd_enable(&config_path, true),
        "disable" => cmd_enable(&config_path, false),
        "daemon" | "run" => daemon(&config_path),
        "test-broadcast" => {
            let config = Config::load(&config_path)?;
            broadcast(message_test_broadcast(config.language));
            Ok(())
        }
        "set-language" => cmd_set_language(&config_path, &args[1..]),
        "install-systemd" => cmd_install_systemd(&args[1..]),
        "print-systemd" => cmd_print_systemd(),
        "-h" | "--help" | "help" => {
            print_usage(config_language_or_default(&config_path));
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    }
}

fn parse_global_args(mut args: Vec<String>) -> Result<(PathBuf, Vec<String>), String> {
    let mut config_path = env::var_os("SMART_SHUTDOWN_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    let mut cleaned = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--config requires a file path".to_string())?;
                config_path = PathBuf::from(value);
            }
            _ => cleaned.push(std::mem::take(&mut args[i])),
        }
        i += 1;
    }
    Ok((config_path, cleaned))
}

fn config_language_or_default(config_path: &Path) -> Language {
    Config::load(config_path)
        .map(|config| config.language)
        .unwrap_or(Language::En)
}

fn print_usage(language: Language) {
    match language {
        Language::En => print_usage_en(),
        Language::ZhCn => print_usage_zh_cn(),
    }
}

fn print_usage_en() {
    println!(
        r#"daily-poweroff

Usage:
  daily-poweroff set HH:MM [--warning-minutes 60,30,15,10,5,3,2,1] [--dry-run true|false]
  daily-poweroff cancel [YYYY-MM-DD ...]
  daily-poweroff cancel --days N [--from YYYY-MM-DD]
  daily-poweroff resume [YYYY-MM-DD ...]
  daily-poweroff resume --days N [--from YYYY-MM-DD]
  daily-poweroff status
  daily-poweroff enable|disable
  daily-poweroff set-language en|zh-CN
  daily-poweroff daemon
  daily-poweroff test-broadcast
  daily-poweroff install-systemd

Global options:
  -c, --config PATH    default: /etc/daily-poweroff.conf

Examples:
  sudo daily-poweroff set 17:30
  sudo daily-poweroff cancel
  sudo daily-poweroff cancel --days 3
  sudo daily-poweroff cancel --from 2026-05-25 --days 3
  sudo daily-poweroff resume --days 3
  sudo daily-poweroff resume 2026-05-26
  sudo daily-poweroff set-language zh-CN

When no explicit date or --from is given, cancel/resume start from the next
scheduled poweroff date.
"#
    );
}

fn print_usage_zh_cn() {
    println!(
        r#"daily-poweroff

用法：
  daily-poweroff set HH:MM [--warning-minutes 60,30,15,10,5,3,2,1] [--dry-run true|false]
  daily-poweroff cancel [YYYY-MM-DD ...]
  daily-poweroff cancel --days N [--from YYYY-MM-DD]
  daily-poweroff resume [YYYY-MM-DD ...]
  daily-poweroff resume --days N [--from YYYY-MM-DD]
  daily-poweroff status
  daily-poweroff enable|disable
  daily-poweroff set-language en|zh-CN
  daily-poweroff daemon
  daily-poweroff test-broadcast
  daily-poweroff install-systemd

全局选项：
  -c, --config PATH    默认：/etc/daily-poweroff.conf

示例：
  sudo daily-poweroff set 17:30
  sudo daily-poweroff cancel
  sudo daily-poweroff cancel --days 3
  sudo daily-poweroff cancel --from 2026-05-25 --days 3
  sudo daily-poweroff resume --days 3
  sudo daily-poweroff resume 2026-05-26
  sudo daily-poweroff set-language zh-CN

不指定具体日期或 --from 时，cancel/resume 会从下一次计划关机日期开始。
"#
    );
}

fn cmd_set(config_path: &Path, args: &[String]) -> Result<(), String> {
    let time_arg = args
        .first()
        .ok_or_else(|| "set requires a time like 17:30".to_string())?;
    let shutdown_time = parse_time(time_arg)?;
    let mut config = Config::load(config_path)?;
    config.enabled = true;
    config.shutdown_time = Some(shutdown_time);

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--warning-minutes" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| {
                    "--warning-minutes requires a comma-separated list".to_string()
                })?;
                config.warning_seconds = parse_warning_minutes(value)?;
            }
            "--dry-run" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--dry-run requires true or false".to_string())?;
                config.dry_run = parse_bool(value)?;
            }
            other => return Err(format!("unknown set option: {other}")),
        }
        i += 1;
    }

    config.save(config_path)?;
    println!(
        "{}",
        message_set_time(config.language, format_time(shutdown_time))
    );
    Ok(())
}

fn cmd_cancel(config_path: &Path, args: &[String], cancel: bool) -> Result<(), String> {
    let now = local_now()?;
    let mut from: Option<String> = None;
    let mut days: Option<i64> = None;
    let mut dates = Vec::new();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--days" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--days requires a positive integer".to_string())?;
                let parsed = value
                    .parse::<i64>()
                    .map_err(|_| "--days must be a positive integer".to_string())?;
                if parsed < 1 {
                    return Err("--days must be at least 1".to_string());
                }
                days = Some(parsed);
            }
            "--from" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--from requires YYYY-MM-DD".to_string())?;
                parse_date(value)?;
                from = Some(value.clone());
            }
            value => {
                parse_date(value)?;
                dates.push(value.to_string());
            }
        }
        i += 1;
    }

    let mut config = Config::load(config_path)?;

    if !dates.is_empty() && (days.is_some() || from.is_some()) {
        return Err("use explicit dates, or use --days/--from; do not combine them".to_string());
    }

    if dates.is_empty() {
        let count = days.unwrap_or(1);
        let from = match from {
            Some(from) => from,
            None => default_action_start_date(&config, &now)?,
        };
        for offset in 0..count {
            dates.push(add_days(&from, offset)?);
        }
    }

    for date in &dates {
        if cancel {
            config.canceled_dates.insert(date.clone());
        } else {
            config.canceled_dates.remove(date);
        }
    }
    config.save(config_path)?;

    println!(
        "{}",
        message_action_dates(config.language, cancel, &dates.join(", "))
    );
    Ok(())
}

fn default_action_start_date(config: &Config, now: &LocalNow) -> Result<String, String> {
    let Some(shutdown_time) = config.shutdown_time else {
        return Ok(now.date.clone());
    };
    let target_epoch = epoch_for_date_time(&now.date, shutdown_time)?;
    if now.epoch > target_epoch {
        add_days(&now.date, 1)
    } else {
        Ok(now.date.clone())
    }
}

fn cmd_status(config_path: &Path) -> Result<(), String> {
    let config = Config::load(config_path)?;
    let language = config.language;
    let now = local_now()?;
    println!(
        "{}: {}",
        status_label(language, "config"),
        config_path.display()
    );
    println!("{}: {}", status_label(language, "enabled"), config.enabled);
    println!(
        "{}: {}",
        status_label(language, "time"),
        config
            .shutdown_time
            .map(format_time)
            .unwrap_or_else(|| status_label(language, "not_set").to_string())
    );
    println!(
        "{}: {} {}",
        status_label(language, "now"),
        now.date,
        now.time
    );
    println!("{}: {}", status_label(language, "dry_run"), config.dry_run);
    println!(
        "{}: {}",
        status_label(language, "shutdown_command"),
        config.shutdown_command
    );
    println!(
        "{}: {}",
        status_label(language, "language"),
        format_language(language)
    );
    println!(
        "{}: {}",
        status_label(language, "warning_minutes"),
        config
            .warning_seconds
            .iter()
            .map(|seconds| {
                if seconds % 60 == 0 {
                    (seconds / 60).to_string()
                } else {
                    format!("{seconds}s")
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    println!(
        "{}: {}",
        status_label(language, "cancelled_dates"),
        if config.canceled_dates.is_empty() {
            status_label(language, "none").to_string()
        } else {
            config
                .canceled_dates
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    if let Some(next) = next_scheduled_date(&config, &now)? {
        println!(
            "{}: {next} {}",
            status_label(language, "next_shutdown"),
            format_time(config.shutdown_time.unwrap())
        );
    } else {
        println!(
            "{}: {}",
            status_label(language, "next_shutdown"),
            status_label(language, "none")
        );
    }
    Ok(())
}

fn cmd_enable(config_path: &Path, enabled: bool) -> Result<(), String> {
    let mut config = Config::load(config_path)?;
    config.enabled = enabled;
    config.save(config_path)?;
    println!("{}", message_enabled(config.language, enabled));
    Ok(())
}

fn cmd_set_language(config_path: &Path, args: &[String]) -> Result<(), String> {
    let value = args
        .first()
        .ok_or_else(|| "set-language requires en or zh-CN".to_string())?;
    if args.len() > 1 {
        return Err("set-language accepts exactly one value: en or zh-CN".to_string());
    }
    let mut config = Config::load(config_path)?;
    config.language = parse_language(value)?;
    config.save(config_path)?;
    println!("{}", message_language_set(config.language));
    Ok(())
}

fn daemon(config_path: &Path) -> Result<(), String> {
    println!(
        "daily-poweroff daemon started, config={}",
        config_path.display()
    );
    let mut sent: HashSet<(String, i64)> = HashSet::new();
    let mut shutdown_sent_for: HashSet<String> = HashSet::new();

    loop {
        let config = match Config::load(config_path) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("failed to load config: {err}");
                sleep_secs(60);
                continue;
            }
        };

        let now = match local_now() {
            Ok(now) => now,
            Err(err) => {
                eprintln!("failed to read local time: {err}");
                sleep_secs(60);
                continue;
            }
        };

        if !config.enabled || config.shutdown_time.is_none() {
            sleep_secs(60);
            continue;
        }

        let shutdown_time = config.shutdown_time.unwrap();
        let target_epoch = match epoch_for_date_time(&now.date, shutdown_time) {
            Ok(epoch) => epoch,
            Err(err) => {
                eprintln!("failed to compute shutdown time: {err}");
                sleep_secs(60);
                continue;
            }
        };

        if now.epoch > target_epoch + 60 {
            sleep_secs(60);
            continue;
        }

        if config.canceled_dates.contains(&now.date) {
            sleep_secs(60);
            continue;
        }

        let remaining = target_epoch - now.epoch;
        if remaining <= 0 {
            if shutdown_sent_for.insert(now.date.clone()) {
                let message =
                    message_poweroff_now(config.language, &now.date, format_time(shutdown_time));
                broadcast(&message);
                run_shutdown(&config)?;
            }
            sleep_secs(60);
            continue;
        }

        if let Some(threshold) =
            next_warning_threshold(&config.warning_seconds, remaining, &sent, &now.date)
        {
            let message = message_poweroff_warning(
                config.language,
                remaining,
                &now.date,
                format_time(shutdown_time),
            );
            broadcast(&message);
            for warning in config.warning_seconds.iter().copied() {
                if warning >= threshold {
                    sent.insert((now.date.clone(), warning));
                }
            }
        }

        sleep_secs(sleep_interval(remaining));
    }
}

fn next_warning_threshold(
    warnings: &[i64],
    remaining: i64,
    sent: &HashSet<(String, i64)>,
    date: &str,
) -> Option<i64> {
    let mut sorted = warnings.to_vec();
    sorted.sort_unstable();
    sorted
        .into_iter()
        .find(|warning| remaining <= *warning && !sent.contains(&(date.to_string(), *warning)))
}

fn sleep_interval(remaining: i64) -> u64 {
    if remaining <= 60 {
        5
    } else if remaining <= 300 {
        15
    } else {
        30
    }
}

fn sleep_secs(seconds: u64) {
    thread::sleep(Duration::from_secs(seconds));
}

fn run_shutdown(config: &Config) -> Result<(), String> {
    if config.dry_run {
        println!(
            "dry_run=true, skipped shutdown command: {}",
            config.shutdown_command
        );
        return Ok(());
    }

    let mut parts = config.shutdown_command.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| "shutdown_command is empty".to_string())?;
    let args: Vec<&str> = parts.collect();
    Command::new(program)
        .args(args)
        .status()
        .map_err(|err| format!("failed to execute shutdown command: {err}"))?;
    Ok(())
}

fn broadcast(message: &str) {
    println!("{message}");
    broadcast_wall(message);
}

fn broadcast_wall(message: &str) {
    let _ = Command::new("wall").arg("-n").arg(message).status();
}

fn cmd_install_systemd(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("install-systemd does not take arguments".to_string());
    }
    let exe =
        env::current_exe().map_err(|err| format!("failed to locate current executable: {err}"))?;
    let system_bin = Path::new(SYSTEM_BIN_PATH);
    fs::copy(&exe, system_bin).map_err(|err| {
        format!(
            "failed to install {} to {}: {err}. Run this command with sudo.",
            exe.display(),
            system_bin.display()
        )
    })?;
    fs::set_permissions(system_bin, fs::Permissions::from_mode(0o755)).map_err(|err| {
        format!(
            "failed to set executable permission on {}: {err}",
            system_bin.display()
        )
    })?;

    let config_path = Path::new(DEFAULT_CONFIG_PATH);
    Config::load(config_path)?.save(config_path)?;

    let unit = systemd_unit();
    let service_path = Path::new(SYSTEMD_SERVICE_PATH);
    fs::write(service_path, unit).map_err(|err| {
        format!(
            "failed to write {}: {err}. Run this command with sudo.",
            service_path.display()
        )
    })?;

    run_systemctl(&["daemon-reload"])?;
    run_systemctl(&["enable", SYSTEMD_SERVICE_NAME])?;
    run_systemctl(&["restart", SYSTEMD_SERVICE_NAME])?;
    println!(
        "installed {} and restarted {}",
        system_bin.display(),
        SYSTEMD_SERVICE_NAME
    );
    Ok(())
}

fn cmd_print_systemd() -> Result<(), String> {
    print!("{}", systemd_unit());
    Ok(())
}

fn run_systemctl(args: &[&str]) -> Result<(), String> {
    let status = Command::new("systemctl")
        .args(args)
        .status()
        .map_err(|err| format!("failed to run systemctl {}: {err}", args.join(" ")))?;
    if !status.success() {
        return Err(format!(
            "systemctl {} failed with status {}",
            args.join(" "),
            status
        ));
    }
    Ok(())
}

fn systemd_unit() -> String {
    format!(
        r#"[Unit]
Description=Daily Poweroff daemon
After=multi-user.target

[Service]
Type=simple
ExecStart={} daemon
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        SYSTEM_BIN_PATH
    )
}

fn next_scheduled_date(config: &Config, now: &LocalNow) -> Result<Option<String>, String> {
    if !config.enabled || config.shutdown_time.is_none() {
        return Ok(None);
    }
    let shutdown_time = config.shutdown_time.unwrap();
    for offset in 0..366 {
        let date = add_days(&now.date, offset)?;
        if offset == 0 {
            let target_epoch = epoch_for_date_time(&date, shutdown_time)?;
            if now.epoch > target_epoch + 60 {
                continue;
            }
        }
        if !config.canceled_dates.contains(&date) {
            return Ok(Some(date));
        }
    }
    Ok(None)
}

impl Config {
    fn default() -> Self {
        Self {
            enabled: true,
            shutdown_time: None,
            canceled_dates: BTreeSet::new(),
            warning_seconds: DEFAULT_WARNING_SECONDS.to_vec(),
            shutdown_command: DEFAULT_SHUTDOWN_COMMAND.to_string(),
            dry_run: false,
            language: Language::En,
        }
    }

    fn load(path: &Path) -> Result<Self, String> {
        let mut config = Self::default();
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(config),
            Err(err) => return Err(format!("failed to read {}: {err}", path.display())),
        };

        for (line_number, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("invalid config line {}: {line}", line_number + 1));
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "enabled" => config.enabled = parse_bool(value)?,
                "time" => {
                    config.shutdown_time = if value.is_empty() {
                        None
                    } else {
                        Some(parse_time(value)?)
                    };
                }
                "canceled_dates" => {
                    config.canceled_dates.clear();
                    for date in comma_values(value) {
                        parse_date(&date)?;
                        config.canceled_dates.insert(date);
                    }
                }
                "warning_seconds" => {
                    config.warning_seconds = parse_warning_seconds(value)?;
                }
                "shutdown_command" => config.shutdown_command = value.to_string(),
                "dry_run" => config.dry_run = parse_bool(value)?,
                "language" => config.language = parse_language(value)?,
                _ => {
                    return Err(format!(
                        "unknown config key on line {}: {key}",
                        line_number + 1
                    ));
                }
            }
        }
        normalize_warnings(&mut config.warning_seconds)?;
        Ok(config)
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
        let time = self
            .shutdown_time
            .map(format_time)
            .unwrap_or_else(|| "".to_string());
        let canceled_dates = self
            .canceled_dates
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let warning_seconds = self
            .warning_seconds
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let content = format!(
            "# Managed by daily-poweroff. Edit carefully, or use the CLI.\n\
enabled={}\n\
time={}\n\
canceled_dates={}\n\
warning_seconds={}\n\
shutdown_command={}\n\
dry_run={}\n\
language={}\n",
            self.enabled,
            time,
            canceled_dates,
            warning_seconds,
            self.shutdown_command,
            self.dry_run,
            format_language(self.language)
        );
        fs::write(path, content).map_err(|err| {
            format!(
                "failed to write {}: {err}. Run this command with sudo or use --config.",
                path.display()
            )
        })
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(format!("invalid boolean: {value}")),
    }
}

fn parse_language(value: &str) -> Result<Language, String> {
    match value {
        "en" | "en-US" => Ok(Language::En),
        "zh-CN" | "zh_cn" | "zh" => Ok(Language::ZhCn),
        _ => Err(format!("invalid language: {value}; expected en or zh-CN")),
    }
}

fn format_language(language: Language) -> &'static str {
    match language {
        Language::En => "en",
        Language::ZhCn => "zh-CN",
    }
}

fn parse_time(value: &str) -> Result<TimeOfDay, String> {
    let Some((hour, minute)) = value.split_once(':') else {
        return Err(format!("invalid time: {value}; expected HH:MM"));
    };
    let hour = hour
        .parse::<u32>()
        .map_err(|_| format!("invalid hour in time: {value}"))?;
    let minute = minute
        .parse::<u32>()
        .map_err(|_| format!("invalid minute in time: {value}"))?;
    if hour > 23 || minute > 59 {
        return Err(format!(
            "invalid time: {value}; expected 00:00 through 23:59"
        ));
    }
    Ok(TimeOfDay { hour, minute })
}

fn format_time(time: TimeOfDay) -> String {
    format!("{:02}:{:02}", time.hour, time.minute)
}

fn parse_date(value: &str) -> Result<(i32, u32, u32), String> {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("invalid date: {value}; expected YYYY-MM-DD"));
    }
    let year = parts[0]
        .parse::<i32>()
        .map_err(|_| format!("invalid date year: {value}"))?;
    let month = parts[1]
        .parse::<u32>()
        .map_err(|_| format!("invalid date month: {value}"))?;
    let day = parts[2]
        .parse::<u32>()
        .map_err(|_| format!("invalid date day: {value}"))?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!("invalid date: {value}"));
    }
    Ok((year, month, day))
}

fn parse_warning_minutes(value: &str) -> Result<Vec<i64>, String> {
    let mut seconds = Vec::new();
    for part in comma_values(value) {
        let minutes = part
            .parse::<i64>()
            .map_err(|_| format!("invalid warning minute: {part}"))?;
        if minutes < 1 {
            return Err("warning minutes must be at least 1".to_string());
        }
        seconds.push(minutes * 60);
    }
    normalize_warnings(&mut seconds)?;
    Ok(seconds)
}

fn parse_warning_seconds(value: &str) -> Result<Vec<i64>, String> {
    let mut seconds = Vec::new();
    for part in comma_values(value) {
        let parsed = part
            .parse::<i64>()
            .map_err(|_| format!("invalid warning second: {part}"))?;
        if parsed < 1 {
            return Err("warning seconds must be at least 1".to_string());
        }
        seconds.push(parsed);
    }
    normalize_warnings(&mut seconds)?;
    Ok(seconds)
}

fn normalize_warnings(warnings: &mut Vec<i64>) -> Result<(), String> {
    if warnings.is_empty() {
        return Err("warning list cannot be empty".to_string());
    }
    warnings.sort_unstable();
    warnings.dedup();
    warnings.reverse();
    Ok(())
}

fn comma_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn local_now() -> Result<LocalNow, String> {
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock is before UNIX epoch: {err}"))?
        .as_secs() as i64;
    let tm = local_tm(epoch)?;
    Ok(LocalNow {
        epoch,
        date: format!(
            "{:04}-{:02}-{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday
        ),
        time: format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec),
    })
}

fn local_tm(epoch: i64) -> Result<Tm, String> {
    let mut tm = Tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    let raw = epoch as c_long;
    let result = unsafe { localtime_r(&raw as *const c_long, &mut tm as *mut Tm) };
    if result.is_null() {
        return Err("localtime_r failed".to_string());
    }
    Ok(tm)
}

fn epoch_for_date_time(date: &str, time: TimeOfDay) -> Result<i64, String> {
    let (year, month, day) = parse_date(date)?;
    let mut tm = Tm {
        tm_sec: 0,
        tm_min: time.minute as c_int,
        tm_hour: time.hour as c_int,
        tm_mday: day as c_int,
        tm_mon: month as c_int - 1,
        tm_year: year as c_int - 1900,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: -1,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    let epoch = unsafe { mktime(&mut tm as *mut Tm) };
    if epoch < 0 {
        return Err(format!(
            "failed to convert local time to epoch: {date} {}",
            format_time(time)
        ));
    }
    Ok(epoch as i64)
}

fn add_days(date: &str, days: i64) -> Result<String, String> {
    let noon = TimeOfDay {
        hour: 12,
        minute: 0,
    };
    let epoch = epoch_for_date_time(date, noon)? + days * 86_400;
    let tm = local_tm(epoch)?;
    Ok(format!(
        "{:04}-{:02}-{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday
    ))
}

fn message_test_broadcast(language: Language) -> &'static str {
    match language {
        Language::En => "daily-poweroff test broadcast: TTY broadcast is working.",
        Language::ZhCn => "daily-poweroff 测试广播：TTY 广播通道可用。",
    }
}

fn message_set_time(language: Language, time: String) -> String {
    match language {
        Language::En => format!("set daily poweroff time to {time}"),
        Language::ZhCn => format!("已设置每日关机时间为 {time}"),
    }
}

fn message_action_dates(language: Language, cancel: bool, dates: &str) -> String {
    match (language, cancel) {
        (Language::En, true) => format!("cancelled: {dates}"),
        (Language::En, false) => format!("resumed: {dates}"),
        (Language::ZhCn, true) => format!("已取消：{dates}"),
        (Language::ZhCn, false) => format!("已恢复：{dates}"),
    }
}

fn message_enabled(language: Language, enabled: bool) -> &'static str {
    match (language, enabled) {
        (Language::En, true) => "enabled",
        (Language::En, false) => "disabled",
        (Language::ZhCn, true) => "已启用",
        (Language::ZhCn, false) => "已停用",
    }
}

fn message_language_set(language: Language) -> &'static str {
    match language {
        Language::En => "language set to en",
        Language::ZhCn => "语言已设置为 zh-CN",
    }
}

fn message_poweroff_now(language: Language, date: &str, time: String) -> String {
    match language {
        Language::En => format!("Powering off now\nScheduled: {date} {time}"),
        Language::ZhCn => format!("现在关机\n计划时间：{date} {time}"),
    }
}

fn message_poweroff_warning(
    language: Language,
    remaining: i64,
    date: &str,
    time: String,
) -> String {
    match language {
        Language::En => format!(
            "Poweroff in {}\nScheduled: {date} {time}\nCancel: sudo daily-poweroff cancel",
            human_remaining(remaining, language)
        ),
        Language::ZhCn => format!(
            "{} 后关机\n计划时间：{date} {time}\n取消：sudo daily-poweroff cancel",
            human_remaining(remaining, language)
        ),
    }
}

fn status_label(language: Language, key: &str) -> &'static str {
    match (language, key) {
        (Language::En, "config") => "config",
        (Language::En, "enabled") => "enabled",
        (Language::En, "time") => "time",
        (Language::En, "now") => "now",
        (Language::En, "dry_run") => "dry_run",
        (Language::En, "shutdown_command") => "shutdown_command",
        (Language::En, "language") => "language",
        (Language::En, "warning_minutes") => "warning_minutes",
        (Language::En, "cancelled_dates") => "cancelled_dates",
        (Language::En, "next_shutdown") => "next_shutdown",
        (Language::En, "not_set") => "not set",
        (Language::En, "none") => "none",
        (Language::ZhCn, "config") => "配置",
        (Language::ZhCn, "enabled") => "启用",
        (Language::ZhCn, "time") => "时间",
        (Language::ZhCn, "now") => "当前时间",
        (Language::ZhCn, "dry_run") => "试运行",
        (Language::ZhCn, "shutdown_command") => "关机命令",
        (Language::ZhCn, "language") => "语言",
        (Language::ZhCn, "warning_minutes") => "提醒时间",
        (Language::ZhCn, "cancelled_dates") => "已取消日期",
        (Language::ZhCn, "next_shutdown") => "下一次关机",
        (Language::ZhCn, "not_set") => "未设置",
        (Language::ZhCn, "none") => "无",
        _ => "unknown",
    }
}

fn human_remaining(seconds: i64, language: Language) -> String {
    if seconds >= 3600 {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        match language {
            Language::En => {
                if minutes == 0 {
                    format!("{hours} hour{}", plural(hours))
                } else {
                    format!(
                        "{hours} hour{} {minutes} minute{}",
                        plural(hours),
                        plural(minutes)
                    )
                }
            }
            Language::ZhCn => {
                if minutes == 0 {
                    format!("{hours}小时")
                } else {
                    format!("{hours}小时{minutes}分钟")
                }
            }
        }
    } else if seconds >= 60 {
        let minutes = (seconds + 59) / 60;
        match language {
            Language::En => format!("{minutes} minute{}", plural(minutes)),
            Language::ZhCn => format!("{minutes}分钟"),
        }
    } else {
        match language {
            Language::En => format!("{seconds} second{}", plural(seconds)),
            Language::ZhCn => format!("{seconds}秒"),
        }
    }
}

fn plural(value: i64) -> &'static str {
    if value == 1 { "" } else { "s" }
}
