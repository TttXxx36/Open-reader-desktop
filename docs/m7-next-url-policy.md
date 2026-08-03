# M7.2 next URL 自动追链策略（评估稿）

状态：评估中，运行时默认关闭。当前版本只在正文结果中保留一个受限 \`next_url\`，阅读器提示“不会自动追链”。

## 目标与非目标

目标是为常见的“正文分页/下一页”书源提供可取消、可诊断、可限额的可选追链能力。

非目标：

- 不执行 XPath、JavaScript、模板脚本或页面内任意代码。
- 不把 next URL 转换为 CSS 规则，也不接受 \`||\` 回退链。
- 不允许没有预算的递归请求，不把跨域跳转默认为可信。
- 不在用户未明确启用时改变现有阅读行为。

## 拟定默认配额

| 配额 | 建议值 | 说明 |
| --- | ---: | --- |
| 自动追链开关 | false | 保持现有行为兼容；首轮只允许单书源显式启用 |
| 最大深度 | 2 | 初始正文为 depth 0，最多再取两页 |
| 最大页面数 | 3 | 含首个正文页；与深度同时取较小值 |
| 单响应体 | 现有 body 上限 | 复用 \`SourceEngine.max_body_bytes\`，不另设无限缓冲 |
| 总响应体 | 2 MiB | 计入正文链所有响应，超出立即停止 |
| 总耗时 | 15 秒 | 独立于现有单阶段 60 秒和 pipeline 120 秒预算，取更小值 |
| 主机策略 | 同源 | scheme、host、有效端口必须与首个正文 URL 一致 |
| 取消检查 | 每页请求前、响应头后、正文读取后 | 复用端到端/远端/多源取消 token |
| 诊断上限 | 8 个步骤 | 每一页记录命中 URL、深度、耗时、状态、字节数和失败原因 |

所有上限都必须在服务端再次校验，不能依赖前端传入值。next URL 仍须通过当前 2 KiB、HTTP(S)、单链接校验。

## 已实现的纯策略闸门

后端已加入 NextPagePolicy 与 evaluate_next_page_policy 纯函数：

- 默认 enabled=false；建议的深度、页数、总字节和总耗时会在服务端再次裁剪，避免调用方传入无限配额。
- 只做候选 URL 的单链接/HTTP(S) 校验、同源比较、深度/页数/字节/时间预算、访问环路判断和剩余预算计算。
- 返回稳定 reason（disabled、depth_limit、page_limit、byte_limit、time_limit、same_origin、cycle 等），不发起请求、不改变当前 next URL 中间结果。
- 远程证据：[GitHub Actions run 30771892118](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30771892118)（策略闸门，59 个 Rust 测试、前端检查通过）。Stop-reason 矩阵又覆盖页数、字节、时间、非法候选/基准、零配额和无限输入裁剪；[run 30772265421](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30772265421)（60 个 Rust 测试、前端检查通过）。累计多页夹具验证页数/时间预算不会重置，深度优先级固定；[run 30772819136](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30772819136)（61 个 Rust 测试、前端检查通过）。

## 已实现的显式 opt-in 请求夹具

`SourceEngine::fetch_chapter_content_with_policy` 已在后端提供显式 opt-in 的受限请求链：只有 `NextPagePolicy.enabled=true` 才会发起后续页请求；默认的 `fetch_chapter_content` 单页路径和现有 Tauri 命令保持不变。合成 HTTP 夹具覆盖首屏→第二页→第三页，断言正文合并、`content.next.depth-1/2` 诊断步骤、访问上限和 `next_url` 清空。

远程证据：[GitHub Actions run 30773702655](https://github.com/TttXxx36/Open-reader-desktop/actions/runs/30773702655)（前端检查、Rust fmt/check、62 个 Rust 测试通过）。该证据只证明受限 opt-in 链路可运行，不代表已把自动追链接入默认阅读流程。

## 失败与回退语义

- 首页成功、后续页失败：保留已取得正文，返回“部分追链”状态和脱敏失败步骤，不回退到旧缓存覆盖新正文。
- 超过深度、页数、响应体或耗时：停止继续请求，保留已取得正文，记录明确的 quota reason。
- 用户取消：立即停止后续请求，返回取消状态；不把取消误记为网络失败，也不触发 stale 回退。
- 同源校验失败：保留 next URL 供诊断，但不发起请求。
- 任一页解析不到正文：停止链路；不尝试把页面当作 CSS/JSONPath 规则重新解释。
- 去重：按脱敏后的绝对 URL 去重，发现环路立即停止。

## 诊断与兼容性

每个追链步骤使用稳定的 stage 名称（例如 \`content.next.depth-1\`），并加入：

- depth、page_index、quota_remaining；
- candidate URL 的脱敏形式；
- request/parse/cache 的结果与累计 start_ms；
- stop_reason（success、cancelled、same_origin、depth_limit、page_limit、byte_limit、time_limit、cycle、parse_error、request_error）。

快照 schema 继续保持向后兼容：新增字段使用可选键；旧快照读取时缺省为“未追链”。

## 进入实现的门槛

1. 已用合成 HTTP 夹具验证成功的三页 opt-in 链；仍需补齐环路、跨源、超时、超体积、取消和部分成功夹具。
2. Rust 单元/集成测试覆盖所有 stop_reason，并证明总响应体与总耗时预算不会被单页重置。
3. 前端明确显示“自动追链已关闭/已启用、当前深度和停止原因”，导出诊断不包含正文、Cookie 或认证头。
4. 兼容性矩阵区分“保留 next URL”“可选自动追链”“默认关闭”；Windows 手工验收确认取消按钮和阅读进度不回退。
5. 未满足以上门槛前，保持当前不自动追链行为，不修改默认设置。

