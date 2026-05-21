# Changelog

本文件记录仓库内已提交但未发版的主要变更，以及后续发布版本的变更摘要。

维护规则：
- 普通提交写入 `## [Unreleased]`。
- 发版时从 `Unreleased` 收束为 `## [vX.Y.Z] - YYYY-MM-DD`。
- 版本号遵循：大改动升主版本，新功能升次版本，修复或小调整升修订版本。
- 内容按影响描述，不逐行罗列 diff。

## [Unreleased]

### Changed

- 新增 Web 行情首页和 WebSocket K 线历史请求能力，腾讯数据源支持分钟/日 K 线拉取与解析。
- 使用 SeaORM 替换手写 SQLx 落库实现，新增通用 ORM 存储、实体定义和数据库 URL 配置兼容。
- 同步启动日志和实现计划中的项目名为 `fast-quote`。

### Added

- 新增运维脚本：`start_fastquote.sh`（启动服务，自动构建、pid 管理、日志重定向）、`stop_fastquote.sh`（优雅停止，SIGTERM → SIGKILL 退避）、`install_fastquote_cron.sh`（安装工作日定时启停 cron）。
- 新增运维文档 `docs/fastquote-operations.md` 及 cron 设计文档。

### Fixed

- SQLite 存储连接时自动创建缺失的数据库文件与父目录，新增 `sqlite_store_creates_missing_database_file` 测试覆盖。
- `.gitignore` 补充排除 `/logs`、`/run`、`/data/*.db` 等运行时产物。
