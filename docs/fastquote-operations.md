# Fastquote Operations

## Manual Start

Build and start the service from the repository root:

```bash
scripts/start_fastquote.sh
```

The script runs `target/release/fastquote` from the repository root. If the release binary is missing, it runs:

```bash
cargo build --release -p fastquote-bin
```

## Manual Stop

```bash
scripts/stop_fastquote.sh
```

The stop script reads `run/fastquote.pid`, sends SIGTERM, waits up to 20 seconds, and sends SIGKILL only if the process does not exit.

## Logs

Application stdout and stderr are appended to a dated log file:

```text
logs/fastquote-YYYY-MM-DD.log
```

Cron command output is appended to:

```text
logs/fastquote-cron.log
```

## Cron Schedule

Install the managed cron entries:

```bash
scripts/install_fastquote_cron.sh
```

The installer replaces only the block between `# fastquote schedule begin` and `# fastquote schedule end`.

Installed schedule:

```cron
30 9 * * 1-5  start fastquote
0 15 * * 1-5  stop fastquote
```

This is a weekday schedule. It does not model exchange holidays.

View the installed crontab:

```bash
crontab -l
```

Remove the managed block manually if needed:

```bash
crontab -l | sed '/# fastquote schedule begin/,/# fastquote schedule end/d' | crontab -
```

## Runtime Files

Generated runtime files are ignored by git:

```text
logs/
run/
data/*.db
data/*.db-*
```
