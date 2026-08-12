import { readFileSync } from "node:fs";

const app = readFileSync(new URL("../src/App.vue", import.meta.url), "utf8");
const sourceView = readFileSync(new URL("../src/components/SourceView.vue", import.meta.url), "utf8");
const remoteReaderView = readFileSync(new URL("../src/components/RemoteReaderView.vue", import.meta.url), "utf8");
const localReaderView = readFileSync(new URL("../src/components/LocalReaderView.vue", import.meta.url), "utf8");
const libraryOverview = readFileSync(new URL("../src/components/LibraryOverview.vue", import.meta.url), "utf8");
const readerSettingsPanel = readFileSync(new URL("../src/components/ReaderSettingsPanel.vue", import.meta.url), "utf8");
const failures = [];

function requireContract(condition, message) {
  if (!condition) failures.push(message);
}

const sourceInputMatches = app.match(/ref="sourceImportInput"/g) ?? [];
const sourceInputIndex = app.indexOf('ref="sourceImportInput"');
const libraryViewIndex = app.indexOf("view === 'library'");
requireContract(sourceInputMatches.length === 1, "书源导入 input 应只声明一次");
requireContract(sourceInputIndex >= 0 && libraryViewIndex >= 0 && sourceInputIndex < libraryViewIndex,
  "书源导入 input 必须位于视图条件之外，避免切换视图后失效");
requireContract(/@click="openSettings"/.test(app), "设置导航必须绑定 openSettings");
requireContract(/v-model="sourceImportUrl"/.test(sourceView), "书源页必须提供 URL 导入输入框");
requireContract(/@click="importSourceUrl"/.test(sourceView), "URL 导入按钮必须绑定 importSourceUrl");
requireContract(app.includes("preview_sources_from_url") || sourceView.includes("preview_sources_from_url"), "前端必须调用 URL 书源预览命令");
requireContract(app.includes("preview_sources_from_url"), "URL 书源导入必须先调用预览命令");
requireContract(app.includes("import_sources_selected") || sourceView.includes("import_sources_selected"), "书源导入必须通过选中项命令保存");
requireContract(/@click="confirmSourceImport"/.test(sourceView), "导入预览必须提供确认按钮");
requireContract(/id="settings"/.test(app), "设置视图必须存在");
requireContract(/import brandMark from "\.\/assets\/open-reader-mark\.svg";/.test(app),
  "品牌图标必须使用真实 SVG 资源");
for (const variable of [
  "--reader-font-family",
  "--reader-content-width",
  "--reader-paragraph-spacing",
  "--reader-text-indent",
  "--reader-letter-spacing",
  "--reader-margin-left",
  "--reader-margin-right",
  "--reader-text-align",
]) {
  requireContract(app.includes(variable), "阅读器缺少 " + variable + " 设置变量");
}

requireContract(app.includes("LibraryOverview"), "首页必须接入 LibraryOverview 组件");
requireContract(app.includes("SourceView") && app.includes("RemoteReaderView") && app.includes("LocalReaderView"), "书源和本地/远端阅读器必须拆为独立视图组件");
requireContract(sourceView.includes("openSources") && remoteReaderView.includes("remoteChapter"), "独立视图组件必须保留运行时上下文绑定");
requireContract(libraryOverview.includes("continue-card") && libraryOverview.includes("recent-reading"), "首页组件必须提供继续阅读与最近阅读");
requireContract(libraryOverview.includes("library-stat-card"), "首页组件必须提供书架统计");
requireContract(readerSettingsPanel.includes("字间距") && readerSettingsPanel.includes("自定义背景色"), "阅读设置组件必须提供字间距与自定义颜色");
requireContract(readerSettingsPanel.includes("readingMode") && readerSettingsPanel.includes("textAlign"), "阅读设置组件必须提供阅读模式与文本对齐");
requireContract(app.includes("SETTINGS_VERSION = 2") && app.includes("normalizeHex"), "阅读设置必须包含版本迁移与颜色校验");
requireContract(![app, sourceView, remoteReaderView, localReaderView].some((value) => value.includes("Desktop · M2") || value.includes("⌘1") || value.includes(">M6<")), "界面不得残留过期版本/平台快捷键文案");
requireContract(app.includes("reading-paged") && app.includes("theme-custom"), "阅读器必须包含分页滚动与自定义主题样式");
requireContract(app.includes(":focus-visible"), "阅读器视图必须提供键盘焦点样式");

requireContract(app.includes("online-search-section") && app.includes("在线搜索结果"), "在线搜索结果必须与本地书架明确分区");
requireContract(app.includes("class=\"local-shelf-section\"") && app.includes("本地书架"), "本地书架必须拥有独立分区标题");
requireContract(app.includes("<button\n            v-for=\"item in searchResult.results\"") && app.includes("@click=\"openRemoteBook(item)\""), "搜索结果条目必须使用可点击按钮打开远端书籍");
// Search results intentionally omit internal scan and parse diagnostics from the user-facing surface.
requireContract(!app.includes("search-diagnostics") && !app.includes("search-results-summary"), "搜索结果不得展示扫描、解析和分页统计");
requireContract(app.includes("const bookUrl = item.book_url?.trim();"), "打开远端书籍前必须校验并规范化书籍链接");
requireContract(sourceView.includes("source-subnav") && sourceView.includes("sourcePanel"), "书源页必须提供已保存、导入、管理的二级导航");
requireContract(sourceView.includes("sourcePanel === 'library'") && sourceView.includes("sourcePanel === 'manage'"), "书源页必须把书源列表与配置编辑拆分");
requireContract(sourceView.includes("source-import-workspace") && sourceView.includes("source-import-methods"), "书源导入必须提供独立工作区");
requireContract(app.includes("settings-layout") && app.includes("settings-nav"), "设置页必须提供分类侧栏");
requireContract(readerSettingsPanel.includes("reader-preview-card") && readerSettingsPanel.includes("settings-preset-row"), "阅读设置必须提供实时预览与快捷预设");

if (failures.length > 0) {
  console.error("UI contract check failed:");
  for (const failure of failures) console.error("- " + failure);
  process.exit(1);
}

console.log("UI contract check passed");
