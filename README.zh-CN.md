# daily-poweroff

[English README](README.md)

Linux 每日定时关机单程序。一个二进制同时提供配置、取消、状态查看、systemd 安装和守护进程。

## 功能

- 设置每日自动关机时间，例如 `17:30`
- 关机前广播提醒：默认 60、30、15、10、5、3、2、1 分钟，以及 30、10 秒
- 使用 `wall -n` 向已登录终端/TTY 广播
- 支持取消或恢复接下来的计划关机日期
- 支持 dry-run 测试，不真的关机

## 从 crates.io 安装

使用 Cargo 安装：

```sh
cargo install daily-poweroff
sudo daily-poweroff install-systemd
```

## 从 GitHub Releases 安装

从项目 Releases 页面下载预构建二进制：

```sh
VERSION=v0.1.0
ARCH=x86_64-unknown-linux-gnu
curl -L -o daily-poweroff \
  "https://github.com/L4nterns/daily-poweroff/releases/download/${VERSION}/daily-poweroff-${ARCH}"
chmod +x daily-poweroff
sudo install -m 0755 daily-poweroff /usr/local/bin/daily-poweroff
sudo daily-poweroff install-systemd
```

请把 `VERSION` 和 `ARCH` 替换成实际要安装的发布版本。

## 从源码构建

也可以在本地自行构建：

```sh
cargo build --release
sudo install -m 0755 target/release/daily-poweroff /usr/local/bin/daily-poweroff
sudo daily-poweroff install-systemd
```

生成的单程序在：

```sh
target/release/daily-poweroff
```

`install-systemd` 会写入 `/etc/systemd/system/daily-poweroff.service`，并执行 `systemctl daemon-reload` 和 `systemctl enable --now daily-poweroff.service`。

默认安装方式面向使用 systemd 的 Linux 系统，并以 root 运行守护进程。

## 使用

设置每日 17:30 关机：

```sh
sudo daily-poweroff set 17:30
```

查看状态：

```sh
daily-poweroff status
```

取消下一次计划关机日期：

```sh
sudo daily-poweroff cancel
```

取消接下来 3 个计划关机日期：

```sh
sudo daily-poweroff cancel --days 3
```

不指定具体日期或 `--from` 时，`cancel` 和 `resume` 都会从下一次计划关机日期开始计算。今天关机时间之前执行表示从今天开始，今天关机时间之后执行表示从明天开始。

取消指定日期：

```sh
sudo daily-poweroff cancel 2026-05-25 2026-05-26
```

取消从指定日期开始的 3 天：

```sh
sudo daily-poweroff cancel --from 2026-05-25 --days 3
```

恢复下一次计划关机日期：

```sh
sudo daily-poweroff resume
```

恢复接下来 3 个计划关机日期：

```sh
sudo daily-poweroff resume --days 3
```

恢复指定日期：

```sh
sudo daily-poweroff resume 2026-05-25 2026-05-26
```

恢复从指定日期开始的 3 天：

```sh
sudo daily-poweroff resume --from 2026-05-25 --days 3
```

临时停用/启用：

```sh
sudo daily-poweroff disable
sudo daily-poweroff enable
```

设置命令输出和广播语言：

```sh
sudo daily-poweroff set-language zh-CN
sudo daily-poweroff set-language en
```

测试终端/TTY 广播：

```sh
sudo daily-poweroff test-broadcast
```

## 配置

默认配置文件：

```text
/etc/daily-poweroff.conf
```

命令会先加载指定的配置文件。如果文件不存在，就使用内置默认配置；随后命令行参数会覆盖对应字段，并把结果写回配置文件。

示例：

```ini
enabled=true
time=17:30
canceled_dates=2026-05-25,2026-05-26
warning_seconds=3600,1800,900,600,300,180,120,60,30,10
shutdown_command=systemctl poweroff
dry_run=false
language=en
```

`language` 默认是 `en`，也支持 `zh-CN`。

测试时可以使用非系统路径：

```sh
daily-poweroff --config /tmp/daily-poweroff.conf set 17:30 --dry-run true
daily-poweroff --config /tmp/daily-poweroff.conf status
```
