# 书源协议（M3）

M3 定义了一个稳定的内部书源模型，目标是让搜索、详情、目录和正文流程共享同一套数据结构。它兼容部分 Legado/阅读 JSON 命名，但不声称已经兼容其全部规则语法。

## 最小配置

```json
{
  "name": "Public HTML Fixture",
  "searchUrl": "https://example.test/search?q={{keyword}}",
  "bookInfoUrl": "https://example.test/book/{{bookId}}",
  "tocUrl": "https://example.test/book/{{bookId}}/toc",
  "contentUrl": "https://example.test/chapter/{{chapterId}}",
  "search": {
    "item": "article.book",
    "title": { "selector": "h2 a" },
    "author": { "selector": ".author" },
    "url": { "selector": "h2 a", "attr": "href" }
  }
}
```

字段也接受 searchUrl、ruleSearch、bookInfoUrl、ruleBookInfo 等 Legado 风格别名。

## 数据模型

- SearchResult：标题、作者、书籍 URL、来源名称。
- BookInfo：标题、作者、简介、封面 URL、书籍 URL。
- SourceChapter：标题、章节 URL、顺序。
- SourceChapterContent：标题、正文和下一章 URL。

M3 只完成模型和提取器；M4 再将这些模型串接到授权/公开测试站点。

## 规则

- item 是 CSS 选择器，用于定位列表项。
- selector 是相对于列表项的 CSS 选择器。
- attr 可读取链接、图片等属性；不设置时读取文本。
- regex 可选，存在捕获组时返回第一个捕获组。
- JSON 响应支持简单路径，例如 $.books[*].title。
- 搜索结果中的标题、作者等非关键字段允许降级为空值；书籍 URL、目录 URL 和正文内容仍是阅读链路的必需字段。
- 书源运行失败会报告阶段和规则，例如 `toc 规则 item 失败`，而不是只返回无法定位的通用错误。
- 规则未匹配时会跳过不完整的搜索/章节项；不会生成伪造的 `#chapter-*` 章节链接。

## replaceRules 内容替换

replaceRules 是可选数组，应用在正文提取完成后，按照数组顺序逐条执行。每条规则包含 pattern、replacement 和可选的 enabled 字段；enabled 未填写时默认为 true。

```json
{
  "replaceRules": [
    { "pattern": "\\s+", "replacement": " ", "enabled": true },
    { "pattern": "广告.*?$", "replacement": "" }
  ]
}
```

pattern 使用 Rust regex 语法，replacement 支持 $1 等捕获组引用。单个书源最多 32 条规则，pattern 最多 512 字节，replacement 最多 4 KiB；停用规则会在校验结果中给出提示。替换只作用于章节正文，不改变搜索、详情或目录字段。

## permission 权限记录

书源可以声明来源权限与人工复核信息：

```json
{
  "permission": {
    "status": "authorized",
    "scope": "自有测试站点/公版内容",
    "reviewedAt": "2026-08-01"
  }
}
```

`status` 允许 `unknown`、`authorized`、`public_domain` 和 `personal`。这些字段是维护记录，不是平台对授权真实性的证明；安全审计会对 `unknown`、缺少 `scope` 或缺少 `reviewedAt` 给出提示。即使权限记录完整，敏感请求头仍会被拒绝。

## HTTP 安全边界

- 只允许 http 和 https，最多跟随 5 次重定向。
- 默认超时 15 秒。
- 默认响应体上限 2 MiB。
- 预览命令最多返回 2,000 个字符。
- 前端 WebView CSP 不开放任意 HTTPS 连接；远程来源请求统一由 Rust 受限客户端发起。
- M3 不执行 JavaScript，不自动携带 Cookie/Authorization，不绕过验证码或付费限制。
- 导入预览中的 XPath、JavaScript、模板和其他 Legado 扩展会保留原文但标记为兼容保留；保存成功不代表这些规则已经可以运行。
- 测试夹具使用 example.test，不依赖真实站点。

## 校验

桌面端“书源”页调用 validate_book_source，检查：

1. JSON 结构和必填字段。
2. URL scheme 与模板占位符。
3. CSS 选择器。
4. 正则表达式。
5. 详情、目录和正文配置是否缺失。

校验通过只代表配置结构可用，不代表目标站点当前可访问或内容已获授权。
