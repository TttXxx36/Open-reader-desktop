# 书源规则执行指标边界

本文定义书源“规则执行成功率”的本地采集边界，解决网络请求成功、缓存命中与规则解析成功被混为一谈的问题。指标只用于用户本机诊断，不上传遥测，也不把它当作远程站点可用性或内容授权证明。

## 1. 与现有请求指标的边界

- `request_metrics` 只统计已完成的网络请求：请求成功、请求失败和新鲜缓存命中分别计数；取消请求不计入成功或失败，stale 回退不计为缓存命中。
- `rule_metrics` 只统计收到响应后真正执行的解析/提取规则。HTTP 错误、响应体超限、取消、同源/策略拒绝、未读取缓存和未配置规则都不进入规则分母。
- 两类指标必须分开显示：HTTP 200 不代表规则拿到了标题、目录或正文；规则失败也不应被解释为网络失败。

## 2. 统计单位与阶段

每个“规则评估尝试”对应一次响应体和一个阶段规则集的执行，而不是每个 HTML 节点或每个字符。这样可以避免搜索结果数量、分页数量和 DOM 大小改变指标分母。

阶段固定为：

| 阶段 | 规则输入 | 成功判定 |
| --- | --- | --- |
| `search` | `item`、`title`、`author`、`url` | 至少产出一条可用结果；结果标题或书籍 URL 至少有一项非空，`author` 缺失只记字段级 no-match |
| `book_info` | `title`、`author`、`intro`、`url` | 产出有效书籍标题；使用“未命名书籍”回退值不算规则成功 |
| `toc` | `item`、`title`、`url` | 至少产出一条同时具有标题和 URL 的章节 |
| `content` | `content`、可选 `next` | 正文提取并清洗后非空；`next` 缺失不影响正文成功 |

URL 回退链中只有首个成功响应会执行一次规则评估；失败候选没有响应体，不进入规则分母。分页或显式 opt-in 的 next URL 每个成功响应各算一次对应阶段评估。

缓存返回已经解析的结果时不重复执行规则，因此只增加缓存命中次数；若未来缓存原始响应体并重新解析，必须明确标记为新的规则评估，不能静默复用旧计数。

## 3. 结果状态与分母

每次规则评估只产生一个阶段级结果，同时可产生字段级辅助计数：

- `success`：达到阶段成功判定，且没有越过输出配额。
- `no_match`：规则语法有效、响应可解析，但没有得到所需值或结果为空。搜索无结果可能是合法查询结果，因此必须单独展示该计数。
- `failure`：规则语法、JSONPath/CSS/正则、响应解析或输出处理发生确定性错误。
- `skipped`：阶段或字段没有配置规则，或请求在响应体到达前被取消/拒绝；不进入分母。

阶段 `rule_attempts = success + no_match + failure`，`success_rate = success / rule_attempts`，`failure_rate = failure / rule_attempts`。没有观测时返回 `0` 并同时提供 `observed=false`，避免把“没有请求”误读成 0% 成功。

`no_match` 不从分母中剔除：它表示规则执行完成但没有产出，是定位失效选择器最重要的信号；界面必须同时展示 no-match 数量和说明，避免把搜索关键词没有命中误报为网络故障。

## 4. 聚合键、隐私与生命周期

聚合键为 `source_id + stage + rule_key`。`rule_key` 只允许固定字段名：`item`、`title`、`author`、`url`、`intro`、`content`、`next`、`replaceRules`；不得存储原始规则、关键词、正文、请求头、Cookie 或完整 URL。

建议的 SQLite 0011 表字段：

| 字段 | 说明 |
| --- | --- |
| `source_id` | 书源稳定 ID |
| `stage` | `search`、`book_info`、`toc`、`content` |
| `rule_key` | 上述固定字段名 |
| `attempts` | 已完成的规则评估次数 |
| `successes` | 阶段/字段成功次数 |
| `no_matches` | 语法有效但没有值的次数 |
| `failures` | 确定性规则错误次数 |
| `updated_at` | 最近更新时间 |

删除书源、replace-all 导入或恢复快照时一并删除对应聚合；旧数据库迁移失败不得删除原书源。指标表不做远程同步，不建立按关键词或正文的历史明细。

## 5. 报告与 UI 约定

- `source_metrics.request_metrics` 保持现有语义；新增可选 `rule_metrics`，旧报告读取方缺少该字段时按“未采集”处理。
- 书源页按阶段展示 attempts、success、no-match、failure、success rate；字段明细只展示固定 `rule_key`，不展示规则原文。
- 诊断报告明确写出两个分母：网络失败率的分母是已完成网络请求，规则成功率的分母是规则评估 attempts；取消、缓存和未配置规则的排除原因要保留在隐私/语义说明中。
- 所有指标继续本地导出；不新增自动上报、站点健康评分或跨用户排行榜。

## 6. 最小测试与验收

实现前先锁定以下授权合成夹具和纯函数测试：

1. 成功 pipeline：search、book_info、toc、content 各产生一次 success，规则指标与请求指标各自只增加一次。
2. 空搜索结果：产生 no-match，不产生网络 failure；success rate 与 no-match 数量同时可见。
3. 无效 CSS/JSONPath/正则：产生 failure；请求仍可单独记为 HTTP success，证明两类指标没有串账。
4. URL 回退链：失败候选不计规则 attempts，首个成功候选只计一次。
5. 分页/next URL：每个成功响应各计一次；策略拒绝、取消、超时和超体积不计规则 attempts。
6. 新鲜缓存命中与 stale 回退：都不计规则 attempts；分别保持现有 cache hit 和失败语义。
7. `source_id + stage + rule_key` 聚合、0011 升级、删除书源和 replace-all 回滚；旧报告缺少 `rule_metrics` 仍可读取。
8. 统计边界：零观测返回 `observed=false`；计数和比率不允许为负数，`success + no_matches + failures == attempts` 始终成立。

完成以上设计和夹具后，才进入 M7.5i 的代码切片；在此之前不把任何规则成功率写入现有请求指标，也不引入远程遥测。
