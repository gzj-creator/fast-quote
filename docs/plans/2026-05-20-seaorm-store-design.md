# SeaORM Store Design

## Goal

All relational database persistence uses SeaORM instead of handwritten `sqlx`
queries. The storage layer is selected by the database URL, so SQLite can remain
the local/default backend while MySQL or PostgreSQL can be enabled by changing
configuration.

## Architecture

`fastquote-store` exposes an ORM-backed store for historical market data. The
store owns a SeaORM `DatabaseConnection`, initializes the required schema, and
writes `depth_tick`, `kline`, and `index_quote` rows through SeaORM entities and
active models.

The public write methods keep the current shape so `Persister` and existing
tests stay focused on persistence behavior:

- `write_depth(source, depth)`
- `write_kline(source, kline)`
- `write_index(source, index)`
- `count_rows(table)`

`Persister` should refer to this as a database store rather than a SQLite store.
Application config should prefer `store.database_url`, with `store.sqlite_url`
kept as a compatibility fallback during the transition.

## Data Model

The existing tables remain unchanged:

- `depth_tick`: primary key `(source, market, code, time_ns)`
- `kline`: primary key `(source, market, code, period, open_time_ns)`
- `index_quote`: primary key `(source, market, code, time_ns)`

JSON bid/ask fields remain text columns for cross-database compatibility.
Timestamp columns keep database-side current timestamp defaults where supported
by SeaQuery schema generation.

## Database Selection

SeaORM is configured with SQLite, MySQL, and PostgreSQL SQLx drivers. Runtime
selection is done by URL scheme:

- `sqlite://...`
- `mysql://...`
- `postgres://...` or `postgresql://...`

SQLite remains the automated test backend via `sqlite::memory:`.

## Testing

The migration is validated by behavior tests, not implementation tests:

- SQLite in-memory schema creation still works.
- Depth, kline, and index rows are persisted.
- Duplicate primary keys are upserted rather than duplicated.
- Config parsing accepts `database_url` and falls back to `sqlite_url`.
