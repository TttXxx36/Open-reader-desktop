import { readFileSync } from "node:fs";

const app = readFileSync(new URL("../src/App.vue", import.meta.url), "utf8");
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
requireContract(/v-model="sourceImportUrl"/.test(app), "书源页必须提供 URL 导入输入框");
requireContract(/@click="importSourceUrl"/.test(app), "URL 导入按钮必须绑定 importSourceUrl");
requireContract(app.includes("import_sources_from_url"), "前端必须调用 URL 书源导入命令");
requireContract(/id="settings"/.test(app), "设置视图必须存在");
requireContract(/import brandMark from "\.\/assets\/open-reader-mark\.svg";/.test(app),
  "品牌图标必须使用真实 SVG 资源");
for (const variable of [
  "--reader-font-family",
  "--reader-content-width",
  "--reader-paragraph-spacing",
  "--reader-text-indent",
]) {
  requireContract(app.includes(variable), "阅读器缺少 " + variable + " 设置变量");
}

if (failures.length > 0) {
  console.error("UI contract check failed:");
  for (const failure of failures) console.error("- " + failure);
  process.exit(1);
}

console.log("UI contract check passed");
