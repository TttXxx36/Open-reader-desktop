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

interface NextPagePolicy {
  enabled: boolean;
  max_depth: number;
  max_pages: number;
  max_bytes: number;
  max_duration_secs: number;
  same_host_only: boolean;
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

interface UnsupportedImportRule {
  context: string;
  value: string;
  reason: string;
  offline_accepted: boolean;
  offline_syntax: string;
  offline_steps: number;
  offline_estimated_work: number;
  offline_elapsed_us: number;
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
  unsupported_rules: UnsupportedImportRule[];
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

interface SourceImportResult {
  imported: SourceSummary[];
  snapshot_id: string;
  skipped: number;
}

interface SourceSnapshotSummary {
  id: string;
  label: string;
  source_count: number;
  created_at: string;
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

interface SourceFailureHistory {
  id: string;
  source_id: string;
  source_name: string;
  stage: string;
  reason_code: string;
  operation_id: string | null;
  message: string;
  created_at: string;
}

interface SourceFailureStats {
  total: number;
  by_reason: Array<{ code: string; count: number }>;
  by_stage: Array<{ code: string; count: number }>;
}

interface SourceRequestMetric {
  stage: string;
  attempts: number;
  successes: number;
  failures: number;
  cache_hits: number;
  failure_rate: number;
  cache_hit_rate: number;
}

interface SourceRequestMetrics {
  total_attempts: number;
  total_successes: number;
  total_failures: number;
  total_cache_hits: number;
  failure_rate: number;
  cache_hit_rate: number;
  by_stage: SourceRequestMetric[];
}

interface SourceMetrics {
  total_sources: number;
  enabled_sources: number;
  audited_sources: number;
  audit_pass: number;
  audit_attention: number;
  failure_events: number;
  cache_entries: number;
  cache_bytes: number;
  request_metrics: SourceRequestMetrics | null;
}

interface SourceDebugStep {
  stage: string;
  url: string;
  duration_ms: number;
  status: number | null;
  bytes: number | null;
  error: string | null;
  variables: Record<string, string>;
  cache_hit: boolean;
}

interface SourcePipelineResult {
  search_results: Array<{ title: string; author: string | null; book_url: string | null; source_name: string }>;
  book_info: { title: string; author: string | null; intro: string | null; cover_url: string | null; book_url: string };
  chapters: Array<{ title: string; url: string; index: number }>;
  first_chapter: { title: string; content: string; next_url: string | null };
  debug_steps: SourceDebugStep[];
}

interface ExportedDiagnosticStep extends SourceDebugStep {
  order: number;
  start_ms: number;
}

interface SourceDiagnosticSnapshot {
  schema_version: 1;
  generated_at: string;
  source_name: string;
  timeline_basis: "relative_monotonic_ms";
  summary: {
    search_results: number;
    chapters: number;
    request_steps: number;
    failed_steps: number;
    cache_hits: number;
    cache_events: number;
    total_duration_ms: number;
  };
  next_page_policy: {
    enabled: boolean;
    max_depth: number;
    max_pages: number;
    max_bytes: number;
    max_duration_secs: number;
    same_host_only: boolean;
    stop_reason: string | null;
    status_label: string;
    status_detail: string;
  };
  multi_source: {
    enabled_sources: number;
    result_count: number;
    failures: Array<{ source_name: string; message: string }>;
    diagnostics: Array<{
      source_name: string;
      pages_scanned: number;
      parsed_items: number;
      stop_reason: string;
    }>;
  } | null;
  cache: SourceCacheStatus | null;
  source_metrics: SourceMetrics;
  steps: ExportedDiagnosticStep[];
  truncated_steps: boolean;
  privacy: string[];
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
  cache_hit: boolean;
}

interface RemoteChapterContent {
  title: string;
  content: string;
  next_url: string | null;
  stale: boolean;
  refresh_error: string | null;
  cache_hit: boolean;
  debug_steps: SourceDebugStep[];
}

interface RemoteNextPageStatus {
  label: string;
  detail: string;
  reason: string | null;
}

interface SourceSearchDiagnostics {
  source_id: string;
  source_name: string;
  pages_scanned: number;
  parsed_items: number;
  stop_reason: string;
}

interface MultiSourceSearchResult {
  results: UnifiedSearchItem[];
  failures: Array<{
    source_id: string;
    source_name: string;
    message: string;
  }>;
  diagnostics: SourceSearchDiagnostics[];
  enabled_sources: number;
}

const SETTINGS_KEY = "open-reader.settings";
const SETTINGS_VERSION = 2;
const NEXT_PAGE_POLICY_KEY = "open-reader.next-page-policy";
const DEFAULT_NEXT_PAGE_POLICY: NextPagePolicy = {
  enabled: false,
  max_depth: 2,
  max_pages: 3,
  max_bytes: 2 * 1024 * 1024,
  max_duration_secs: 15,
  same_host_only: true,
};
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
const nextPagePolicy = ref<NextPagePolicy>(loadNextPagePolicy());
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
const sourceBatchGroup = ref("");
const sourceSnapshots = ref<SourceSnapshotSummary[]>([]);
const sourceImportStrategy = ref<"update" | "skip-existing" | "new">("update");
const sourceImportSnapshotId = ref<string | null>(null);
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
const sourcePipelineOperationId = ref<string | null>(null);
const sourceKeyword = ref("demo");
const sourcePipeline = ref<SourcePipelineResult | null>(null);
const searchKeyword = ref("");
const searchPageLimit = ref(1);
const searchBusy = ref(false);
const searchOperationId = ref<string | null>(null);
const searchResult = ref<MultiSourceSearchResult | null>(null);
const retryingSourceId = ref<string | null>(null);
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
const sourceFailureHistory = ref<SourceFailureHistory[] | null>(null);
const sourceFailureHistoryBusy = ref(false);
const sourceFailureStats = ref<SourceFailureStats | null>(null);
const sourceRequestMetrics = ref<SourceRequestMetrics | null>(null);
const sourceMetrics = computed<SourceMetrics>(() => {
  const audits = sourceAudit.value ?? [];
  return {
    total_sources: sources.value.length,
    enabled_sources: sources.value.filter((source) => source.enabled).length,
    audited_sources: audits.length,
    audit_pass: audits.filter((audit) => audit.pass && audit.warnings.length === 0).length,
    audit_attention: audits.filter((audit) => audit.pass && audit.warnings.length > 0).length,
    failure_events: sourceFailureStats.value?.total ?? 0,
    cache_entries: sourceCacheStatus.value?.entries ?? 0,
    cache_bytes: sourceCacheStatus.value?.bytes ?? 0,
    request_metrics: sourceRequestMetrics.value,
  };
});
const remoteBusy = ref(false);
const remoteOperationId = ref<string | null>(null);
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
const remoteNextPageStatus = computed<RemoteNextPageStatus>(() => {
  const content = remoteChapter.value;
  if (!nextPagePolicy.value.enabled) {
    return {
      label: "自动追链已关闭",
      detail: content?.next_url ? "后续链接已保留，不会发起额外请求。" : "本章没有待追踪的后续链接。",
      reason: "disabled",
    };
  }
  const policyStep = [...(content?.debug_steps ?? [])]
    .reverse()
    .find((step) => step.stage === "content.next.policy");
  const reason = policyStep?.error?.match(/next URL\s+([a-z_]+)/)?.[1] ?? null;
  if (reason) {
    return {
      label: "追链已停止：" + nextPageStopLabel(reason),
      detail: content?.next_url ? "后续链接已保留，可手动刷新或关闭追链。" : "已保留已获取正文。",
      reason,
    };
  }
  return {
    label: content?.next_url ? "追链已完成当前配额" : "追链已完成",
    detail: content?.next_url ? "仍有后续链接，但不会超过安全配额。" : "没有待处理的后续链接。",
    reason: null,
  };
});
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
watch(nextPagePolicy, (value) => {
  try {
    localStorage.setItem(NEXT_PAGE_POLICY_KEY, JSON.stringify(value));
  } catch {
    // 策略设置无法持久化时仍保持本次会话状态。
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

function paginationStopLabel(reason: string) {
  return ({
    empty_page: "遇到空页",
    no_new_results: "没有新增结果",
    max_pages: "达到页数上限",
    request_failed: "请求失败",
  } as Record<string, string>)[reason] ?? reason;
}

function nextPageStopLabel(reason: string) {
  return ({
    disabled: "未启用",
    depth_limit: "达到深度上限",
    page_limit: "达到页面上限",
    byte_limit: "达到响应体上限",
    time_limit: "达到时间上限",
    same_origin: "跨源候选已拒绝",
    cycle: "检测到环路",
    invalid_next_url: "后续链接无效",
    invalid_base_url: "基准链接无效",
    quota_zero: "配额为零",
    request_error: "后续请求失败",
    parse_error: "后续页面解析失败",
  } as Record<string, string>)[reason] ?? reason;
}

function normalizeHex(value: unknown, fallback: string) {
  return typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value) ? value.toLowerCase() : fallback;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function loadNextPagePolicy(): NextPagePolicy {
  try {
    const saved = JSON.parse(localStorage.getItem(NEXT_PAGE_POLICY_KEY) ?? "{}") as unknown;
    const payload = isRecord(saved) ? saved : {};
    return {
      ...DEFAULT_NEXT_PAGE_POLICY,
      enabled: payload.enabled === true,
      max_depth: clampNumber(payload.max_depth, DEFAULT_NEXT_PAGE_POLICY.max_depth, 1, 2),
      max_pages: clampNumber(payload.max_pages, DEFAULT_NEXT_PAGE_POLICY.max_pages, 1, 3),
      max_bytes: clampNumber(payload.max_bytes, DEFAULT_NEXT_PAGE_POLICY.max_bytes, 64 * 1024, 2 * 1024 * 1024),
      max_duration_secs: clampNumber(payload.max_duration_secs, DEFAULT_NEXT_PAGE_POLICY.max_duration_secs, 1, 15),
      same_host_only: payload.same_host_only !== false,
    };
  } catch {
    return { ...DEFAULT_NEXT_PAGE_POLICY };
  }
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
  await Promise.all([loadSources(), loadSourceSnapshots(), refreshSourceCacheStatus(), loadSourceFailureHistory(), loadSourceFailureStats(), loadSourceRequestMetrics()]);
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

function resetNextPagePolicy() {
  nextPagePolicy.value = { ...DEFAULT_NEXT_PAGE_POLICY };
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

async function loadSourceSnapshots() {
  try {
    sourceSnapshots.value = await invoke<SourceSnapshotSummary[]>("list_source_snapshots");
  } catch (error) {
    errorMessage.value = String(error);
  }
}

async function loadSourceFailureHistory() {
  sourceFailureHistoryBusy.value = true;
  try {
    sourceFailureHistory.value = await invoke<SourceFailureHistory[]>("list_source_failure_history", {
      sourceId: sourceId.value,
      limit: 64,
    });
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceFailureHistoryBusy.value = false;
  }
}

async function loadSourceRequestMetrics() {
  try {
    sourceRequestMetrics.value = await invoke<SourceRequestMetrics>("get_source_request_metrics");
  } catch (error) {
    errorMessage.value = String(error);
  }
}

async function loadSourceFailureStats() {
  try {
    sourceFailureStats.value = await invoke<SourceFailureStats>("get_source_failure_stats");
  } catch (error) {
    errorMessage.value = String(error);
  }
}

async function clearSourceFailureHistory() {
  const scope = sourceId.value ? "当前书源" : "全部书源";
  if (!window.confirm(`确定清空${scope}的失败历史吗？`)) return;
  sourceFailureHistoryBusy.value = true;
  try {
    await invoke<number>("clear_source_failure_history", { sourceId: sourceId.value });
    await loadSourceFailureHistory();
    await loadSourceFailureStats();
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceFailureHistoryBusy.value = false;
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

function formatPercent(value: number) {
  const normalized = Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0;
  return `${Math.round(normalized * 100)}%`;
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
  void loadSourceFailureHistory();
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
  sourceFailureHistory.value = null;
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

async function applySourceBatch(action: "enable" | "disable" | "explore-on" | "explore-off" | "group" | "delete") {
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
    } else if (action === "group") {
      const groupName = sourceBatchGroup.value.trim();
      if (!groupName) {
        errorMessage.value = "请输入目标分组";
        return;
      }
      await invoke("set_sources_group", {
        sourceIds,
        groupName,
      });
      sourceBatchGroup.value = "";
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
  if (!keyword || searchBusy.value || retryingSourceId.value) return;

  const maxPages = Math.min(20, Math.max(1, Math.trunc(searchPageLimit.value || 1)));
  searchPageLimit.value = maxPages;
  const operationId = "search-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 10);
  searchOperationId.value = operationId;
  searchBusy.value = true;
  searchResult.value = null;
  sourceTransferMessage.value = "";
  errorMessage.value = "";
  try {
    searchResult.value = await invoke<MultiSourceSearchResult>("search_sources", {
      keyword,
      maxPages,
      operationId,
    });
  } catch (error) {
    const message = String(error);
    if (message.includes("已取消")) {
      sourceTransferMessage.value = "多源搜索已取消";
    } else {
      errorMessage.value = message;
    }
  } finally {
    if (searchOperationId.value === operationId) {
      searchOperationId.value = null;
    }
    searchBusy.value = false;
  }
}

async function retrySourceSearch(sourceId: string) {
  const keyword = searchKeyword.value.trim();
  if (!keyword || searchBusy.value || retryingSourceId.value) return;

  const maxPages = Math.min(20, Math.max(1, Math.trunc(searchPageLimit.value || 1)));
  searchPageLimit.value = maxPages;
  const operationId = "retry-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 10);
  retryingSourceId.value = sourceId;
  searchOperationId.value = operationId;
  searchBusy.value = true;
  sourceTransferMessage.value = "";
  errorMessage.value = "";

  try {
    const retryResult = await invoke<MultiSourceSearchResult>("retry_source_search", {
      sourceId,
      keyword,
      maxPages,
      operationId,
    });
    const current = searchResult.value;
    if (!current) {
      searchResult.value = retryResult;
    } else {
      const retainedResults = current.results.filter((item) => item.source_id !== sourceId);
      const seen = new Set<string>();
      const mergedResults = [...retainedResults, ...retryResult.results].filter((item) => {
        const key = item.source_id + "\u0000" + item.title + "\u0000" + (item.author || "");
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });
      searchResult.value = {
        ...current,
        results: mergedResults,
        failures: [
          ...current.failures.filter((failure) => failure.source_id !== sourceId),
          ...retryResult.failures,
        ],
        diagnostics: [
          ...current.diagnostics.filter((diagnostic) => diagnostic.source_id !== sourceId),
          ...retryResult.diagnostics,
        ],
      };
    }
    sourceTransferMessage.value = retryResult.failures.length
      ? "书源重试仍失败，已保留最新失败原因"
      : "书源重试成功，已更新该书源结果";
  } catch (error) {
    const message = String(error);
    if (message.includes("已取消")) {
      sourceTransferMessage.value = "书源重试已取消";
    } else {
      errorMessage.value = message;
    }
  } finally {
    if (searchOperationId.value === operationId) {
      searchOperationId.value = null;
    }
    retryingSourceId.value = null;
    searchBusy.value = false;
  }
}

async function cancelSearch() {
  const operationId = searchOperationId.value;
  if (!operationId) return;

  try {
    await invoke<boolean>("cancel_source_operation", { operationId });
    sourceTransferMessage.value = "正在取消多源搜索…";
  } catch (error) {
    errorMessage.value = String(error);
  }
}

function clearSearch() {
  searchResult.value = null;
  searchKeyword.value = "";
}

async function finishSourceImport(result: SourceImportResult, label: string) {
  await loadSources();
  await loadSourceSnapshots();
  sourceImportSnapshotId.value = result.snapshot_id;
  if (result.imported[0]) {
    selectSource(result.imported[0]);
  }
  const skipped = result.skipped ? `，跳过 ${result.skipped} 个冲突项` : "";
  sourceTransferMessage.value = "已从" + label + "导入 " + result.imported.length + " 个书源" + skipped;
}

async function restoreSourceSnapshot(snapshotId = sourceImportSnapshotId.value ?? sourceSnapshots.value[0]?.id) {
  if (!snapshotId) {
    errorMessage.value = "当前没有可恢复的书源快照";
    return;
  }
  if (!window.confirm("恢复快照会替换当前全部书源，确定继续吗？")) return;

  sourceTransferBusy.value = true;
  errorMessage.value = "";
  try {
    await invoke<SourceSummary[]>("restore_source_snapshot", { snapshotId });
    sourceImportSnapshotId.value = null;
    await Promise.all([loadSources(), loadSourceSnapshots()]);
    newSourceDraft();
    sourceTransferMessage.value = "已恢复书源快照";
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceTransferBusy.value = false;
  }
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
    const result = await invoke<SourceImportResult>("import_sources_selected", {
      bundleJson: payload,
      indices,
      conflictStrategy: sourceImportStrategy.value,
    });
    await finishSourceImport(result, label);
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

  const operationId = "remote-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 10);
  remoteOperationId.value = operationId;
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
      operationId,
    });
    const firstChapter = loaded.chapters[0];
    if (!firstChapter) {
      throw new Error("书源未返回可阅读章节");
    }

    const firstContent = await invoke<RemoteChapterContent>("fetch_source_chapter", {
      sourceId: loaded.source_id,
      chapter: firstChapter,
      forceRefresh: false,
      nextPagePolicy: nextPagePolicy.value,
      operationId,
    });
    remoteBook.value = loaded;
    remoteChapterRef.value = firstChapter;
    remoteChapter.value = firstContent;
    searchResult.value = null;
    view.value = "reader";
  } catch (error) {
    const message = String(error);
    if (message.includes("已取消")) {
      sourceTransferMessage.value = "远端打开已取消";
    } else {
      errorMessage.value = message;
    }
    remoteBook.value = null;
    remoteChapter.value = null;
    remoteChapterRef.value = null;
  } finally {
    if (remoteOperationId.value === operationId) {
      remoteOperationId.value = null;
    }
    remoteBusy.value = false;
  }
}

async function loadRemoteChapter(chapterItem: RemoteChapter, forceRefresh = false) {
  if (!remoteBook.value || remoteBusy.value) return;

  const operationId = "chapter-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 10);
  remoteOperationId.value = operationId;
  remoteBusy.value = true;
  errorMessage.value = "";
  try {
    remoteChapter.value = await invoke<RemoteChapterContent>("fetch_source_chapter", {
      sourceId: remoteBook.value.source_id,
      chapter: chapterItem,
      forceRefresh: forceRefresh || nextPagePolicy.value.enabled,
      nextPagePolicy: nextPagePolicy.value,
      operationId,
    });
    remoteChapterRef.value = chapterItem;
  } catch (error) {
    const message = String(error);
    if (message.includes("已取消")) {
      sourceTransferMessage.value = "章节加载已取消";
    } else {
      errorMessage.value = message;
    }
  } finally {
    if (remoteOperationId.value === operationId) {
      remoteOperationId.value = null;
    }
    remoteBusy.value = false;
  }
}

async function refreshRemoteBook() {
  if (!remoteBook.value || remoteBusy.value) return;

  const operationId = "refresh-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 10);
  remoteOperationId.value = operationId;
  const currentUrl = remoteChapterRef.value?.url;
  remoteBusy.value = true;
  errorMessage.value = "";
  try {
    const loaded = await invoke<RemoteBookDetail>("fetch_source_book", {
      sourceId: remoteBook.value.source_id,
      bookUrl: remoteBook.value.book_info.book_url,
      forceRefresh: true,
      operationId,
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
      nextPagePolicy: nextPagePolicy.value,
      operationId,
    });
    remoteBook.value = loaded;
    remoteChapterRef.value = target;
    remoteChapter.value = content;
  } catch (error) {
    const message = String(error);
    if (message.includes("已取消")) {
      sourceTransferMessage.value = "远端刷新已取消";
    } else {
      errorMessage.value = message;
    }
  } finally {
    if (remoteOperationId.value === operationId) {
      remoteOperationId.value = null;
    }
    remoteBusy.value = false;
  }
}

async function cancelRemoteOperation() {
  const operationId = remoteOperationId.value;
  if (!operationId) return;

  try {
    await invoke<boolean>("cancel_source_operation", { operationId });
    sourceTransferMessage.value = "正在取消远端请求…";
  } catch (error) {
    errorMessage.value = String(error);
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
  if (sourcePipelineBusy.value) return;

  const operationId = "pipeline-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 10);
  sourcePipelineOperationId.value = operationId;
  sourcePipelineBusy.value = true;
  sourcePipeline.value = null;
  sourceTransferMessage.value = "";
  errorMessage.value = "";
  try {
    sourcePipeline.value = await invoke<SourcePipelineResult>("run_source_pipeline", {
      configJson: sourceJson.value,
      keyword: sourceKeyword.value.trim(),
      operationId,
    });
  } catch (error) {
    const message = String(error);
    if (message.includes("已取消")) {
      sourceTransferMessage.value = "书源调试已取消";
    } else {
      errorMessage.value = message;
    }
  } finally {
    if (sourcePipelineOperationId.value === operationId) {
      sourcePipelineOperationId.value = null;
    }
    sourcePipelineBusy.value = false;
  }
}

async function cancelSourcePipeline() {
  const operationId = sourcePipelineOperationId.value;
  if (!operationId) return;

  try {
    await invoke<boolean>("cancel_source_operation", { operationId });
    sourceTransferMessage.value = "正在取消书源调试…";
  } catch (error) {
    errorMessage.value = String(error);
  }
}

const MAX_DIAGNOSTIC_STEPS = 256;
const MAX_DIAGNOSTIC_BYTES = 256 * 1024;

function truncateDiagnostic(value: string, limit: number) {
  const normalized = value.replace(/[\u0000-\u001f\u007f]/g, " ");
  return normalized.length > limit ? normalized.slice(0, limit) + "…" : normalized;
}

function sanitizeDiagnosticUrl(value: string) {
  try {
    const parsed = new URL(value);
    parsed.search = "";
    parsed.hash = "";
    return truncateDiagnostic(parsed.origin + parsed.pathname, 512);
  } catch {
    return truncateDiagnostic(value.replace(/[?#].*$/, ""), 512);
  }
}

function sanitizeDiagnosticStep(step: SourceDebugStep, prefix = ""): SourceDebugStep {
  return {
    stage: truncateDiagnostic(prefix + step.stage, 128),
    url: sanitizeDiagnosticUrl(step.url),
    duration_ms: Number.isFinite(step.duration_ms) ? Math.max(0, Math.round(step.duration_ms)) : 0,
    status: typeof step.status === "number" ? step.status : null,
    bytes: typeof step.bytes === "number" ? Math.max(0, Math.round(step.bytes)) : null,
    error: step.error ? truncateDiagnostic(step.error, 512) : null,
    variables: Object.fromEntries(
      Object.keys(step.variables ?? {})
        .slice(0, 32)
        .map((key) => [truncateDiagnostic(key, 64), "<redacted>"]),
    ),
    cache_hit: Boolean(step.cache_hit),
  };
}

function sanitizeDiagnosticMessage(value: string) {
  return truncateDiagnostic(
    value.replace(/https?:\/\/[^\s]+/g, (match) => sanitizeDiagnosticUrl(match)),
    512,
  );
}

function cacheDiagnosticStep(stage: string, url: string, error: string | null): SourceDebugStep {
  return {
    stage,
    url: sanitizeDiagnosticUrl(url),
    duration_ms: 0,
    status: null,
    bytes: null,
    error: error ? sanitizeDiagnosticMessage(error) : null,
    variables: {},
    cache_hit: true,
  };
}

function failureDiagnosticStep(stage: string, message: string): SourceDebugStep {
  return {
    stage: truncateDiagnostic(stage, 128),
    url: "",
    duration_ms: 0,
    status: null,
    bytes: null,
    error: sanitizeDiagnosticMessage(message),
    variables: {},
    cache_hit: false,
  };
}

function exportSourceDiagnostics() {
  const pipeline = sourcePipeline.value;
  if (!pipeline && !remoteBook.value && !remoteChapter.value && !searchResult.value) {
    errorMessage.value = "请先运行书源调试、搜索或打开远端章节";
    return;
  }

  const sourceValue = sourceValidation.value?.source;
  const sourceName = isRecord(sourceValue) && typeof sourceValue.name === "string"
    ? truncateDiagnostic(sourceValue.name, 128)
    : remoteBook.value?.source_name
      || (searchResult.value ? "多源搜索" : "当前书源");
  const searchFailureEvents = (searchResult.value?.failures ?? [])
    .slice(0, 64)
    .map((failure) => failureDiagnosticStep(
      "search." + truncateDiagnostic(failure.source_name, 96) + ".failure",
      failure.message,
    ));
  const rawSteps = [
    ...(pipeline?.debug_steps ?? []).map((step) => ({ step, prefix: "pipeline." })),
    ...(remoteBook.value?.debug_steps ?? []).map((step) => ({ step, prefix: "book." })),
    ...(remoteChapter.value?.debug_steps ?? []).map((step) => ({ step, prefix: "chapter." })),
  ];
  const cacheEvents = [
    ...(remoteBook.value?.cache_hit || remoteBook.value?.stale
      ? [cacheDiagnosticStep(
          remoteBook.value.stale ? "book.stale_fallback" : "book.cache_hit",
          remoteBook.value.book_info.book_url,
          remoteBook.value.refresh_error,
        )]
      : []),
    ...(remoteChapter.value?.cache_hit || remoteChapter.value?.stale
      ? [cacheDiagnosticStep(
          remoteChapter.value.stale ? "chapter.stale_fallback" : "chapter.cache_hit",
          remoteChapterRef.value?.url || "",
          remoteChapter.value.refresh_error,
        )]
      : []),
  ];
  const sanitizedSteps = [
    ...searchFailureEvents,
    ...rawSteps.map(({ step, prefix }) => sanitizeDiagnosticStep(step, prefix)),
    ...cacheEvents,
  ];
  let elapsedMs = 0;
  const steps: ExportedDiagnosticStep[] = sanitizedSteps
    .slice(0, MAX_DIAGNOSTIC_STEPS)
    .map((step, index) => {
      const positioned: ExportedDiagnosticStep = {
        ...step,
        order: index + 1,
        start_ms: elapsedMs,
      };
      elapsedMs += step.duration_ms;
      return positioned;
    });
  const snapshot: SourceDiagnosticSnapshot = {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    source_name: sourceName,
    timeline_basis: "relative_monotonic_ms",
    summary: {
      search_results: pipeline?.search_results.length ?? searchResult.value?.results.length ?? 0,
      chapters: pipeline?.chapters.length ?? remoteBook.value?.chapters.length ?? 0,
      request_steps: steps.length,
      failed_steps: steps.filter((step) => Boolean(step.error)).length,
      cache_hits: steps.filter((step) => step.cache_hit).length,
      cache_events: cacheEvents.length,
      total_duration_ms: steps.reduce((total, step) => total + step.duration_ms, 0),
    },
    next_page_policy: {
      ...nextPagePolicy.value,
      stop_reason: remoteNextPageStatus.value.reason,
      status_label: remoteNextPageStatus.value.label,
      status_detail: remoteNextPageStatus.value.detail,
    },
    multi_source: searchResult.value ? {
      enabled_sources: searchResult.value.enabled_sources,
      result_count: searchResult.value.results.length,
      failures: searchResult.value.failures.slice(0, 64).map((failure) => ({
        source_name: truncateDiagnostic(failure.source_name, 128),
        message: sanitizeDiagnosticMessage(failure.message),
      })),
      diagnostics: searchResult.value.diagnostics.slice(0, 64).map((diagnostic) => ({
        source_name: truncateDiagnostic(diagnostic.source_name, 128),
        pages_scanned: Math.max(0, Math.round(diagnostic.pages_scanned)),
        parsed_items: Math.max(0, Math.round(diagnostic.parsed_items)),
        stop_reason: truncateDiagnostic(diagnostic.stop_reason, 64),
      })),
    } : null,
    cache: sourceCacheStatus.value ? { ...sourceCacheStatus.value } : null,
    source_metrics: { ...sourceMetrics.value },
    steps,
    truncated_steps: sanitizedSteps.length > steps.length,
    privacy: [
      "不导出关键词、书籍/章节 ID、请求头、Cookie 或正文",
      "URL 查询参数和片段已移除",
      "变量值统一替换为 <redacted>",
      "多源搜索只导出失败原因与停止统计，不导出搜索关键词",
      "分页追链只导出启用状态、安全配额和停止原因，不导出后续 URL",
    ],
  };
  const payload = JSON.stringify(snapshot, null, 2);
  if (payload.length > MAX_DIAGNOSTIC_BYTES) {
    errorMessage.value = "诊断快照超过 256 KB 限制，请减少调试步骤后重试";
    return;
  }

  const blob = new Blob([payload], { type: "application/json;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = "open-reader-diagnostics-" + new Date().toISOString().slice(0, 10) + ".json";
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
  sourceTransferMessage.value = "已导出脱敏诊断快照（" + steps.length + " 个步骤）";
}

function exportSourceFailureReport() {
  const history = (sourceFailureHistory.value ?? []).slice(0, 64).map((failure) => ({
    id: truncateDiagnostic(failure.id, 128),
    source_id: truncateDiagnostic(failure.source_id, 128),
    source_name: truncateDiagnostic(failure.source_name, 128),
    stage: truncateDiagnostic(failure.stage, 64),
    reason_code: truncateDiagnostic(failure.reason_code, 64),
    operation_id: failure.operation_id ? truncateDiagnostic(failure.operation_id, 128) : null,
    message: sanitizeDiagnosticMessage(failure.message),
    created_at: truncateDiagnostic(failure.created_at, 64),
  }));
  const stats = sourceFailureStats.value
    ? {
        total: Math.max(0, Math.round(sourceFailureStats.value.total)),
        by_reason: sourceFailureStats.value.by_reason.slice(0, 32).map((item) => ({
          code: truncateDiagnostic(item.code, 64),
          count: Math.max(0, Math.round(item.count)),
        })),
        by_stage: sourceFailureStats.value.by_stage.slice(0, 32).map((item) => ({
          code: truncateDiagnostic(item.code, 64),
          count: Math.max(0, Math.round(item.count)),
        })),
      }
    : null;
  const report = {
    schema_version: 1,
    report_type: "source_failure_history",
    generated_at: new Date().toISOString(),
    scope: sourceId.value ? "current_source" : "all_sources",
    source_id: sourceId.value ? truncateDiagnostic(sourceId.value, 128) : null,
    source_metrics: { ...sourceMetrics.value },
    stats,
    entries: history,
    truncated_entries: (sourceFailureHistory.value?.length ?? 0) > history.length,
    privacy: [
      "仅导出本机失败摘要，不上传或包含关键词、正文、Cookie、请求头",
      "失败消息中的 URL 查询参数和片段已移除",
      "任务 ID 仅用于关联同一次本地请求，不代表账号或身份信息",
    ],
  };
  const payload = JSON.stringify(report, null, 2);
  if (payload.length > MAX_DIAGNOSTIC_BYTES) {
    errorMessage.value = "失败报告超过 256 KB 限制，请先清理历史后重试";
    return;
  }

  const blob = new Blob([payload], { type: "application/json;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = "open-reader-source-failures-" + new Date().toISOString().slice(0, 10) + ".json";
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
  sourceTransferMessage.value = "已导出脱敏失败报告（" + history.length + " 条）";
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

provide("open-reader-context", { SETTINGS_KEY, SETTINGS_VERSION, DEFAULT_READER_SETTINGS, readerFontStacks, view, books, recentBooks, continueBook, detail, chapter, fileInput, sourceImportInput, status, errorMessage, isImporting, settings, sourceBusy, sourceValidation, sources, filteredSources, sourceGroupFilter, sourceGroupDraft, sourceWeightDraft, sourceOrderDraft, sourceExploreDraft, sourceCommentDraft, selectedSourceIds, sourceBatchBusy, sourceBatchGroup, allFilteredSourcesSelected, sourceId, sourceListBusy, sourcePipelineBusy, sourceKeyword, sourcePipeline, searchKeyword, searchPageLimit, searchBusy, searchResult, sourceTransferBusy, sourceTransferMessage, sourceImportUrl, sourceImportPreview, sourceImportPayload, sourceImportLabel, sourceImportStrategy, sourceSnapshots, sourceImportSnapshotId, retryingSourceId, sourceAuditBusy, sourceAudit, sourceCacheBusy, sourceCacheStatus, sourceFailureHistory, sourceFailureHistoryBusy, sourceFailureStats, sourceRequestMetrics, sourceMetrics, remoteBusy, remoteBook, remoteChapter, remoteChapterRef, nextPagePolicy, remoteNextPageStatus, sourceJson, chapterParagraphs, chapterBlocks, remoteChapterParagraphs, readerStyle, themeLabels, parseContentBlocks, contentBlockTag, clampNumber, normalizeHex, isRecord, loadSettings, loadNextPagePolicy, loadBooks, openSources, openSettings, closeSettings, resetSettings, resetNextPagePolicy, loadSources, loadSourceSnapshots, runSourceAudit, refreshSourceCacheStatus, loadSourceFailureHistory, clearSourceFailureHistory, loadSourceFailureStats, loadSourceRequestMetrics, formatBytes, formatPercent, selectSource, newSourceDraft, saveSource, saveSourceMetadata, toggleSource, toggleSourceExplore, toggleSourceSelection, toggleSelectAllSources, applySourceBatch, reorderSource, deleteSource, searchSources, retrySourceSearch, cancelSearch, clearSearch, finishSourceImport, exportSources, openSourceImportPicker, showSourceImportPreview, clearSourceImportPreview, confirmSourceImport, restoreSourceSnapshot, importSourceUrl, importSourceFile, openRemoteBook, loadRemoteChapter, cancelRemoteOperation, remoteChapterIndex, goToRemoteChapter, previousRemoteChapter, nextRemoteChapter, runSourcePipeline, cancelSourcePipeline, exportSourceDiagnostics, exportSourceFailureReport, validateSource, openFilePicker, importFile, openBook, loadChapter, saveProgress, continueReading, closeReader, cycleTheme, formatProgress, currentChapterIndex, goToChapter, previousChapter, nextChapter });
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
          <label class="search-page-limit">
            <span>页数</span>
            <input v-model.number="searchPageLimit" type="number" min="1" max="20" aria-label="搜索页数上限" />
          </label>
          <button class="secondary-button" type="button" :disabled="searchBusy || !searchKeyword.trim()" @click="searchSources">
            {{ searchBusy ? "搜索中…" : "搜索书源" }}
          </button>
          <button v-if="searchBusy" class="secondary-button" type="button" @click="cancelSearch">
            取消搜索
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
          已查询 {{ searchResult.enabled_sources }} 个启用书源，最多扫描 {{ searchPageLimit }} 页，去重后 {{ searchResult.results.length }} 条结果。
        </p>
        <ul v-if="searchResult.diagnostics.length" class="search-diagnostics">
          <li v-for="diagnostic in searchResult.diagnostics" :key="diagnostic.source_id">
            <strong>{{ diagnostic.source_name }}</strong>
            <span>扫描 {{ diagnostic.pages_scanned }} 页 · 解析 {{ diagnostic.parsed_items }} 条 · {{ paginationStopLabel(diagnostic.stop_reason) }}</span>
          </li>
        </ul>
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
          <div v-for="failure in searchResult.failures" :key="failure.source_id + '-' + failure.message" class="search-failure-row">
            <p>{{ failure.source_name }}：{{ failure.message }}</p>
            <button
              class="source-link-button"
              type="button"
              :disabled="searchBusy || Boolean(retryingSourceId)"
              @click="retrySourceSearch(failure.source_id)"
            >
              {{ retryingSourceId === failure.source_id ? "重试中…" : "重试此源" }}
            </button>
          </div>
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

      <section class="settings-panel next-page-settings">
        <div class="settings-section-heading">
          <div>
            <span class="eyebrow">REMOTE SOURCE SAFETY</span>
            <h2>正文分页追链</h2>
          </div>
          <label class="policy-toggle">
            <input v-model="nextPagePolicy.enabled" type="checkbox" />
            <span>{{ nextPagePolicy.enabled ? "已启用" : "默认关闭" }}</span>
          </label>
        </div>
        <p class="settings-note">
          仅对明确支持 nextUrl / nextPage 的书源生效；服务端仍会强制同源、深度、页面、响应体和时间上限。
          开启后请点击“刷新内容”使当前章节重新请求；取消按钮可随时终止远端请求。
        </p>
        <div v-if="nextPagePolicy.enabled" class="settings-grid next-page-grid">
          <label class="settings-field">
            <span>最大深度 <strong>{{ nextPagePolicy.max_depth }}</strong></span>
            <input v-model.number="nextPagePolicy.max_depth" type="range" min="1" max="2" step="1" />
          </label>
          <label class="settings-field">
            <span>最大页面数 <strong>{{ nextPagePolicy.max_pages }}</strong></span>
            <input v-model.number="nextPagePolicy.max_pages" type="range" min="1" max="3" step="1" />
          </label>
          <label class="settings-field">
            <span>总响应体 <strong>{{ formatBytes(nextPagePolicy.max_bytes) }}</strong></span>
            <input v-model.number="nextPagePolicy.max_bytes" type="range" min="65536" max="2097152" step="65536" />
          </label>
          <label class="settings-field">
            <span>总耗时 <strong>{{ nextPagePolicy.max_duration_secs }} 秒</strong></span>
            <input v-model.number="nextPagePolicy.max_duration_secs" type="range" min="1" max="15" step="1" />
          </label>
          <label class="policy-check">
            <input v-model="nextPagePolicy.same_host_only" type="checkbox" />
            <span>仅允许同源后续链接</span>
          </label>
        </div>
        <button class="secondary-button policy-reset" type="button" @click="resetNextPagePolicy">恢复追链默认值</button>
      </section>
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

.search-page-limit {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: #9fb1c8;
  font-size: 12px;
}

.search-page-limit input {
  width: 54px;
  padding: 9px 7px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 8px;
  color: #dce7f7;
  background: #0c111b;
}

.library-search-input:focus,
.search-page-limit input:focus {
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

.search-diagnostics {
  display: grid;
  gap: 7px;
  margin: 13px 0 0;
  padding: 0;
  list-style: none;
  color: #9fb1c8;
  font-size: 12px;
}

.search-diagnostics li {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: baseline;
  padding: 8px 10px;
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 9px;
  background: rgba(12, 17, 27, 0.42);
}

.search-diagnostics strong {
  color: #dce7f7;
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

.source-preview-unsupported {
  display: grid;
  gap: 0.2rem;
  margin-top: 0.35rem;
  color: #ffb0bc;
  font-size: 0.75rem;
  line-height: 1.45;
}

.source-preview-unsupported span {
  overflow-wrap: anywhere;
  text-align: left;
}

.source-preview-unsupported small {
  display: block;
  margin-top: 0.15rem;
  color: #d7c4a1;
  font-size: 0.7rem;
}

.source-preview-actions {
  justify-content: flex-start;
  margin-top: 14px;
}

.source-conflict-strategy {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  color: #aebbd0;
  font-size: 11px;
}

.source-conflict-strategy select,
.source-batch-group-input {
  padding: 7px 9px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 8px;
  color: #dce7f7;
  background: #0c111b;
  font-size: 11px;
}

.source-snapshot-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 9px;
  padding: 8px 11px;
  border: 1px solid rgba(155, 231, 216, 0.16);
  border-radius: 9px;
  color: #8391a6;
  background: rgba(12, 17, 27, 0.38);
  font-size: 11px;
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

.source-batch-group-input {
  width: 92px;
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

.reader-cache-note {
  color: #b9d9ff;
  background: rgba(54, 92, 145, 0.22);
}

.reader-next-note {
  color: #c9c5ff;
  background: rgba(90, 74, 152, 0.2);
}

.reader-next-note strong {
  color: #eeeaff;
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

.next-page-settings {
  margin-top: 18px;
}

.policy-toggle,
.policy-check {
  display: inline-flex;
  align-items: center;
  gap: 9px;
  color: #c6d1e2;
  font-size: 13px;
}

.policy-toggle input,
.policy-check input {
  width: 16px;
  height: 16px;
  accent-color: #86dfc2;
}

.policy-reset {
  margin-top: 22px;
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
