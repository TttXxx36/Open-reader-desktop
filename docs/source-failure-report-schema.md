# 本地书源失败报告格式

本文件定义书源页“导出报告”生成的 JSON 约定。报告只用于用户本地诊断，不上传远程服务。

## 当前版本

- `schema_version`: `1`
- `report_type`: `source_failure_history`
- 文件名：`open-reader-source-failures-YYYY-MM-DD.json`
- 大小上限：256 KiB
- 当前导出最多包含 64 条失败记录、32 个原因统计项和 32 个阶段统计项。

顶层字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `schema_version` | number | 格式版本；当前为 1。 |
| `report_type` | string | 固定为 `source_failure_history`。 |
| `generated_at` | string | 本地生成时间，ISO 8601。 |
| `scope` | string | `current_source` 或 `all_sources`。 |
| `source_id` | string/null | 当前书源筛选范围；全量报告为 null。 |
| `source_metrics` | object | 启用源、审计摘要、失败记录数和缓存占用的非敏感汇总。 |
| `stats` | object/null | 总量、原因和阶段计数。 |
| `entries` | array | 脱敏失败摘要。 |
| `truncated_entries` | boolean | 是否因条数上限截断。 |
| `privacy` | string[] | 报告的隐私边界说明。 |

`source_metrics` 包含书源/审计/缓存摘要，以及可选的 `request_metrics`：`total_attempts`、`total_successes`、`total_failures`、`total_cache_hits`、`failure_rate`、`cache_hit_rate` 和按阶段统计。请求指标只统计已完成的网络请求；取消的请求不计入成功或失败。缓存命中率的分母是网络请求次数加缓存命中次数，失败率的分母是已完成网络请求次数；没有观测值时比例为 0，不代表真实网络成功率。

每条 `entries` 记录包含：

`id`、`source_id`、`source_name`、`stage`、`reason_code`、`operation_id`、`message`、`created_at`。其中 `operation_id` 可以为 null：这是 0009 迁移前的旧记录，读取和导出时按“无关联任务”处理。

请求指标由本机 SQLite 0010 迁移维护，按来源和阶段聚合；缓存命中只在读取未过期缓存时递增，stale 回退不计为命中。

## 兼容与迁移规则

1. 读取方必须先检查 `report_type` 和正整数 `schema_version`；未知版本只读元数据并提示升级，不猜测字段含义。
2. v1 到后续版本优先采用向后兼容的新增字段；旧字段不改语义、不复用名称。破坏性变更才提升主版本。
3. 缺少可选字段时使用安全默认值：`operation_id = null`、`stats = null`、`truncated_entries = false`。
4. 未知字段必须忽略，不能因为新增字段导致旧报告整体失败。
5. `message`、`source_name` 和 ID 展示前仍需截断/脱敏；不得把报告当作授权凭据或网络重放输入。
6. 报告不包含关键词、正文、Cookie、请求头、认证信息或未脱敏 URL；任何自动化处理都应保持这一边界。

数据库 0008 → 0009 的升级由应用启动时自动执行，旧失败记录的 `operation_id` 保持 null；GitHub Actions 夹具覆盖该升级路径。
