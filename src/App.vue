<script setup lang="ts">
import { computed, onMounted, provide, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import brandMark from "./assets/open-reader-mark.svg";
import LibraryOverview from "./components/LibraryOverview.vue";
import ReaderSettingsPanel from "./components/ReaderSettingsPanel.vue";
import SourceView from "./components/SourceView.vue";
import RemoteReaderView from "./components/RemoteReaderView.vue";
import LocalReaderView from "./components/LocalReaderView.vue";

type View = "library" | "reader" | "sources" | "settings";
type ReaderTheme = "night" | "paper" | "sepia" | "custom";
type ReaderTextAlign = "left" | "justify" | "center";
type ReaderMode = "scroll" | "paged";
type ReaderFont = "system" | "yahei" | "serif" | "kai";

interface BookSummary {
  id: string;
  title: string;
  author: string | null;
  format: string;
  chapter_count: number;
  current_chapter: number;
  progress: number;
  updated_at: string;
}

interface ChapterSummary {
  id: string;
  title: string;
  index: number;
}

interface BookDetail {
  book: BookSummary;
  chapters: ChapterSummary[];
}

interface ChapterContent {
  id: string;
  title: string;
  content: string;
  content_format?: string;
  index: number;
  total: number;
}

type ContentBlockKind = "paragraph" | "heading" | "quote" | "image";

interface ContentSpan {
  text: string;
  emphasis?: "strong" | "em" | null;
}

interface ContentBlock {
  kind: ContentBlockKind;
  level?: number | null;
  spans: ContentSpan[];
  alt?: string | null;
  src?: string | null;
}

interface ReaderSettings {
  version: number;
  fontFamily: ReaderFont;
  fontSize: number;
  lineHeight: number;
  letterSpacing: number;
  contentWidth: number;
  marginLeft: number;
  marginRight: number;
  paragraphSpacing: number;
  textIndent: number;
  textAlign: ReaderTextAlign;
  readingMode: ReaderMode;
  theme: ReaderTheme;
  customBackground: string;
  customText: string;
  customAccent: string;
}

interface SourceValidation {
  valid: boolean;
  source: Record<string, unknown> | null;
  errors: string[];
  warnings: string[];
}

interface SourceSummary {
  id: string;
  name: string;
  enabled: boolean;
  config_json: string;
  updated_at: string;
  source_url: string | null;
  group_name: string;
  source_type: number;
  weight: number;
  enabled_explore: boolean;
  custom_order: number;
  comment: string;
  book_url_pattern: string | null;
  explore_url: string | null;
}

interface SourceImportPreviewEntry {
  index: number;
  name: string | null;
  enabled: boolean;
  valid: boolean;
  error: string | null;
  action: string;
  existing_id: string | null;
  changed_fields: string[];
}

interface SourceImportPreview {
  entries: SourceImportPreviewEntry[];
  valid_count: number;
  invalid_count: number;
}

interface SourceImportUrlPreview {
  payload: string;
  preview: SourceImportPreview;
}

interface SourceAuditReport {
  source_id: string;
  source_name: string;
  enabled: boolean;
  permission_status: string;
  permission_scope: string | null;
  reviewed_at: string | null;
  hosts: string[];
  sensitive_headers: string[];
  errors: string[];
  warnings: string[];
  pass: boolean;
}

interface SourceCacheStatus {
  entries: number;
  bytes: number;
  expired_entries: number;
  oldest_fetched_at: number | null;
  max_entries: number;
  max_bytes: number;
}

interface SourceDebugStep {
  stage: string;
  url: string;
  duration_ms: number;
  status: number | null;
  bytes: number | null;
  error: string | null;
}

interface SourcePipelineResult {
  search_results: Array<{ title: string; author: string | null; book_url: string | null; source_name: string }>;
  book_info: { title: string; author: string | null; intro: string | null; cover_url: string | null; book_url: string };
  chapters: Array<{ title: string; url: string; index: number }>;
  first_chapter: { title: string; content: string; next_url: string | null };
  debug_steps: SourceDebugStep[];
}

interface UnifiedSearchItem {
  source_id: string;
  source_name: string;
  title: string;
  author: string | null;
  book_url: string | null;
}

interface RemoteChapter {
  title: string;
  url: string;
  index: number;
}

interface ChapterUpdateSummary {
  changed: boolean;
  fingerprint: string;
  added: number;
  removed: number;
  retained: number;
}

interface RemoteBookDetail {
  source_id: string;
  source_name: string;
  book_info: {
    title: string;
    author: string | null;
    intro: string | null;
    cover_url: string | null;
    book_url: string;
  };
  chapters: RemoteChapter[];
  debug_steps: SourceDebugStep[];
  chapter_fingerprint: string;
  chapter_update: ChapterUpdateSummary | null;
  stale: boolean;
  refresh_error: string | null;
}

interface RemoteChapterContent {
  title: string;
  content: string;
  next_url: string | null;
  stale: boolean;
  refresh_error: string | null;
}

interface MultiSourceSearchResult {
  results: UnifiedSearchItem[];
  failures: Array<{
    source_id: string;
    source_name: string;
    message: string;
  }>;
  enabled_sources: number;
}

const SETTINGS_KEY = "open-reader.settings";
const SETTINGS_VERSION = 2;
const DEFAULT_READER_SETTINGS: ReaderSettings = {
  version: SETTINGS_VERSION,
  fontFamily: "system",
  fontSize: 19,
  lineHeight: 1.9,
  letterSpacing: 0,
  contentWidth: 820,
  marginLeft: 0,
  marginRight: 0,
  paragraphSpacing: 1.3,
  textIndent: 2,
  textAlign: "left",
  readingMode: "scroll",
  theme: "night",
  customBackground: "#101827",
  customText: "#dfe7f2",
  customAccent: "#86dfc2",
};
const readerFontStacks: Record<ReaderFont, string> = {
  system: '"Segoe UI Variable", "Microsoft YaHei UI", "Microsoft YaHei", "Noto Sans CJK SC", sans-serif',
  yahei: '"Microsoft YaHei UI", "Microsoft YaHei", "Noto Sans CJK SC", sans-serif',
  serif: '"Noto Serif CJK SC", "Source Han Serif SC", "Songti SC", SimSun, serif',
  kai: '"KaiTi", "Kaiti SC", "STKaiti", serif',
};
const view = ref<View>("library");
const books = ref<BookSummary[]>([]);
const recentBooks = computed(() => books.value.slice(0, 4));
const continueBook = computed(() =>
  books.value.find((book) => book.progress > 0 && book.progress < 1) ?? books.value[0] ?? null,
);
const detail = ref<BookDetail | null>(null);
const chapter = ref<ChapterContent | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
const sourceImportInput = ref<HTMLInputElement | null>(null);
const status = ref("正在加载书架…");
const errorMessage = ref("");
const isImporting = ref(false);
const settings = ref<ReaderSettings>(loadSettings());
const sourceBusy = ref(false);
const sourceValidation = ref<SourceValidation | null>(null);
const sources = ref<SourceSummary[]>([]);
const sourceGroupFilter = ref("");
const sourceGroupDraft = ref("");
const sourceWeightDraft = ref("0");
const sourceOrderDraft = ref("0");
const sourceExploreDraft = ref(false);
const sourceCommentDraft = ref("");
const selectedSourceIds = ref<string[]>([]);
const sourceBatchBusy = ref(false);
const filteredSources = computed(() => {
  const filter = sourceGroupFilter.value.trim().toLocaleLowerCase();
  if (!filter) return sources.value;
  return sources.value.filter((source) =>
    source.group_name.toLocaleLowerCase().includes(filter),
  );
});
const allFilteredSourcesSelected = computed(() =>
  filteredSources.value.length > 0 &&
  filteredSources.value.every((source) => selectedSourceIds.value.includes(source.id)),
);
const sourceId = ref<string | null>(null);
const sourceListBusy = ref(false);
const sourcePipelineBusy = ref(false);
const sourceKeyword = ref("demo");
const sourcePipeline = ref<SourcePipelineResult | null>(null);
const searchKeyword = ref("");
const searchBusy = ref(false);
const searchResult = ref<MultiSourceSearchResult | null>(null);
const sourceTransferBusy = ref(false);
const sourceTransferMessage = ref("");
const sourceImportUrl = ref("");
const sourceImportPreview = ref<SourceImportPreview | null>(null);
const sourceImportPayload = ref("");
const sourceImportLabel = ref("");
const sourceAuditBusy = ref(false);
const sourceAudit = ref<SourceAuditReport[] | null>(null);
const sourceCacheBusy = ref(false);
const sourceCacheStatus = ref<SourceCacheStatus | null>(null);
const remoteBusy = ref(false);
const remoteBook = ref<RemoteBookDetail | null>(null);
const remoteChapter = ref<RemoteChapterContent | null>(null);
const remoteChapterRef = ref<RemoteChapter | null>(null);
const sourceJson = ref(`{
  "name": "Public HTML Fixture",
  "searchUrl": "https://example.test/search?q={{keyword}}",
  "search": {
    "item": "article.book",
    "title": { "selector": "h2 a" },
    "author": { "selector": ".author" },
    "url": { "selector": "h2 a", "attr": "href" }
  },
  "bookInfoUrl": "https://example.test/book/{{bookId}}",
  "tocUrl": "https://example.test/book/{{bookId}}/toc",
  "contentUrl": "https://example.test/chapter/{{chapterId}}"
}`);

const chapterParagraphs = computed(() =>
  chapter.value?.content.split(/\n{2,}/).filter(Boolean) ?? [],
);
const chapterBlocks = computed(() => parseContentBlocks(chapter.value));
const remoteChapterParagraphs = computed(() =>
  remoteChapter.value?.content.split(/\n{2,}/).filter(Boolean) ?? [],
);
const readerStyle = computed(() => ({
  "--reader-font-family": readerFontStacks[settings.value.fontFamily],
  "--reader-font-size": String(settings.value.fontSize) + "px",
  "--reader-line-height": settings.value.lineHeight,
  "--reader-letter-spacing": String(settings.value.letterSpacing) + "em",
  "--reader-content-width": String(settings.value.contentWidth) + "px",
  "--reader-margin-left": String(settings.value.marginLeft) + "px",
  "--reader-margin-right": String(settings.value.marginRight) + "px",
  "--reader-paragraph-spacing": String(settings.value.paragraphSpacing) + "em",
  "--reader-text-indent": String(settings.value.textIndent) + "em",
  "--reader-text-align": settings.value.textAlign,
  "--reader-custom-background": settings.value.customBackground,
  "--reader-custom-text": settings.value.customText,
  "--reader-custom-accent": settings.value.customAccent,
}));
const themeLabels: Record<ReaderTheme, string> = {
  night: "夜间",
  paper: "纸张",
  sepia: "暖色",
  custom: "自定义",
};
onMounted(loadBooks);
watch(settings, (value) => {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({
      version: SETTINGS_VERSION,
      settings: { ...value, version: SETTINGS_VERSION },
    }));
  } catch {
    // localStorage 不可用时保持阅读，不阻断正文渲染。
  }
}, { deep: true });

function parseContentBlocks(value: ChapterContent | null): ContentBlock[] {
  if (!value || value.content_format !== "blocks-v1") return [];

  try {
    const parsed = JSON.parse(value.content) as {
      version?: unknown;
      blocks?: unknown;
    };
    if (parsed.version !== 1 || !Array.isArray(parsed.blocks)) return [];

    return parsed.blocks.flatMap((candidate) => {
      if (!candidate || typeof candidate !== "object") return [];
      const record = candidate as Record<string, unknown>;
      const kind = record.kind;
      if (kind !== "paragraph" && kind !== "heading" && kind !== "quote" && kind !== "image") {
        return [];
      }

      const spans = Array.isArray(record.spans)
        ? record.spans.flatMap((span) => {
            if (!span || typeof span !== "object") return [];
            const item = span as Record<string, unknown>;
            if (typeof item.text !== "string" || !item.text) return [];
            const emphasis: ContentSpan["emphasis"] =
              item.emphasis === "strong" || item.emphasis === "em"
                ? item.emphasis
                : null;
            return [{ text: item.text, emphasis }];
          })
        : [];

      return [{
        kind,
        level: typeof record.level === "number" ? record.level : null,
        spans,
        alt: typeof record.alt === "string" ? record.alt : null,
        src: typeof record.src === "string" && /^data:image\/(png|jpeg|gif|webp|bmp);base64,/i.test(record.src)
          ? record.src
          : null,
      }];
    });
  } catch {
    return [];
  }
}

function contentBlockTag(block: ContentBlock): string {
  if (block.kind === "quote") return "blockquote";
  if (block.kind !== "heading") return "p";
  const level = Math.min(Math.max(Math.round(block.level ?? 3), 2), 6);
  return "h" + level;
}

function clampNumber(value: unknown, fallback: number, min: number, max: number) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
}

function normalizeHex(value: unknown, fallback: string) {
  return typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value) ? value.toLowerCase() : fallback;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function loadSettings(): ReaderSettings {
  try {
    const raw = JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "{}") as unknown;
    const payload = isRecord(raw) ? raw : {};
    const saved = isRecord(payload.settings) ? payload.settings : payload;
    const fontFamily = saved.fontFamily;
    const theme = saved.theme;
    const textAlign = saved.textAlign;
    const readingMode = saved.readingMode;
    const migrated: ReaderSettings = {
      ...DEFAULT_READER_SETTINGS,
      version: SETTINGS_VERSION,
      fontFamily: fontFamily === "yahei" || fontFamily === "serif" || fontFamily === "kai" ? fontFamily : "system",
      fontSize: clampNumber(saved.fontSize, DEFAULT_READER_SETTINGS.fontSize, 15, 30),
      lineHeight: clampNumber(saved.lineHeight, DEFAULT_READER_SETTINGS.lineHeight, 1.4, 2.4),
      letterSpacing: clampNumber(saved.letterSpacing, DEFAULT_READER_SETTINGS.letterSpacing, -0.02, 0.12),
      contentWidth: clampNumber(saved.contentWidth, DEFAULT_READER_SETTINGS.contentWidth, 560, 1100),
      marginLeft: clampNumber(saved.marginLeft, DEFAULT_READER_SETTINGS.marginLeft, 0, 96),
      marginRight: clampNumber(saved.marginRight, DEFAULT_READER_SETTINGS.marginRight, 0, 96),
      paragraphSpacing: clampNumber(saved.paragraphSpacing, DEFAULT_READER_SETTINGS.paragraphSpacing, 0.4, 2.4),
      textIndent: clampNumber(saved.textIndent, DEFAULT_READER_SETTINGS.textIndent, 0, 2),
      textAlign: textAlign === "justify" || textAlign === "center" ? textAlign : "left",
      readingMode: readingMode === "paged" ? "paged" : "scroll",
      theme: theme === "paper" || theme === "sepia" || theme === "custom" ? theme : "night",
      customBackground: normalizeHex(saved.customBackground, DEFAULT_READER_SETTINGS.customBackground),
      customText: normalizeHex(saved.customText, DEFAULT_READER_SETTINGS.customText),
      customAccent: normalizeHex(saved.customAccent, DEFAULT_READER_SETTINGS.customAccent),
    };
    if (payload.version !== SETTINGS_VERSION || !isRecord(payload.settings)) {
      try {
        localStorage.setItem(SETTINGS_KEY, JSON.stringify({
          version: SETTINGS_VERSION,
          settings: migrated,
        }));
      } catch {
        // 迁移失败不影响本次启动。
      }
    }
    return migrated;
  } catch {
    return { ...DEFAULT_READER_SETTINGS };
  }
}
async function loadBooks() {
  try {
    books.value = await invoke<BookSummary[]>("list_books");
    status.value = books.value.length ? `共 ${books.value.length} 本书` : "书架已准备好";
    errorMessage.value = "";
  } catch (error) {
    status.value = "请在 Tauri 桌面模式中打开";
    errorMessage.value = String(error);
  }
}

async function openSources() {
  view.value = "sources";
  errorMessage.value = "";
  await Promise.all([loadSources(), refreshSourceCacheStatus()]);
}

function openSettings() {
  view.value = "settings";
  errorMessage.value = "";
}

function closeSettings() {
  view.value = remoteBook.value && remoteChapter.value || detail.value && chapter.value ? "reader" : "library";
  errorMessage.value = "";
}

function resetSettings() {
  settings.value = { ...DEFAULT_READER_SETTINGS };
}

async function loadSources() {
  sourceListBusy.value = true;
  try {
    sources.value = await invoke<SourceSummary[]>("list_sources");
    selectedSourceIds.value = selectedSourceIds.value.filter((id) =>
      sources.value.some((source) => source.id === id),
    );
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceListBusy.value = false;
  }
}

async function runSourceAudit() {
  sourceAuditBusy.value = true;
  errorMessage.value = "";
  try {
    sourceAudit.value = await invoke<SourceAuditReport[]>("audit_sources");
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceAuditBusy.value = false;
  }
}

async function refreshSourceCacheStatus() {
  sourceCacheBusy.value = true;
  try {
    sourceCacheStatus.value = await invoke<SourceCacheStatus>("get_source_cache_status");
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceCacheBusy.value = false;
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}

function selectSource(source: SourceSummary) {
  sourceId.value = source.id;
  sourceJson.value = source.config_json;
  sourceGroupDraft.value = source.group_name;
  sourceWeightDraft.value = String(source.weight);
  sourceOrderDraft.value = String(source.custom_order);
  sourceExploreDraft.value = source.enabled_explore;
  sourceCommentDraft.value = source.comment;
  sourceValidation.value = null;
  sourcePipeline.value = null;
  errorMessage.value = "";
}

function newSourceDraft() {
  sourceId.value = null;
  sourceGroupDraft.value = "";
  sourceWeightDraft.value = "0";
  sourceOrderDraft.value = "0";
  sourceExploreDraft.value = false;
  sourceCommentDraft.value = "";
  sourceValidation.value = null;
  sourcePipeline.value = null;
  errorMessage.value = "";
}

async function saveSource() {
  sourceBusy.value = true;
  errorMessage.value = "";
  try {
    const saved = await invoke<SourceSummary>("save_source", {
      sourceId: sourceId.value,
      configJson: sourceJson.value,
    });
    sourceId.value = saved.id;
    await loadSources();
    selectSource(saved);
    sourceValidation.value = {
      valid: true,
      source: null,
      errors: [],
      warnings: [],
    };
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceBusy.value = false;
  }
}

async function saveSourceMetadata() {
  if (!sourceId.value) {
    errorMessage.value = "请先保存书源配置，再编辑元数据";
    return;
  }

  const weight = Number(sourceWeightDraft.value);
  const customOrder = Number(sourceOrderDraft.value);
  if (!Number.isInteger(weight) || !Number.isInteger(customOrder)) {
    errorMessage.value = "权重和自定义顺序必须是整数";
    return;
  }

  sourceBusy.value = true;
  errorMessage.value = "";
  try {
    const saved = await invoke<SourceSummary>("update_source_metadata", {
      sourceId: sourceId.value,
      groupName: sourceGroupDraft.value,
      weight,
      customOrder,
      enabledExplore: sourceExploreDraft.value,
      comment: sourceCommentDraft.value,
    });
    await loadSources();
    selectSource(saved);
    sourceTransferMessage.value = "书源元数据已保存";
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceBusy.value = false;
  }
}

async function toggleSource(source: SourceSummary) {
  try {
    await invoke("set_source_enabled", {
      sourceId: source.id,
      enabled: !source.enabled,
    });
    await loadSources();
  } catch (error) {
    errorMessage.value = String(error);
  }
}

async function toggleSourceExplore(source: SourceSummary) {
  try {
    const saved = await invoke<SourceSummary>("set_source_explore_enabled", {
      sourceId: source.id,
      enabled: !source.enabled_explore,
    });
    await loadSources();
    if (sourceId.value === saved.id) selectSource(saved);
  } catch (error) {
    errorMessage.value = String(error);
  }
}

function toggleSourceSelection(sourceId: string) {
  selectedSourceIds.value = selectedSourceIds.value.includes(sourceId)
    ? selectedSourceIds.value.filter((id) => id !== sourceId)
    : [...selectedSourceIds.value, sourceId];
}

function toggleSelectAllSources() {
  const visibleIds = filteredSources.value.map((source) => source.id);
  if (!visibleIds.length) return;
  if (allFilteredSourcesSelected.value) {
    selectedSourceIds.value = selectedSourceIds.value.filter((id) => !visibleIds.includes(id));
  } else {
    selectedSourceIds.value = [...new Set([...selectedSourceIds.value, ...visibleIds])];
  }
}

async function applySourceBatch(action: "enable" | "disable" | "explore-on" | "explore-off" | "delete") {
  const sourceIds = [...selectedSourceIds.value];
  if (!sourceIds.length) {
    errorMessage.value = "请先选择书源";
    return;
  }
  if (action === "delete" && !window.confirm(`确定删除选中的 ${sourceIds.length} 个书源吗？`)) return;

  sourceBatchBusy.value = true;
  errorMessage.value = "";
  try {
    if (action === "delete") {
      await invoke("delete_sources", { sourceIds });
    } else if (action === "enable" || action === "disable") {
      await invoke("set_sources_enabled", {
        sourceIds,
        enabled: action === "enable",
      });
    } else {
      await invoke("set_sources_explore_enabled", {
        sourceIds,
        enabled: action === "explore-on",
      });
    }
    await loadSources();
    selectedSourceIds.value = [];
    if (sourceId.value && !sources.value.some((source) => source.id === sourceId.value)) {
      newSourceDraft();
    }
    sourceTransferMessage.value = `已完成 ${sourceIds.length} 个书源的批量操作`;
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceBatchBusy.value = false;
  }
}

async function reorderSource(source: SourceSummary, direction: "up" | "down") {
  const groupSources = sources.value.filter((item) => item.group_name === source.group_name);
  const index = groupSources.findIndex((item) => item.id === source.id);
  const targetIndex = direction === "up" ? index - 1 : index + 1;
  if (index < 0 || targetIndex < 0 || targetIndex >= groupSources.length) return;

  const orderedIds = groupSources.map((item) => item.id);
  [orderedIds[index], orderedIds[targetIndex]] = [orderedIds[targetIndex], orderedIds[index]];
  sourceBatchBusy.value = true;
  errorMessage.value = "";
  try {
    await invoke("reorder_sources", { sourceIds: orderedIds });
    await loadSources();
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceBatchBusy.value = false;
  }
}

async function deleteSource(source: SourceSummary) {
  if (!window.confirm(`确定删除书源“${source.name}”吗？`)) return;
  try {
    await invoke("delete_source", { sourceId: source.id });
    if (sourceId.value === source.id) newSourceDraft();
    await loadSources();
  } catch (error) {
    errorMessage.value = String(error);
  }
}

async function searchSources() {
  const keyword = searchKeyword.value.trim();
  if (!keyword) return;

  searchBusy.value = true;
  searchResult.value = null;
  errorMessage.value = "";
  try {
    searchResult.value = await invoke<MultiSourceSearchResult>("search_sources", { keyword });
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    searchBusy.value = false;
  }
}

function clearSearch() {
  searchResult.value = null;
  searchKeyword.value = "";
}

async function finishSourceImport(imported: SourceSummary[], label: string) {
  await loadSources();
  if (imported[0]) {
    selectSource(imported[0]);
  }
  sourceTransferMessage.value = "已从" + label + "导入 " + imported.length + " 个书源";
}

async function exportSources() {
  sourceTransferBusy.value = true;
  errorMessage.value = "";
  try {
    const payload = await invoke<string>("export_sources");
    const blob = new Blob([payload], { type: "application/json;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = "open-reader-sources-" + new Date().toISOString().slice(0, 10) + ".json";
    anchor.click();
    URL.revokeObjectURL(url);
    sourceTransferMessage.value = "书源已导出，请检查下载目录";
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceTransferBusy.value = false;
  }
}

function openSourceImportPicker() {
  sourceTransferMessage.value = "";
  sourceImportInput.value?.click();
}

function showSourceImportPreview(
  preview: SourceImportPreview,
  payload: string,
  label: string,
) {
  sourceImportPreview.value = preview;
  sourceImportPayload.value = payload;
  sourceImportLabel.value = label;
  sourceTransferMessage.value =
    "已解析 " + preview.entries.length + " 个书源：" +
    preview.valid_count + " 个可导入，" +
    preview.invalid_count + " 个将跳过";
}

function clearSourceImportPreview() {
  sourceImportPreview.value = null;
  sourceImportPayload.value = "";
  sourceImportLabel.value = "";
}

async function confirmSourceImport() {
  const preview = sourceImportPreview.value;
  const payload = sourceImportPayload.value;
  const label = sourceImportLabel.value || "来源";
  if (!preview || !payload) return;

  const indices = preview.entries
    .filter((entry) => entry.valid)
    .map((entry) => entry.index);
  if (indices.length === 0) {
    errorMessage.value = "没有可导入的兼容书源";
    return;
  }

  sourceTransferBusy.value = true;
  sourceTransferMessage.value = "";
  errorMessage.value = "";
  try {
    const imported = await invoke<SourceSummary[]>("import_sources_selected", {
      bundleJson: payload,
      indices,
    });
    await finishSourceImport(imported, label);
    clearSourceImportPreview();
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceTransferBusy.value = false;
  }
}

async function importSourceUrl() {
  const url = sourceImportUrl.value.trim();
  if (!url) {
    errorMessage.value = "请先输入书源 URL";
    return;
  }

  sourceTransferBusy.value = true;
  sourceTransferMessage.value = "";
  errorMessage.value = "";
  try {
    const result = await invoke<SourceImportUrlPreview>("preview_sources_from_url", { url });
    showSourceImportPreview(result.preview, result.payload, "URL");
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceTransferBusy.value = false;
  }
}

async function importSourceFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;

  if (file.size > 2 * 1024 * 1024) {
    errorMessage.value = "书源文件超过 2 MB 限制";
    input.value = "";
    return;
  }

  sourceTransferBusy.value = true;
  sourceTransferMessage.value = "";
  errorMessage.value = "";
  try {
    const payload = await file.text();
    const preview = await invoke<SourceImportPreview>("preview_sources", {
      bundleJson: payload,
    });
    showSourceImportPreview(preview, payload, "本地文件");
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceTransferBusy.value = false;
    input.value = "";
  }
}

async function openRemoteBook(item: UnifiedSearchItem) {
  if (!item.book_url || remoteBusy.value) return;

  remoteBusy.value = true;
  errorMessage.value = "";
  remoteBook.value = null;
  remoteChapter.value = null;
  remoteChapterRef.value = null;

  try {
    const loaded = await invoke<RemoteBookDetail>("fetch_source_book", {
      sourceId: item.source_id,
      bookUrl: item.book_url,
      forceRefresh: false,
    });
    const firstChapter = loaded.chapters[0];
    if (!firstChapter) {
      throw new Error("书源未返回可阅读章节");
    }

    const firstContent = await invoke<RemoteChapterContent>("fetch_source_chapter", {
      sourceId: loaded.source_id,
      chapter: firstChapter,
      forceRefresh: false,
    });
    remoteBook.value = loaded;
    remoteChapterRef.value = firstChapter;
    remoteChapter.value = firstContent;
    searchResult.value = null;
    view.value = "reader";
  } catch (error) {
    errorMessage.value = String(error);
    remoteBook.value = null;
    remoteChapter.value = null;
    remoteChapterRef.value = null;
  } finally {
    remoteBusy.value = false;
  }
}

async function loadRemoteChapter(chapterItem: RemoteChapter, forceRefresh = false) {
  if (!remoteBook.value || remoteBusy.value) return;

  remoteBusy.value = true;
  errorMessage.value = "";
  try {
    remoteChapter.value = await invoke<RemoteChapterContent>("fetch_source_chapter", {
      sourceId: remoteBook.value.source_id,
      chapter: chapterItem,
      forceRefresh,
    });
    remoteChapterRef.value = chapterItem;
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    remoteBusy.value = false;
  }
}

async function refreshRemoteBook() {
  if (!remoteBook.value || remoteBusy.value) return;

  const currentUrl = remoteChapterRef.value?.url;
  remoteBusy.value = true;
  errorMessage.value = "";
  try {
    const loaded = await invoke<RemoteBookDetail>("fetch_source_book", {
      sourceId: remoteBook.value.source_id,
      bookUrl: remoteBook.value.book_info.book_url,
      forceRefresh: true,
    });
    const currentIndex = remoteChapterIndex();
    const target = loaded.chapters.find((item) => item.url === currentUrl)
      ?? loaded.chapters[Math.min(Math.max(currentIndex, 0), loaded.chapters.length - 1)]
      ?? loaded.chapters[0];
    if (!target) {
      throw new Error("书源未返回可阅读章节");
    }

    const content = await invoke<RemoteChapterContent>("fetch_source_chapter", {
      sourceId: loaded.source_id,
      chapter: target,
      forceRefresh: true,
    });
    remoteBook.value = loaded;
    remoteChapterRef.value = target;
    remoteChapter.value = content;
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    remoteBusy.value = false;
  }
}

function remoteChapterIndex() {
  if (!remoteBook.value || !remoteChapterRef.value) return -1;
  return remoteBook.value.chapters.findIndex((item) => item.url === remoteChapterRef.value?.url);
}

function goToRemoteChapter(chapterItem: RemoteChapter) {
  void loadRemoteChapter(chapterItem);
}

function previousRemoteChapter() {
  if (!remoteBook.value) return;
  const previous = remoteBook.value.chapters[remoteChapterIndex() - 1];
  if (previous) void loadRemoteChapter(previous);
}

function nextRemoteChapter() {
  if (!remoteBook.value) return;
  const next = remoteBook.value.chapters[remoteChapterIndex() + 1];
  if (next) void loadRemoteChapter(next);
}

async function runSourcePipeline() {
  sourcePipelineBusy.value = true;
  sourcePipeline.value = null;
  errorMessage.value = "";
  try {
    sourcePipeline.value = await invoke<SourcePipelineResult>("run_source_pipeline", {
      configJson: sourceJson.value,
      keyword: sourceKeyword.value.trim(),
    });
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourcePipelineBusy.value = false;
  }
}

async function validateSource() {
  sourceBusy.value = true;
  sourceValidation.value = null;
  errorMessage.value = "";
  try {
    sourceValidation.value = await invoke<SourceValidation>("validate_book_source", {
      configJson: sourceJson.value,
    });
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceBusy.value = false;
  }
}

function openFilePicker() {
  fileInput.value?.click();
}

async function importFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;

  if (file.size > 64 * 1024 * 1024) {
    errorMessage.value = "文件超过 64 MB 限制";
    input.value = "";
    return;
  }

  isImporting.value = true;
  status.value = `正在解析《${file.name}》…`;
  errorMessage.value = "";

  try {
    const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
    const imported = await invoke<BookSummary>("import_book", {
      fileName: file.name,
      bytes,
    });
    await loadBooks();
    await openBook(imported.id);
  } catch (error) {
    errorMessage.value = String(error);
    status.value = "导入失败";
  } finally {
    isImporting.value = false;
    input.value = "";
  }
}

async function openBook(bookId: string) {
  try {
    const loaded = await invoke<BookDetail>("get_book_detail", { bookId });
    detail.value = loaded;
    view.value = "reader";

    const safeIndex = Math.min(
      Math.max(loaded.book.current_chapter, 0),
      Math.max(loaded.chapters.length - 1, 0),
    );
    const firstChapter = loaded.chapters[safeIndex];
    if (firstChapter) {
      await loadChapter(firstChapter.id, false);
    }
  } catch (error) {
    errorMessage.value = String(error);
  }
}

async function loadChapter(chapterId: string, persist = true) {
  if (!detail.value) return;

  try {
    const loaded = await invoke<ChapterContent>("get_chapter_content", {
      bookId: detail.value.book.id,
      chapterId,
    });
    chapter.value = loaded;
    const index = detail.value.chapters.findIndex((item) => item.id === chapterId);
    detail.value.book.current_chapter = Math.max(index, 0);
    if (persist && index >= 0) {
      await saveProgress(index, chapterId);
    }
  } catch (error) {
    errorMessage.value = String(error);
  }
}

async function saveProgress(index: number, chapterId: string) {
  if (!detail.value) return;

  const lastIndex = Math.max(detail.value.chapters.length - 1, 1);
  const progress = index / lastIndex;
  await invoke("save_progress", {
    bookId: detail.value.book.id,
    chapterId,
    currentChapter: index,
    progress,
  });
  detail.value.book.progress = progress;
  const bookIndex = books.value.findIndex((item) => item.id === detail.value?.book.id);
  if (bookIndex >= 0) {
    books.value[bookIndex] = { ...books.value[bookIndex], current_chapter: index, progress };
  }
}

function continueReading(book: BookSummary) {
  void openBook(book.id);
}

function closeReader() {
  view.value = "library";
  detail.value = null;
  chapter.value = null;
  remoteBook.value = null;
  remoteChapter.value = null;
  remoteChapterRef.value = null;
  void loadBooks();
}

function cycleTheme() {
  settings.value.theme = settings.value.theme === "night"
    ? "paper"
    : settings.value.theme === "paper"
      ? "sepia"
      : settings.value.theme === "sepia"
        ? "custom"
        : "night";
}

function formatProgress(progress: number) {
  return `${Math.round(progress * 100)}%`;
}

function currentChapterIndex() {
  if (!detail.value || !chapter.value) return -1;
  return detail.value.chapters.findIndex((item) => item.id === chapter.value?.id);
}

function goToChapter(chapterItem: ChapterSummary) {
  void loadChapter(chapterItem.id);
}

function previousChapter() {
  if (!detail.value) return;
  const index = currentChapterIndex();
  const previous = detail.value.chapters[index - 1];
  if (previous) void loadChapter(previous.id);
}

function nextChapter() {
  if (!detail.value) return;
  const index = currentChapterIndex();
  const next = detail.value.chapters[index + 1];
  if (next) void loadChapter(next.id);
}

provide("open-reader-context", { SETTINGS_KEY, SETTINGS_VERSION, DEFAULT_READER_SETTINGS, readerFontStacks, view, books, recentBooks, continueBook, detail, chapter, fileInput, sourceImportInput, status, errorMessage, isImporting, settings, sourceBusy, sourceValidation, sources, filteredSources, sourceGroupFilter, sourceGroupDraft, sourceWeightDraft, sourceOrderDraft, sourceExploreDraft, sourceCommentDraft, selectedSourceIds, sourceBatchBusy, allFilteredSourcesSelected, sourceId, sourceListBusy, sourcePipelineBusy, sourceKeyword, sourcePipeline, searchKeyword, searchBusy, searchResult, sourceTransferBusy, sourceTransferMessage, sourceImportUrl, sourceImportPreview, sourceImportPayload, sourceImportLabel, sourceAuditBusy, sourceAudit, sourceCacheBusy, sourceCacheStatus, remoteBusy, remoteBook, remoteChapter, remoteChapterRef, sourceJson, chapterParagraphs, chapterBlocks, remoteChapterParagraphs, readerStyle, themeLabels, parseContentBlocks, contentBlockTag, clampNumber, normalizeHex, isRecord, loadSettings, loadBooks, openSources, openSettings, closeSettings, resetSettings, loadSources, runSourceAudit, refreshSourceCacheStatus, formatBytes, selectSource, newSourceDraft, saveSource, saveSourceMetadata, toggleSource, toggleSourceExplore, toggleSourceSelection, toggleSelectAllSources, applySourceBatch, reorderSource, deleteSource, searchSources, clearSearch, finishSourceImport, exportSources, openSourceImportPicker, showSourceImportPreview, clearSourceImportPreview, confirmSourceImport, importSourceUrl, importSourceFile, openRemoteBook, loadRemoteChapter, remoteChapterIndex, goToRemoteChapter, previousRemoteChapter, nextRemoteChapter, runSourcePipeline, validateSource, openFilePicker, importFile, openBook, loadChapter, saveProgress, continueReading, closeReader, cycleTheme, formatProgress, currentChapterIndex, goToChapter, previousChapter, nextChapter });
</script>

<template>
  <main class="shell">
    <input
      ref="sourceImportInput"
      class="file-input"
      type="file"
      accept=".json,application/json"
      @change="importSourceFile"
    />
    <aside class="sidebar">
      <div class="brand">
        <span class="brand-mark"><img :src="brandMark" alt="" aria-hidden="true" /></span>
        <div>
          <strong>Open Reader</strong>
          <span>Windows 阅读器</span>
        </div>
      </div>

      <nav class="nav" aria-label="主导航">
        <button class="nav-item" :class="{ active: view === 'library' }" type="button" @click="closeReader">书架</button>
        <button class="nav-item" :class="{ active: view === 'sources' }" type="button" @click="openSources">书源</button>
        <button class="nav-item" :class="{ active: view === 'settings' }" type="button" @click="openSettings">设置</button>
      </nav>

      <div class="sidebar-note">
        <span class="eyebrow">LOCAL FIRST</span>
        <p>TXT / EPUB 内容保存在本机，阅读进度写入 SQLite。</p>
      </div>
    </aside>

    <section v-if="view === 'library'" class="content" id="library">
      <input
        ref="fileInput"
        class="file-input"
        type="file"
        accept=".txt,.epub,text/plain,application/epub+zip"
        @change="importFile"
      />

      <header class="topbar">
        <div>
          <span class="eyebrow">YOUR LIBRARY</span>
          <h1>书架</h1>
        </div>
        <div class="library-actions">
          <input
            v-model="searchKeyword"
            class="library-search-input"
            aria-label="搜索书源中的书"
            placeholder="搜索书源中的书"
            @keyup.enter="searchSources"
          />
          <button class="secondary-button" type="button" :disabled="searchBusy || !searchKeyword.trim()" @click="searchSources">
            {{ searchBusy ? "搜索中…" : "搜索书源" }}
          </button>
          <button class="import-button" type="button" :disabled="isImporting" @click="openFilePicker">
            {{ isImporting ? "解析中…" : "导入 TXT / EPUB" }}
          </button>
        </div>
      </header>

      <div class="status-banner" role="status">
        <span class="status-dot"></span>
        <span>{{ status }}</span>
        <span v-if="errorMessage" class="error-text">{{ errorMessage }}</span>
      </div>

      <LibraryOverview
        :books="books"
        :continue-book="continueBook"
        :recent-books="recentBooks"
        @continue="continueReading"
        @import="openFilePicker"
        @sources="openSources"
      />

      <section v-if="searchResult" class="search-results-panel" aria-live="polite">
        <div class="search-results-heading">
          <div>
            <span class="eyebrow">MULTI-SOURCE SEARCH</span>
            <h2>搜索结果</h2>
          </div>
          <button class="source-link-button" type="button" @click="clearSearch">清除</button>
        </div>
        <p class="search-results-summary">
          已查询 {{ searchResult.enabled_sources }} 个启用书源，去重后 {{ searchResult.results.length }} 条结果。
        </p>
        <p v-if="!searchResult.results.length" class="search-results-empty">没有找到匹配书籍。</p>
        <div v-else class="search-results-list">
          <article
            v-for="item in searchResult.results"
            :key="item.source_id + '-' + item.title + '-' + (item.author || '')"
            class="search-result-row"
            :class="{ clickable: Boolean(item.book_url) }"
            :tabindex="item.book_url ? 0 : undefined"
            @click="openRemoteBook(item)"
            @keydown.enter="openRemoteBook(item)"
          >
            <div>
              <h3>{{ item.title || "未命名书籍" }}</h3>
              <p>{{ item.author || "作者未知" }}</p>
            </div>
            <div class="search-result-actions">
              <span class="search-source-badge">{{ item.source_name }}</span>
              <span class="search-open-label">{{ item.book_url ? (remoteBusy ? "加载中…" : "打开") : "无链接" }}</span>
            </div>
          </article>
        </div>
        <div v-if="searchResult.failures.length" class="search-failures">
          <strong>{{ searchResult.failures.length }} 个书源失败，已隔离</strong>
          <p v-for="failure in searchResult.failures" :key="failure.source_id + '-' + failure.message">
            {{ failure.source_name }}：{{ failure.message }}
          </p>
        </div>
        <p class="search-results-note">有书籍链接的结果可直接打开详情和章节阅读；无链接结果仅展示元数据。</p>
      </section>

      <section v-if="books.length" class="library-grid" aria-label="本地书架">
        <article
          v-for="book in books"
          :key="book.id"
          class="book-card"
          tabindex="0"
          @click="continueReading(book)"
          @keydown.enter="continueReading(book)"
        >
          <div class="book-cover" :class="`format-${book.format}`">
            <span>{{ book.format.toUpperCase() }}</span>
          </div>
          <div class="book-card-body">
            <span class="book-format">{{ book.chapter_count }} 章 · {{ formatProgress(book.progress) }}</span>
            <h2>{{ book.title }}</h2>
            <p>{{ book.author || "本地导入" }}</p>
            <div class="progress-track"><span :style="{ width: `${book.progress * 100}%` }"></span></div>
          </div>
        </article>
      </section>

      <section v-else class="empty-state">
        <div class="empty-icon">✦</div>
        <h3>书架还是空的</h3>
        <p>导入一本 TXT 或 EPUB，马上开始离线阅读。文件内容只会保存在本机。</p>
        <button class="text-button" type="button" @click="openFilePicker">选择本地书籍 →</button>
      </section>
    </section>


    <SourceView v-else-if="view === 'sources'" />

    <section v-else-if="view === 'settings'" class="content settings-content" id="settings">
      <header class="topbar">
        <div>
          <span class="eyebrow">APP & READER SETTINGS</span>
          <h1>设置</h1>
        </div>
        <button class="secondary-button" type="button" @click="closeSettings">返回阅读</button>
      </header>

      <ReaderSettingsPanel v-model="settings" @reset="resetSettings" />
    </section>

    <RemoteReaderView v-else-if="remoteBook && remoteChapter" />

    <LocalReaderView v-else-if="detail && chapter" />
  </main>
</template>

<style>
.library-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 9px;
  flex-wrap: wrap;
}

.library-search-input {
  width: 210px;
  padding: 10px 12px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 9px;
  color: #dce7f7;
  background: #0c111b;
}

.library-search-input:focus {
  border-color: rgba(139, 183, 255, 0.75);
  outline: none;
}

.search-results-panel {
  margin-top: 22px;
  padding: 22px;
  border: 1px solid rgba(121, 201, 255, 0.22);
  border-radius: 16px;
  background: rgba(17, 34, 52, 0.72);
}

.search-results-heading {
  display: flex;
  align-items: start;
  justify-content: space-between;
  gap: 18px;
}

.search-results-heading h2 {
  margin: 9px 0 0;
  font-size: 20px;
}

.search-results-summary,
.search-results-empty,
.search-results-note {
  color: #8391a6;
  font-size: 12px;
  line-height: 1.6;
}

.search-results-summary {
  margin: 16px 0 0;
}

.search-results-empty {
  margin: 22px 0 0;
}

.search-results-list {
  display: grid;
  gap: 9px;
  margin-top: 18px;
}

 .search-result-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 13px 14px;
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 10px;
  background: rgba(12, 17, 27, 0.52);
}

.search-result-row.clickable {
  cursor: pointer;
}

.search-result-row.clickable:hover,
.search-result-row.clickable:focus-visible {
  border-color: rgba(139, 183, 255, 0.7);
  outline: none;
}

.search-result-actions {
  display: flex;
  align-items: center;
  gap: 9px;
  flex: 0 0 auto;
}

.search-open-label {
  color: #8fcfff;
  font-size: 11px;
}

.search-result-row h3 {
  margin: 0;
  font-size: 14px;
}

.search-result-row p {
  margin: 5px 0 0;
  color: #8391a6;
  font-size: 12px;
}

.search-source-badge {
  flex: 0 0 auto;
  padding: 5px 8px;
  border-radius: 999px;
  color: #b9f6dd;
  background: rgba(30, 101, 82, 0.24);
  font-size: 10px;
}

.search-failures {
  margin-top: 18px;
  padding-top: 15px;
  border-top: 1px solid rgba(255, 176, 188, 0.14);
}

.search-failures strong {
  color: #ffcf9b;
  font-size: 12px;
}

.search-failures p {
  margin: 8px 0 0;
  color: #ffb0bc;
  font-size: 11px;
  line-height: 1.5;
}

.search-results-note {
  margin: 17px 0 0;
}
.nav-item {
  border: 0;
  text-align: left;
  cursor: pointer;
}

.nav-item:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.source-content {
  max-width: 1240px;
}

.source-grid {
  display: grid;
  grid-template-columns: minmax(205px, 0.65fr) minmax(0, 1.35fr) minmax(280px, 0.8fr);
  gap: 18px;
  margin-top: 28px;
}

.source-library,
.source-editor,
.source-result {
  min-width: 0;
  padding: 22px;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 16px;
  background: rgba(19, 27, 42, 0.72);
}

.source-toolbar-actions,
.source-debug-controls,
.source-row-actions {
  display: flex;
  align-items: center;
  gap: 9px;
}

.source-toolbar-actions {
  flex-wrap: wrap;
}

.source-url-input {
  width: 240px;
  padding: 10px 12px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 9px;
  color: #dce7f7;
  background: #0c111b;
}

.source-url-input:focus {
  border-color: rgba(139, 183, 255, 0.75);
  outline: none;
}

.source-import-preview {
  margin-top: 18px;
  padding: 18px 20px;
  border: 1px solid rgba(139, 183, 255, 0.28);
  border-radius: 16px;
  background: rgba(18, 34, 53, 0.72);
}

.source-import-preview-heading,
.source-preview-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.source-import-preview-heading h2 {
  margin: 5px 0 0;
  font-size: 18px;
}

.source-preview-count {
  color: #b9f6dd;
  font-size: 12px;
  font-weight: 700;
}

.source-import-preview-note {
  margin: 12px 0;
  color: #a8b6ca;
  font-size: 12px;
  line-height: 1.6;
}

.source-preview-list {
  display: grid;
  gap: 7px;
  max-height: 240px;
  margin: 0;
  padding: 0;
  overflow: auto;
  list-style: none;
}

.source-preview-entry {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  padding: 9px 11px;
  border: 1px solid rgba(155, 231, 216, 0.16);
  border-radius: 9px;
  color: #dce7f7;
  background: rgba(12, 17, 27, 0.55);
  font-size: 12px;
}

.source-preview-entry strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-preview-main {
  min-width: 0;
  display: grid;
  gap: 3px;
}

.source-preview-main small {
  overflow: hidden;
  color: #8391a6;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-preview-status {
  flex: 0 0 auto;
  color: #b9f6dd;
  text-align: right;
}

.source-preview-entry span {
  color: #b9f6dd;
  text-align: right;
}

.source-preview-entry.invalid {
  border-color: rgba(255, 176, 188, 0.3);
}

.source-preview-entry.invalid span {
  color: #ffb0bc;
}

.source-preview-actions {
  justify-content: flex-start;
  margin-top: 14px;
}

.secondary-button {
  padding: 10px 14px;
  border: 1px solid rgba(155, 231, 216, 0.45);
  border-radius: 10px;
  color: #b9f6dd;
  background: rgba(19, 48, 53, 0.82);
  cursor: pointer;
  font-weight: 700;
}

.secondary-button:disabled {
  cursor: wait;
  opacity: 0.55;
}

.source-library {
  min-width: 0;
  padding: 22px;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 16px;
  background: rgba(19, 27, 42, 0.72);
}

.source-link-button {
  padding: 3px 0;
  border: 0;
  color: #8fcfff;
  background: transparent;
  cursor: pointer;
  font-size: 11px;
}

.source-link-button.danger {
  color: #ffb0bc;
}

.source-list {
  display: grid;
  gap: 9px;
  margin-top: 20px;
}

.source-list-empty,
.source-inline-error,
.source-inline-success {
  color: #8391a6;
  font-size: 12px;
  line-height: 1.6;
}

.source-list-empty {
  margin-top: 28px;
}

.source-inline-error {
  color: #ffb0bc;
}

.source-inline-success {
  color: #b9f6dd;
}

.source-row {
  padding: 12px;
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 10px;
  background: rgba(12, 17, 27, 0.52);
  cursor: pointer;
}

.source-row.selected {
  border-color: rgba(121, 201, 255, 0.72);
  background: rgba(20, 44, 63, 0.72);
}

.source-row-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.source-select-control {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
}

.source-select-control input {
  width: 14px;
  height: 14px;
  accent-color: #86dfc2;
}

.source-row.checked {
  border-color: rgba(134, 223, 194, 0.52);
}

.source-row-heading strong {
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.source-row-heading span {
  color: #ffcf9b;
  font-size: 10px;
}

.source-row-heading span.enabled {
  color: #b9f6dd;
}

.source-library-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.source-batch-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 9px;
  margin-top: 15px;
  padding: 9px 11px;
  border: 1px solid rgba(121, 201, 255, 0.22);
  border-radius: 9px;
  color: #aebbd0;
  background: rgba(20, 44, 63, 0.45);
  font-size: 11px;
}

.source-batch-bar strong {
  color: #dce7f7;
}

.source-library-actions input {
  width: 120px;
  padding: 8px 10px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 8px;
  color: #dce7f7;
  background: #0c111b;
  font-size: 12px;
}

.source-row-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 8px;
  color: #8391a6;
  font-size: 10px;
}

.source-row-meta span {
  padding: 3px 6px;
  border: 1px solid rgba(148, 163, 184, 0.12);
  border-radius: 999px;
}

.source-row-actions {
  justify-content: flex-end;
  margin-top: 11px;
}

.source-debug {
  margin-top: 18px;
  padding: 22px;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 16px;
  background: rgba(19, 27, 42, 0.72);
}

.source-audit-panel {
  margin-top: 18px;
  padding: 22px;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 16px;
  background: rgba(19, 27, 42, 0.72);
}

.source-audit-grid {
  display: grid;
  grid-template-columns: minmax(240px, 0.7fr) minmax(0, 1.3fr);
  gap: 16px;
  margin-top: 18px;
}

.source-audit-card {
  min-width: 0;
  padding: 16px;
  border: 1px solid rgba(148, 163, 184, 0.13);
  border-radius: 12px;
  background: rgba(12, 17, 27, 0.48);
}

.source-audit-card strong {
  display: block;
  margin-top: 10px;
  font-size: 20px;
}

.source-audit-card p {
  margin: 8px 0 0;
  color: #8391a6;
  font-size: 12px;
  line-height: 1.6;
}

.source-audit-list {
  display: grid;
  gap: 10px;
}

.source-audit-row {
  padding: 11px;
  border: 1px solid rgba(148, 163, 184, 0.12);
  border-radius: 9px;
}

.source-audit-row > div {
  display: flex;
  justify-content: space-between;
  gap: 10px;
}

.source-audit-row span {
  color: #ffcf9b;
  font-size: 10px;
}

.source-audit-row span.enabled {
  color: #b9f6dd;
}

.source-audit-warning {
  color: #e3c788 !important;
}


.source-debug-heading {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 18px;
}

.source-debug-controls input {
  width: 180px;
  padding: 10px 12px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 9px;
  color: #dce7f7;
  background: #0c111b;
}

.source-debug-empty {
  margin: 22px 0 0;
  color: #8391a6;
  font-size: 12px;
}

.source-debug-summary {
  display: flex;
  align-items: baseline;
  gap: 12px;
  margin-top: 22px;
}

.source-debug-summary span {
  color: #8391a6;
  font-size: 12px;
}

.source-debug-steps {
  display: grid;
  gap: 9px;
  margin: 17px 0 0;
  padding: 0;
  list-style: none;
}

.source-debug-steps li {
  padding: 11px 12px;
  border: 1px solid rgba(148, 163, 184, 0.12);
  border-radius: 9px;
  background: rgba(12, 17, 27, 0.5);
}

.source-debug-steps li > div {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}

.source-debug-steps li span,
.source-debug-steps code {
  color: #8391a6;
  font-size: 11px;
}

.source-debug-steps code {
  display: block;
  margin-top: 7px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-debug-steps p {
  margin: 7px 0 0;
  color: #ffb0bc;
  font-size: 11px;
}

.source-section-heading {
  display: flex;
  align-items: start;
  justify-content: space-between;
  gap: 16px;
}

.source-section-heading h2 {
  margin: 9px 0 0;
  font-size: 20px;
}

.source-limit {
  color: #8391a6;
  font-size: 11px;
}

.source-editor textarea {
  display: block;
  width: 100%;
  min-height: 460px;
  margin-top: 22px;
  padding: 16px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 10px;
  color: #dce7f7;
  background: #0c111b;
  font: 13px/1.7 "Cascadia Code", Consolas, monospace;
  resize: vertical;
}

.source-editor textarea:focus {
  border-color: rgba(139, 183, 255, 0.75);
  outline: none;
}

.source-hint {
  margin: 13px 0 0;
  color: #8391a6;
  font-size: 12px;
  line-height: 1.6;
}

.source-metadata-panel {
  margin-top: 20px;
  padding: 18px;
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 12px;
  background: rgba(12, 17, 27, 0.42);
}

.source-meta-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-top: 16px;
}

.source-meta-field {
  display: grid;
  gap: 7px;
  color: #aebbd0;
  font-size: 12px;
}

.source-meta-field input:not([type="checkbox"]) {
  width: 100%;
  padding: 9px 10px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 8px;
  color: #dce7f7;
  background: #0c111b;
}

.source-meta-field input:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.source-meta-checkbox {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-top: 26px;
}

.source-meta-checkbox input {
  accent-color: #86dfc2;
}

.source-meta-wide {
  grid-column: 1 / -1;
}

.source-meta-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 15px;
}

.source-meta-actions span {
  color: #8391a6;
  font-size: 11px;
}

.source-result-empty {
  display: grid;
  justify-items: start;
  padding: 58px 0 18px;
}

.source-result-empty h3 {
  margin: 17px 0 7px;
}

.source-result-empty p {
  color: #8391a6;
  line-height: 1.7;
}

.validation-state {
  display: inline-flex;
  margin-top: 26px;
  padding: 8px 11px;
  border-radius: 999px;
  color: #ffb0bc;
  background: rgba(188, 59, 83, 0.16);
  font-size: 12px;
}

.validation-state.valid {
  color: #b9f6dd;
  background: rgba(30, 101, 82, 0.24);
}

.validation-list {
  margin-top: 22px;
}

.validation-list strong {
  font-size: 12px;
}

.validation-list p {
  margin: 9px 0 0;
  color: #ffb0bc;
  font-size: 12px;
  line-height: 1.6;
}

.validation-list.warnings p {
  color: #e3c788;
}

@media (max-width: 1100px) {
  .library-actions {
    justify-content: flex-start;
  }

  .source-grid {
    grid-template-columns: minmax(180px, 0.6fr) minmax(0, 1.4fr);
  }

  .source-result {
    grid-column: 1 / -1;
  }
}

@media (max-width: 900px) {
  .source-library-actions {
    align-items: flex-start;
    flex-direction: column;
  }

  .source-meta-grid {
    grid-template-columns: 1fr;
  }

  .source-meta-wide {
    grid-column: auto;
  }

  .source-meta-actions {
    align-items: flex-start;
    flex-direction: column;
  }

  .source-grid {
    grid-template-columns: 1fr;
  }

  .source-result {
    grid-column: auto;
  }

  .source-debug-heading {
    align-items: start;
    flex-direction: column;
  }

  .source-audit-grid {
    grid-template-columns: 1fr;
  }
}

.file-input {
  display: none;
}

.import-button,
.text-button {
  border: 0;
  border-radius: 10px;
  color: #07111f;
  background: linear-gradient(135deg, #9be7d8, #79c9ff);
  cursor: pointer;
  font-weight: 750;
}

.import-button {
  padding: 11px 16px;
}

.import-button:disabled {
  cursor: wait;
  opacity: 0.6;
}

.text-button {
  padding: 9px 13px;
}

.status-banner {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-top: 20px;
  color: #8d9bb0;
  font-size: 13px;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #79e3ba;
  box-shadow: 0 0 12px #79e3ba;
}

.error-text {
  overflow: hidden;
  color: #ff9eae;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.library-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(224px, 1fr));
  gap: 16px;
  margin-top: 24px;
}

.book-card {
  overflow: hidden;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 16px;
  background: rgba(19, 27, 42, 0.72);
  cursor: pointer;
  transition: border-color 160ms ease, transform 160ms ease;
}

.book-card:hover,
.book-card:focus-visible {
  border-color: rgba(139, 183, 255, 0.7);
  outline: none;
  transform: translateY(-2px);
}

.book-cover {
  display: flex;
  height: 132px;
  align-items: end;
  padding: 16px;
  color: rgba(255, 255, 255, 0.76);
  background: linear-gradient(145deg, #263c73, #17233e);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.14em;
}

.book-cover.format-epub {
  background: linear-gradient(145deg, #6f4a8e, #2c244f);
}

.book-card-body {
  padding: 16px;
}

.book-format,
.book-card-body p {
  color: #8391a6;
  font-size: 12px;
}

.book-card-body h2 {
  overflow: hidden;
  margin: 8px 0 4px;
  font-size: 17px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-card-body p {
  margin: 0;
}

.progress-track {
  height: 4px;
  margin-top: 17px;
  overflow: hidden;
  border-radius: 99px;
  background: rgba(148, 163, 184, 0.16);
}

.progress-track span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: #83e2c0;
}

.reader-content {
  width: min(1280px, 100%);
}

.reader-toolbar {
  display: flex;
  align-items: center;
  gap: 22px;
  margin-bottom: 22px;
}

.reader-heading {
  display: grid;
  min-width: 0;
  flex: 1;
  gap: 5px;
}

.reader-heading strong,
.reader-heading span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.reader-heading span {
  color: #8391a6;
  font-size: 12px;
}

.reader-controls {
  display: flex;
  align-items: center;
  gap: 12px;
}

.reader-controls label {
  display: flex;
  align-items: center;
  gap: 7px;
  color: #8c9ab0;
  font-size: 12px;
}

.reader-controls input {
  width: 72px;
  accent-color: #86dfc2;
}

.toolbar-button {
  padding: 9px 12px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 9px;
  color: #c6d1e2;
  background: rgba(19, 27, 42, 0.78);
  cursor: pointer;
}

.toolbar-button:hover:not(:disabled) {
  border-color: rgba(139, 183, 255, 0.7);
}

.toolbar-button:disabled {
  cursor: not-allowed;
  opacity: 0.35;
}

.reader-notices {
  display: grid;
  gap: 7px;
  margin: 0 0 14px;
}

.reader-notices p {
  margin: 0;
  padding: 9px 12px;
  border-radius: 9px;
  font-size: 12px;
  line-height: 1.5;
}

.reader-update-note {
  color: #b9f6dd;
  background: rgba(30, 101, 82, 0.24);
}

.reader-stale-note {
  color: #ffcf9b;
  background: rgba(139, 90, 34, 0.2);
}

.reader-layout {
  display: grid;
  grid-template-columns: 248px minmax(0, 1fr);
  min-height: 650px;
  overflow: hidden;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 18px;
  background: #111a2b;
}

.chapter-panel {
  overflow: auto;
  padding: 22px 12px;
  border-right: 1px solid rgba(148, 163, 184, 0.12);
}

.chapter-panel > .eyebrow {
  display: block;
  padding: 0 10px 14px;
}

.chapter-item {
  display: grid;
  grid-template-columns: 25px 1fr;
  gap: 8px;
  width: 100%;
  padding: 9px 10px;
  border: 0;
  border-radius: 8px;
  color: #91a0b5;
  background: transparent;
  cursor: pointer;
  text-align: left;
}

.chapter-item span {
  color: #61728b;
  font-variant-numeric: tabular-nums;
}

.chapter-item strong {
  overflow: hidden;
  font-size: 12px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chapter-item:hover,
.chapter-item.selected {
  color: #eff5ff;
  background: rgba(114, 149, 255, 0.16);
}

.reader-page {
  min-width: 0;
  padding: 56px clamp(28px, 6vw, 86px) 70px;
  color: #dfe7f2;
  width: min(100%, var(--reader-content-width));
  margin: 0 auto;
  font-family: var(--reader-font-family);
  font-size: var(--reader-font-size);
  line-height: var(--reader-line-height);
}

.reader-page h2 {
  margin: 9px 0 38px;
  color: #f5f8ff;
  font-size: 1.6em;
  letter-spacing: -0.035em;
  line-height: 1.25;
}

.reader-page p {
  margin: 0 0 var(--reader-paragraph-spacing);
  text-indent: var(--reader-text-indent);
  white-space: pre-wrap;
}

.reader-rich-block {
  margin: 0 0 var(--reader-paragraph-spacing);
  text-indent: var(--reader-text-indent);
  white-space: pre-wrap;
}

.reader-rich-heading {
  margin-top: 1.1em;
  color: #f5f8ff;
  font-size: 1.18em;
  font-weight: 700;
  line-height: 1.45;
  text-indent: 0;
}

.reader-rich-quote {
  padding: 0.45em 1em;
  border-left: 3px solid rgba(121, 201, 255, 0.55);
  color: #afbdd0;
  text-indent: 0;
}

.reader-rich-image {
  margin: 0 0 var(--reader-paragraph-spacing);
  padding: 0.72em 1em;
  border: 1px dashed rgba(148, 163, 184, 0.3);
  border-radius: 8px;
  color: #9eacc0;
  background: rgba(148, 163, 184, 0.08);
  text-align: center;
}

.reader-rich-image img {
  display: block;
  max-width: 100%;
  max-height: 520px;
  margin: 0 auto;
  object-fit: contain;
}

.theme-paper .reader-rich-heading {
  color: #3f3a34;
}

.theme-paper .reader-rich-quote,
.theme-paper .reader-rich-image {
  color: #756b5e;
  border-color: rgba(91, 76, 57, 0.35);
}

.theme-sepia .reader-rich-heading {
  color: #f3e7ce;
}

.theme-sepia .reader-rich-quote,
.theme-sepia .reader-rich-image {
  color: #d2c2a4;
}

.reader-meta {
  color: #71829a;
  font-size: 11px;
  letter-spacing: 0.1em;
}

.chapter-navigation {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 54px;
  padding-top: 22px;
  border-top: 1px solid rgba(148, 163, 184, 0.14);
  font-size: 12px;
}

.theme-paper .reader-layout {
  background: #f6f1e7;
}

.theme-paper .reader-page,
.theme-paper .reader-page h2 {
  color: #3f3a34;
}

.theme-paper .chapter-panel {
  border-color: rgba(91, 76, 57, 0.16);
  background: #ece4d4;
}

.theme-paper .chapter-item {
  color: #756b5e;
}

.theme-paper .chapter-item:hover,
.theme-paper .chapter-item.selected {
  color: #3f3a34;
  background: rgba(137, 111, 77, 0.15);
}

.theme-paper .reader-meta,
.theme-paper .reader-heading span {
  color: #877967;
}

.theme-sepia .reader-layout {
  background: #2c2922;
}

.theme-sepia .reader-page {
  color: #e9dcc0;
}

.theme-sepia .reader-page h2 {
  color: #f3e7ce;
}

@media (max-width: 1100px) {
  .reader-toolbar {
    align-items: start;
    flex-wrap: wrap;
  }

  .reader-controls {
    width: 100%;
    justify-content: flex-end;
  }
}

@media (max-width: 800px) {
  .reader-layout {
    grid-template-columns: 1fr;
  }

  .chapter-panel {
    max-height: 180px;
    border-right: 0;
    border-bottom: 1px solid rgba(148, 163, 184, 0.12);
  }
}

.settings-content {
  max-width: 1080px;
}

.settings-panel {
  margin-top: 28px;
  padding: 24px;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 16px;
  background: rgba(19, 27, 42, 0.72);
}

.settings-section-heading {
  display: flex;
  align-items: start;
  justify-content: space-between;
  gap: 18px;
}

.settings-section-heading h2 {
  margin: 9px 0 0;
  font-size: 20px;
}

.settings-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
  margin-top: 24px;
}

.settings-field {
  display: grid;
  gap: 8px;
  color: #aebbd0;
  font-size: 13px;
}

.settings-field > span {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}

.settings-field strong {
  color: #dce7f7;
  font-variant-numeric: tabular-nums;
}

.settings-field select,
.settings-field input[type="range"] {
  width: 100%;
}

.settings-field select {
  padding: 10px 12px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 9px;
  color: #dce7f7;
  background: #0c111b;
}

.settings-field input[type="range"] {
  accent-color: #86dfc2;
}

.settings-note {
  margin: 24px 0 0;
  color: #8391a6;
  font-size: 12px;
  line-height: 1.7;
}

@media (max-width: 720px) {
  .settings-grid {
    grid-template-columns: 1fr;
  }
}

.reader-page {
  box-sizing: border-box;
  margin-left: var(--reader-margin-left);
  margin-right: var(--reader-margin-right);
  letter-spacing: var(--reader-letter-spacing);
  text-align: var(--reader-text-align);
}

.reader-page h2,
.reader-rich-heading {
  text-align: var(--reader-text-align);
}

.reader-meta,
.chapter-navigation,
.reader-rich-image {
  text-align: left;
}

.reader-rich-block {
  text-align: var(--reader-text-align);
}

.reading-paged .reader-page {
  max-height: calc(100vh - 190px);
  overflow-y: auto;
  scroll-behavior: smooth;
  scrollbar-gutter: stable;
}

.theme-custom .reader-layout {
  border-color: var(--reader-custom-accent);
  background: var(--reader-custom-background);
}

.theme-custom .reader-page {
  color: var(--reader-custom-text);
  background: var(--reader-custom-background);
}

.theme-custom .reader-page h2,
.theme-custom .reader-rich-heading {
  color: var(--reader-custom-text);
}

.theme-custom .reader-meta,
.theme-custom .reader-heading span,
.theme-custom .reader-rich-quote,
.theme-custom .reader-rich-image {
  color: var(--reader-custom-text);
}

.theme-custom .reader-rich-quote,
.theme-custom .reader-rich-image {
  border-color: var(--reader-custom-accent);
}

.theme-custom .chapter-panel {
  border-color: var(--reader-custom-accent);
  background: var(--reader-custom-background);
}

.theme-custom .chapter-item:hover,
.theme-custom .chapter-item.selected {
  color: var(--reader-custom-background);
  background: var(--reader-custom-accent);
}

.nav-item:focus-visible,
.toolbar-button:focus-visible,
.secondary-button:focus-visible,
.import-button:focus-visible,
.text-button:focus-visible,
.source-link-button:focus-visible,
.chapter-item:focus-visible,
.reader-page:focus-visible {
  outline: 2px solid #79c9ff;
  outline-offset: 3px;
}

</style>
