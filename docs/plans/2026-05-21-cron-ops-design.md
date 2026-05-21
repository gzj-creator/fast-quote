# Cron Operations Design

**Goal:** Run fastquote only during the regular market window and persist logs to disk without changing the Rust runtime.

**Approach:** Add operational shell scripts at the repository root. The start script runs the release binary from the repo root so the existing `config/config.toml` path still works, writes stdout/stderr to a dated file under `logs/`, and records a pid file under `run/`. The stop script reads the pid file, sends SIGTERM, waits briefly, and escalates only if the process does not exit.

**Scheduling:** A cron installer manages a clearly marked block in the current user's crontab:

- `30 9 * * 1-5` starts fastquote.
- `0 15 * * 1-5` stops fastquote.

This handles weekdays only. Exchange holidays are intentionally not modeled; the cron entries can be disabled or adjusted by operations when needed.

**Runtime artifacts:** Generated logs, pid files, and local SQLite database files are ignored by git.

**Verification:** Shell scripts are syntax-checked with `bash -n`. The start script is exercised with a harmless command override so it can prove pid/log behavior without opening network ports. The stop script is verified against that temporary process.
