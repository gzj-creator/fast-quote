# Changelog

本文件记录仓库内已提交但未发版的主要变更，以及后续发布版本的变更摘要。

维护规则：
- 普通提交写入 `## [Unreleased]`。
- 发版时从 `Unreleased` 收束为 `## [vX.Y.Z] - YYYY-MM-DD`。
- 版本号遵循：大改动升主版本，新功能升次版本，修复或小调整升修订版本。
- 内容按影响描述，不逐行罗列 diff。

## [Unreleased]

### Changed

- 使用 SeaORM 替换手写 SQLx 落库实现，新增通用 ORM 存储、实体定义和数据库 URL 配置兼容。
- 同步启动日志和实现计划中的项目名为 `fast-quote`。
