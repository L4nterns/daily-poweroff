# daily-poweroff

[中文说明](README.zh-CN.md)

A single-binary Linux daily poweroff scheduler. The same executable provides configuration, cancellation, status inspection, systemd installation, and the long-running daemon.

## Features

- Set a daily automatic poweroff time, for example `17:30`
- Broadcast warnings before poweroff: by default 60, 30, 15, 10, 5, 3, 2, 1 minutes, plus 30 and 10 seconds
- Broadcast to logged-in terminals/TTYs with `wall -n`
- Cancel or resume upcoming scheduled poweroff dates
- Dry-run mode for testing without powering off the machine

## Install From GitHub Releases

Download a prebuilt binary from the project releases page:

```sh
VERSION=v0.1.0
ARCH=x86_64-unknown-linux-gnu
curl -L -o daily-poweroff \
  "https://github.com/L4nterns/daily-poweroff/releases/download/${VERSION}/daily-poweroff-${ARCH}"
chmod +x daily-poweroff
sudo install -m 0755 daily-poweroff /usr/local/bin/daily-poweroff
sudo daily-poweroff install-systemd
```

Replace `VERSION` and `ARCH` with the release you want to install.

## Build From Source

You can also build the binary locally:

```sh
cargo build --release
sudo install -m 0755 target/release/daily-poweroff /usr/local/bin/daily-poweroff
sudo daily-poweroff install-systemd
```

The single binary is generated at:

```sh
target/release/daily-poweroff
```

`install-systemd` writes `/etc/systemd/system/daily-poweroff.service`, then runs `systemctl daemon-reload` and `systemctl enable --now daily-poweroff.service`.

This default installation targets Linux systems with systemd and runs the daemon as root.

## Usage

Commands that modify the default `/etc/daily-poweroff.conf` need `sudo`. Read-only commands such as `status` do not. If you pass `--config` with a file your user can write, `sudo` is not needed for configuration tests.

Set daily poweroff at 17:30:

```sh
sudo daily-poweroff set 17:30
```

Show status:

```sh
daily-poweroff status
```

Cancel the next scheduled date:

```sh
sudo daily-poweroff cancel
```

Cancel the next 3 scheduled dates:

```sh
sudo daily-poweroff cancel --days 3
```

Without explicit dates or `--from`, `cancel` and `resume` start from the next scheduled poweroff date. Before today's poweroff time that means today; after today's poweroff time that means tomorrow.

Cancel explicit dates:

```sh
sudo daily-poweroff cancel 2026-05-25 2026-05-26
```

Cancel 3 days starting from a given date:

```sh
sudo daily-poweroff cancel --from 2026-05-25 --days 3
```

Resume the next scheduled date:

```sh
sudo daily-poweroff resume
```

Resume the next 3 scheduled dates:

```sh
sudo daily-poweroff resume --days 3
```

Resume explicit dates:

```sh
sudo daily-poweroff resume 2026-05-25 2026-05-26
```

Resume 3 days starting from a given date:

```sh
sudo daily-poweroff resume --from 2026-05-25 --days 3
```

Temporarily disable or enable scheduling:

```sh
sudo daily-poweroff disable
sudo daily-poweroff enable
```

Set output and broadcast language:

```sh
sudo daily-poweroff set-language zh-CN
sudo daily-poweroff set-language en
```

Test terminal/TTY broadcast:

```sh
sudo daily-poweroff test-broadcast
```

## Configuration

Default config file:

```text
/etc/daily-poweroff.conf
```

Commands load the selected config file first. If it does not exist, built-in defaults are used, then command-line options override the relevant fields and the result is written back.

Example:

```ini
enabled=true
time=17:30
canceled_dates=2026-05-25,2026-05-26
warning_seconds=3600,1800,900,600,300,180,120,60,30,10
shutdown_command=systemctl poweroff
dry_run=false
language=en
```

`language` defaults to `en` and also supports `zh-CN`.

Use a non-system config path for tests:

```sh
daily-poweroff --config /tmp/daily-poweroff.conf set 17:30 --dry-run true
daily-poweroff --config /tmp/daily-poweroff.conf status
```
