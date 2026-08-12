<script setup lang="ts">
import { computed, onMounted, provide, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openNativeDialog } from "@tauri-apps/plugin-dialog";
import brandMark from "./assets/open-reader-mark.svg";
import LibraryOverview from "./components/LibraryOverview.vue";
import ReaderSettingsPanel from "./components/ReaderSettingsPanel.vue";
import SourceView from "./components/SourceView.vue";
import RemoteReaderView from "./components/RemoteReaderView.vue";
import LocalReaderView from "./components/LocalReaderView.vue";

const MAX_SOURCE_IMPORT_BYTES = 16 * 1024 * 1024;

type View = "library" | "reader" | "sources" | "settings";
type ReaderTheme = "night" | "paper" | "sepia" | "custom";
type ReaderTextAlign = "left" | "justify" | "center";
type ReaderMode = "scroll" | "paged";
type ReaderFont = "system" | "yahei" | "serif" | "kai";
type BookSort = "updated_at" | "title" | "author" | "progress" | "custom_order";

interface BookSummary {
  id: string;
  title: string;
  author: string | null;
  format: string;
  content_kind: string;
  cover_path: string | null;
  cover_state: string | null;
  shelf_group: string;
  tags: string[];
  custom_order: number;
  chapter_count: number;
  current_chapter: number;
  progress: number;
  updated_at: string;
  image_sequence_state: string | null;
  image_sequence_missing_pages: number;
  image_sequence_stale_pages: number;
}
interface DuplicateBookGroup {
  key: string;
  books: BookSummary[];
}


interface BookMergeBookPreview {
  id: string;
  title: string;
  author: string | null;
  format: string;
  content_kind: string;
  chapter_count: number;
  progress: number;
  current_chapter: number;
  shelf_group: string;
  tags: string[];
  cover_state: string | null;
  image_sequence_state: string | null;
  image_sequence_root_id: string | null;
  image_sequence_page_count: number | null;
}

interface BookMergeChapterCandidate {
  source_book_id: string;
  chapter_id: string;
  title: string;
  reason: string;
}

interface BookMergePreview {
  preview_id: string;
  created_at: number;
  expires_at: number;
  input_fingerprint: string;
  canonical_book_id: string;
  archived_book_ids: string[];
  books: BookMergeBookPreview[];
  append_candidates: BookMergeChapterCandidate[];
  chapter_conflicts: BookMergeChapterCandidate[];
  identical_chapter_count: number;
  progress_candidates: Array<{
    book_id: string;
    progress: number;
    current_chapter: number;
  }>;
  suggested_shelf_group: string;
  suggested_tags: string[];
  cover_candidates: Array<{
    book_id: string;
    state: string | null;
    source_kind: string | null;
    cache_key: string | null;
  }>;
  image_sequence_blocked: boolean;
  conflicts: string[];
  blocked_reasons: string[];
}

type TxtChapterRule = "auto" | "disabled" | "regex";

interface TxtReplacement {
  from: string;
  to: string;
}

interface TxtParseOptions {
  chapter_rule: TxtChapterRule;
  custom_pattern: string;
  normalize_full_width_space: boolean;
  replacements: TxtReplacement[];
}

interface BookImportPreview {
  title: string;
  format: string;
  encoding: string | null;
  chapter_count: number;
  first_chapter_title: string | null;
  warnings: string[];
}

type BookFormatSupport = "importable" | "probe_only" | "unsupported" | "signature_mismatch";

type FormatProbeMetadata =
  | { kind: "pdf"; version: string }
  | { kind: "image"; mime: string; width: number | null; height: number | null }
  | { kind: "mobi"; record_offset: number; header_length: number | null };

interface ImageDocumentPreview {
  file_name: string;
  mime: string;
  width: number;
  height: number;
  color_type: string;
  decoded_bytes: number;
}

type ImageReadingDirection = "ltr" | "rtl" | "vertical";
type ImageSpreadMode = "single" | "double" | "long_strip";

interface ImageSequencePage {
  index: number;
  file_name: string;
  mime: string;
  width: number;
  height: number;
  decoded_bytes: number;
}

interface ImageSequencePreview {
  direction: ImageReadingDirection;
  spread: ImageSpreadMode;
  page_count: number;
  total_pixels: number;
  total_decoded_bytes: number;
  pages: ImageSequencePage[];
}

interface ImageSequenceInput {
  file_name: string;
  bytes: number[];
}

interface ImageReadingLocation {
  cache_key: string;
  page_index: number;
  zoom: number;
  direction: ImageReadingDirection;
  spread: ImageSpreadMode;
}

interface ImageSequenceRecordPage {
  sequence_id: string;
  page_index: number;
  relative_path: string;
  file_size: number;
  modified_at_ns: number | null;
  content_digest: string | null;
  digest_version: number;
  mime: string;
  width: number;
  height: number;
  state: string;
}

interface ImageSequenceRecordSummary {
  book_id: string;
  title: string;
  author: string | null;
  root_id: string;
  root_path: string;
  cache_key: string;
  direction: string;
  spread: string;
  page_count: number;
  total_pixels: number;
  total_decoded_bytes: number;
  current_page: number;
  zoom: number;
  state: string;
  progress: number;
  updated_at: string;
}

interface ImageSequenceRecordDetail {
  sequence: ImageSequenceRecordSummary;
  pages: ImageSequenceRecordPage[];
}

interface ImageRelinkAssignment {
  page_index: number;
  old_relative_path: string;
  new_relative_path: string | null;
  status: string;
  match_kind: string;
  file_size: number;
  modified_at_ns: number | null;
}

interface ImageRelinkPreview {
  book_id: string;
  old_root_path: string;
  new_root_path: string;
  matched_pages: number;
  missing_pages: number;
  added_files: number;
  changed_pages: number;
  reordered: boolean;
  assignments: ImageRelinkAssignment[];
  missing_page_indices: number[];
  added_paths: string[];
}


interface ImageThumbnailCacheEntry {
  cache_key: string;
  page_index: number;
  byte_len: number;
  cache_hit: boolean;
}

interface ImageThumbnailCacheSummary {
  cache_key: string;
  page_count: number;
  cache_hits: number;
  cache_writes: number;
  evicted_files: number;
  cleaned_temp_files: number;
  cache_bytes: number;
  entries: ImageThumbnailCacheEntry[];
}

interface ImageThumbnailPageBytes {
  page_index: number;
  mime: string;
  bytes: number[];
}

interface BookFormatProbe {
  format: string;
  support: BookFormatSupport;
  signature_match: boolean;
  message: string;
  metadata?: FormatProbeMetadata | null;
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

interface ContentLink {
  label: string;
  href: string;
  targetChapter?: number | null;
}

interface ContentBlock {
  kind: ContentBlockKind;
  level?: number | null;
  anchor?: string | null;
  style?: string | null;
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

interface SourceRuleMetric {
  stage: string;
  rule_key: string;
  attempts: number;
  successes: number;
  no_matches: number;
  failures: number;
  skipped: number;
  success_rate: number;
  failure_rate: number;
  observed: boolean;
}

interface SourceRuleMetrics {
  total_attempts: number;
  total_successes: number;
  total_no_matches: number;
  total_failures: number;
  total_skipped: number;
  success_rate: number;
  failure_rate: number;
  observed: boolean;
  by_rule: SourceRuleMetric[];
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
  rule_metrics: SourceRuleMetrics | null;
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

const MAX_IMAGE_FILE_BYTES = 64 * 1024 * 1024;
const MAX_IMAGE_SEQUENCE_INPUT_BYTES = 256 * 1024 * 1024;
const MAX_IMAGE_THUMBNAILS = 24;
const IMAGE_LOCATION_KEY_PREFIX = "open-reader.image-location.";
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
const settingsSection = ref<"reader" | "appearance" | "network">("reader");
const books = ref<BookSummary[]>([]);
const libraryQuery = ref("");
const libraryGroupFilter = ref("");
const librarySort = ref<BookSort>("updated_at");
const librarySortDescending = ref(true);
const libraryLoadId = ref(0);
const editingBookId = ref<string | null>(null);
const metadataGroupDraft = ref("");
const metadataTagsDraft = ref("");
const metadataOrderDraft = ref("0");
const metadataBusy = ref(false);
const selectedBookIds = ref<string[]>([]);
const batchMetadataGroupDraft = ref("");
const batchMetadataTagsDraft = ref("");
const batchMetadataBusy = ref(false);
const duplicatePanelOpen = ref(false);
const duplicateBusy = ref(false);
const duplicateGroups = ref<DuplicateBookGroup[]>([]);
const duplicatePreviewBusy = ref(false);
const duplicatePreview = ref<BookMergePreview | null>(null);
const bookGroups = computed(() =>
  [...new Set(books.value.map((book) => book.shelf_group.trim()).filter(Boolean))]
    .sort((left, right) => left.localeCompare(right, "zh-CN")),
);
const allVisibleBooksSelected = computed(() =>
  books.value.length > 0 &&
  books.value.every((book) => selectedBookIds.value.includes(book.id)),
);
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
const bookImportPreview = ref<BookImportPreview | null>(null);
const imagePreview = ref<ImageDocumentPreview | null>(null);
const imagePreviewUrl = ref("");
const imageSequencePreview = ref<ImageSequencePreview | null>(null);
const imageSequenceUrls = ref<string[]>([]);
const imageSequenceDirection = ref<ImageReadingDirection>("ltr");
const imageSequenceSpread = ref<ImageSpreadMode>("single");
const imageSequenceLocation = ref<ImageReadingLocation | null>(null);
const imageSequenceBookId = ref<string | null>(null);
const imageSequenceBookTitle = ref("");
const imageSequenceRootPath = ref("");
const imageSequencePageDigests = ref<string[]>([]);
const imageSequencePageStates = ref<string[]>([]);
const imageSequenceRecordState = ref("");
const imageSequenceReadyPages = ref(0);
const imageSequenceMissingPages = ref(0);
const imageSequenceStalePages = ref(0);
const imageRelinkPreview = ref<ImageRelinkPreview | null>(null);
const imageRelinkBusy = ref(false);
const imageRelinkApplying = ref(false);
const imageRelinkOperationId = ref<string | null>(null);
const imageDigestBusy = ref(false);
const imageThumbnailCache = ref<ImageThumbnailCacheSummary | null>(null);
const imageCacheBusy = ref(false);
const imageCacheOperationId = ref<string | null>(null);
const imageSequenceCachedUrls = ref<Record<number, string>>({});
let imageSequenceInputs: ImageSequenceInput[] = [];
let imageSequenceThumbnailLoadId = 0;
const bookImportFileName = ref("");
const bookImportBytes = ref<number[]>([]);
const txtParseOptions = ref<TxtParseOptions>({
  chapter_rule: "auto",
  custom_pattern: "",
  normalize_full_width_space: false,
  replacements: [],
});
const txtReplacementDraft = ref("");
const bookImportBusy = ref(false);
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
const sourceBatchWeight = ref("");
const sourceBatchComment = ref("");
const sourceDragId = ref<string | null>(null);
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
const sourceRuleMetrics = ref<SourceRuleMetrics | null>(null);
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
    rule_metrics: sourceRuleMetrics.value,
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
const chapterLinks = computed(() => parseContentLinks(chapter.value));
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
watch([libraryQuery, libraryGroupFilter, librarySort, librarySortDescending], () => {
  void loadBooks();
});
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

function parseSafeContentStyle(value: unknown): string | null {
  if (typeof value !== "string" || !value.trim() || value.length > 512) return null;

  const declarations: string[] = [];
  for (const rawDeclaration of value.split(";").slice(0, 8)) {
    const separator = rawDeclaration.indexOf(":");
    if (separator <= 0) continue;
    const property = rawDeclaration.slice(0, separator).trim().toLowerCase();
    const cssValue = rawDeclaration.slice(separator + 1).trim().toLowerCase();
    if (!cssValue || cssValue.length > 64 || /[{}<>"']/.test(cssValue)) continue;

    const allowed = property === "text-align"
      ? ["left", "right", "center", "justify"].includes(cssValue)
      : property === "font-style"
        ? ["normal", "italic", "oblique"].includes(cssValue)
        : property === "font-weight"
          ? ["normal", "bold", "bolder", "lighter"].includes(cssValue) || /^[1-9]\d{2}$/.test(cssValue)
          : property === "text-decoration"
            ? ["none", "underline", "line-through"].includes(cssValue)
            : false;
    if (allowed && declarations.length < 4) {
      declarations.push(property + ": " + cssValue);
    }
  }
  return declarations.length ? declarations.join("; ") : null;
}

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
        anchor: typeof record.anchor === "string" ? record.anchor : null,
        style: parseSafeContentStyle(record.style),
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

function parseContentLinks(value: ChapterContent | null): ContentLink[] {
  if (!value || value.content_format !== "blocks-v1") return [];

  try {
    const parsed = JSON.parse(value.content) as {
      version?: unknown;
      links?: unknown;
    };
    if (parsed.version !== 1 || !Array.isArray(parsed.links)) return [];

    return parsed.links.flatMap((candidate) => {
      if (!candidate || typeof candidate !== "object") return [];
      const record = candidate as Record<string, unknown>;
      if (typeof record.label !== "string" || typeof record.href !== "string") return [];

      const href = record.href.trim();
      const lower = href.toLocaleLowerCase();
      const path = href.split("#")[0];
      if (
        !record.label.trim()
        || !href
        || href.length > 512
        || href.startsWith("/")
        || href.includes(":")
        || path.split("/").some((part) => part === "..")
        || /^(javascript:|data:|https?:|\/\/)/i.test(lower)
      ) {
        return [];
      }

      const targetChapter = typeof record.target_chapter === "number"
        && Number.isSafeInteger(record.target_chapter)
        && record.target_chapter >= 0
        && record.target_chapter < 100000
        ? record.target_chapter
        : null;
      return [{ label: record.label.trim(), href, targetChapter }];
    });
  } catch {
    return [];
  }
}

function scrollToFragment(href: string) {
  if (!href.startsWith("#") || href.length > 129 || /[\s"'<>/]/.test(href.slice(1))) {
    return;
  }
  const target = document.getElementById(href.slice(1));
  target?.scrollIntoView({ behavior: "smooth", block: "center" });
}

function fragmentFromHref(href: string): string {
  const hashIndex = href.indexOf("#");
  if (hashIndex < 0) return "";
  const fragment = href.slice(hashIndex);
  if (!fragment || fragment.length > 129 || /[\s"'<>/]/.test(fragment.slice(1))) {
    return "";
  }
  return fragment;
}

async function openContentLink(targetChapter: number | null | undefined, href: string) {
  if (!detail.value || !Number.isSafeInteger(targetChapter) || (targetChapter ?? -1) < 0) {
    return;
  }
  const chapterItem = detail.value.chapters[targetChapter as number];
  if (!chapterItem) return;

  const fragment = fragmentFromHref(href);
  if (chapterItem.id === chapter.value?.id) {
    if (fragment) scrollToFragment(fragment);
    return;
  }

  await loadChapter(chapterItem.id);
  if (fragment) {
    window.setTimeout(() => scrollToFragment(fragment), 0);
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
  const loadId = libraryLoadId.value + 1;
  libraryLoadId.value = loadId;
  try {
    const loaded = await invoke<BookSummary[]>("list_books_with_options", {
      options: {
        group: libraryGroupFilter.value || null,
        query: libraryQuery.value.trim() || null,
        sort: librarySort.value,
        descending: librarySortDescending.value,
      },
    });
    if (loadId !== libraryLoadId.value) return;
    books.value = loaded;
    selectedBookIds.value = selectedBookIds.value.filter((id) =>
      loaded.some((book) => book.id === id),
    );
    status.value = books.value.length ? `共 ${books.value.length} 本书` : "书架已准备好";
    errorMessage.value = "";
  } catch (error) {
    if (loadId !== libraryLoadId.value) return;
    status.value = "请在 Tauri 桌面模式中打开";
    errorMessage.value = String(error);
  }
}

async function loadDuplicateGroups() {
  duplicateBusy.value = true;
  errorMessage.value = "";
  try {
    duplicateGroups.value = await invoke<DuplicateBookGroup[]>("find_duplicate_books");
  } catch (error) {
    errorMessage.value = "检查重复书失败：" + String(error);
    duplicateGroups.value = [];
  } finally {
    duplicateBusy.value = false;
  }
}

async function toggleDuplicatePanel() {
  duplicatePanelOpen.value = !duplicatePanelOpen.value;
  if (duplicatePanelOpen.value) {
    await loadDuplicateGroups();
  }
}

async function previewDuplicateMerge(group: DuplicateBookGroup, canonicalBookId: string) {
  if (duplicatePreviewBusy.value) return;
  duplicatePreviewBusy.value = true;
  errorMessage.value = "";
  try {
    duplicatePreview.value = await invoke<BookMergePreview>("preview_book_merge", {
      request: {
        book_ids: group.books.map((book) => book.id),
        canonical_book_id: canonicalBookId,
      },
    });
    status.value = "重复书合并预览已生成，尚未修改任何数据";
  } catch (error) {
    duplicatePreview.value = null;
    errorMessage.value = "生成合并预览失败：" + String(error);
    status.value = "重复书合并预览失败";
  } finally {
    duplicatePreviewBusy.value = false;
  }
}

async function revalidateDuplicateMerge() {
  const current = duplicatePreview.value;
  if (!current || duplicatePreviewBusy.value) return;
  duplicatePreviewBusy.value = true;
  errorMessage.value = "";
  try {
    duplicatePreview.value = await invoke<BookMergePreview>("revalidate_book_merge_preview", {
      request: {
        preview: {
          book_ids: current.books.map((book) => book.id),
          canonical_book_id: current.canonical_book_id,
        },
        preview_id: current.preview_id,
        created_at: current.created_at,
        expires_at: current.expires_at,
        input_fingerprint: current.input_fingerprint,
      },
    });
    status.value = "重复书合并预览已重新验证，尚未修改任何数据";
  } catch (error) {
    duplicatePreview.value = null;
    errorMessage.value = "重新验证合并预览失败：" + String(error);
    status.value = "合并预览已失效，请重新生成";
  } finally {
    duplicatePreviewBusy.value = false;
  }
}

function clearDuplicatePreview() {
  duplicatePreview.value = null;
}


function beginBookMetadataEdit(book: BookSummary) {
  editingBookId.value = book.id;
  metadataGroupDraft.value = book.shelf_group;
  metadataTagsDraft.value = book.tags.join(", ");
  metadataOrderDraft.value = String(book.custom_order);
  errorMessage.value = "";
}

function cancelBookMetadataEdit() {
  editingBookId.value = null;
  metadataGroupDraft.value = "";
  metadataTagsDraft.value = "";
  metadataOrderDraft.value = "0";
}

async function saveBookMetadata(book: BookSummary) {
  if (metadataBusy.value || editingBookId.value !== book.id) return;
  const tags = metadataTagsDraft.value
    .split(/[,，、\n]/)
    .map((tag) => tag.trim())
    .filter(Boolean);
  const customOrder = Number.parseInt(metadataOrderDraft.value, 10);
  metadataBusy.value = true;
  errorMessage.value = "";
  try {
    await invoke<BookSummary>("update_book_metadata", {
      write: {
        book_id: book.id,
        shelf_group: metadataGroupDraft.value.trim(),
        tags,
        cover_path: book.cover_path,
        custom_order: Number.isFinite(customOrder) ? customOrder : 0,
      },
    });
    cancelBookMetadataEdit();
    await loadBooks();
    status.value = "书籍分组和标签已保存";
  } catch (error) {
    errorMessage.value = "保存书籍元数据失败：" + String(error);
    status.value = "书籍元数据保存失败";
  } finally {
    metadataBusy.value = false;
  }
}

function toggleBookSelection(bookId: string) {
  selectedBookIds.value = selectedBookIds.value.includes(bookId)
    ? selectedBookIds.value.filter((id) => id !== bookId)
    : [...selectedBookIds.value, bookId];
}

function toggleVisibleBookSelection() {
  selectedBookIds.value = allVisibleBooksSelected.value
    ? []
    : books.value.map((book) => book.id);
}

function clearBookSelection() {
  selectedBookIds.value = [];
  batchMetadataGroupDraft.value = "";
  batchMetadataTagsDraft.value = "";
}

async function saveBatchBookMetadata() {
  if (batchMetadataBusy.value || selectedBookIds.value.length === 0) return;
  const tags = batchMetadataTagsDraft.value
    .split(/[,，、\n]/)
    .map((tag) => tag.trim())
    .filter(Boolean);
  batchMetadataBusy.value = true;
  errorMessage.value = "";
  try {
    await invoke<BookSummary[]>("update_books_metadata", {
      write: {
        book_ids: selectedBookIds.value,
        shelf_group: batchMetadataGroupDraft.value.trim(),
        tags,
      },
    });
    const count = selectedBookIds.value.length;
    clearBookSelection();
    await loadBooks();
    status.value = `已批量更新 ${count} 本书的分组和标签`;
  } catch (error) {
    errorMessage.value = "批量保存书籍元数据失败：" + String(error);
    status.value = "批量元数据保存失败";
  } finally {
    batchMetadataBusy.value = false;
  }
}

async function openSources() {
  view.value = "sources";
  errorMessage.value = "";
  await Promise.all([loadSources(), loadSourceSnapshots(), refreshSourceCacheStatus(), loadSourceFailureHistory(), loadSourceFailureStats(), loadSourceRequestMetrics(), loadSourceRuleMetrics()]);
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

async function loadSourceRuleMetrics() {
  try {
    sourceRuleMetrics.value = await invoke<SourceRuleMetrics>("get_source_rule_metrics");
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

async function applySourceBatch(
  action: "enable" | "disable" | "explore-on" | "explore-off" | "group" | "metadata" | "delete",
) {
  const sourceIds = [...selectedSourceIds.value];
  if (!sourceIds.length) {
    errorMessage.value = "请先选择书源";
    return;
  }
  if (action === "delete" && !window.confirm(`确定删除选中的 ${sourceIds.length} 个书源吗？`)) return;

  let batchWeight: number | null = null;
  let batchComment: string | null = null;
  if (action === "metadata") {
    const rawWeight = sourceBatchWeight.value.trim();
    batchComment = sourceBatchComment.value.trim() || null;
    if (!rawWeight && !batchComment) {
      errorMessage.value = "请至少填写批量权重或批量备注";
      return;
    }
    if (rawWeight) {
      const parsedWeight = Number(rawWeight);
      if (!Number.isInteger(parsedWeight)) {
        errorMessage.value = "批量权重必须是整数";
        return;
      }
      batchWeight = parsedWeight;
    }
  }

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
    } else if (action === "metadata") {
      await invoke("update_sources_metadata", {
        sourceIds,
        weight: batchWeight,
        comment: batchComment,
      });
      sourceBatchWeight.value = "";
      sourceBatchComment.value = "";
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

function beginSourceDrag(sourceId: string) {
  sourceDragId.value = sourceId;
}

function clearSourceDrag() {
  sourceDragId.value = null;
}

async function dropSourceDrag(targetId: string) {
  const sourceId = sourceDragId.value;
  sourceDragId.value = null;
  if (!sourceId || sourceId === targetId || sourceBatchBusy.value) return;

  const source = sources.value.find((item) => item.id === sourceId);
  const target = sources.value.find((item) => item.id === targetId);
  if (!source || !target || source.group_name !== target.group_name) {
    errorMessage.value = "拖拽排序只允许在同一分组内进行";
    return;
  }

  const orderedIds = sources.value
    .filter((item) => item.group_name === source.group_name)
    .map((item) => item.id);
  const fromIndex = orderedIds.indexOf(sourceId);
  const targetIndex = orderedIds.indexOf(targetId);
  if (fromIndex < 0 || targetIndex < 0) return;

  const [movedId] = orderedIds.splice(fromIndex, 1);
  const insertIndex = orderedIds.indexOf(targetId);
  orderedIds.splice(insertIndex < 0 ? targetIndex : insertIndex, 0, movedId);

  sourceBatchBusy.value = true;
  errorMessage.value = "";
  try {
    await invoke("reorder_sources", { sourceIds: orderedIds });
    await loadSources();
    sourceTransferMessage.value = "已更新同组书源顺序";
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceBatchBusy.value = false;
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
    preview.invalid_count + " 个需人工处理；脚本、XPath、模板会保留原文但不执行";
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

  if (file.size > MAX_SOURCE_IMPORT_BYTES) {
    errorMessage.value = "书源文件超过 16 MB 限制";
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
  const bookUrl = item.book_url?.trim();
  if (!bookUrl) {
    sourceTransferMessage.value = "该结果没有可用的书籍链接";
    return;
  }
  if (remoteBusy.value) return;

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
      bookUrl,
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

function resetBookImportPreview() {
  bookImportPreview.value = null;
  imagePreview.value = null;
  imageSequencePreview.value = null;
  if (imagePreviewUrl.value) {
    URL.revokeObjectURL(imagePreviewUrl.value);
    imagePreviewUrl.value = "";
  }
  imageSequenceUrls.value.forEach((url) => URL.revokeObjectURL(url));
  imageSequenceUrls.value = [];
  Object.values(imageSequenceCachedUrls.value).forEach((url) => URL.revokeObjectURL(url));
  imageSequenceCachedUrls.value = {};
  imageSequenceThumbnailLoadId += 1;
  imageSequenceInputs = [];
  imageSequenceDirection.value = "ltr";
  imageSequenceSpread.value = "single";
  imageSequenceLocation.value = null;
  imageSequenceBookId.value = null;
  imageSequenceBookTitle.value = "";
  imageSequenceRootPath.value = "";
  imageSequencePageDigests.value = [];
  imageSequencePageStates.value = [];
  imageSequenceRecordState.value = "";
  imageSequenceReadyPages.value = 0;
  imageSequenceMissingPages.value = 0;
  imageSequenceStalePages.value = 0;
  imageRelinkPreview.value = null;
  imageRelinkBusy.value = false;
  imageRelinkApplying.value = false;
  imageRelinkOperationId.value = null;
  imageThumbnailCache.value = null;
  bookImportFileName.value = "";
  bookImportBytes.value = [];
  txtParseOptions.value = {
    chapter_rule: "auto",
    custom_pattern: "",
    normalize_full_width_space: false,
    replacements: [],
  };
  txtReplacementDraft.value = "";
}

function ensureTxtPattern() {
  if (txtParseOptions.value.chapter_rule === "regex") {
    if (!txtParseOptions.value.custom_pattern.trim()) {
      txtParseOptions.value.custom_pattern = "^第\\s*\\d+章";
    }
  } else {
    txtParseOptions.value.custom_pattern = "";
  }
}

function applyTxtReplacementDraft() {
  txtParseOptions.value.replacements = txtReplacementDraft.value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.includes("=>"))
    .map((line) => {
      const separator = line.indexOf("=>");
      return {
        from: line.slice(0, separator).trim(),
        to: line.slice(separator + 2).trim(),
      };
    })
    .filter((rule) => rule.from.length > 0);
}

function describeBookFormatProbe(probe: BookFormatProbe) {
  if (!probe.metadata) return probe.message;

  if (probe.metadata.kind === "image") {
    const size = probe.metadata.width && probe.metadata.height
      ? `，${probe.metadata.width}×${probe.metadata.height}`
      : "";
    return `${probe.message}（${probe.metadata.mime}${size}）`;
  }
  if (probe.metadata.kind === "pdf") {
    return `${probe.message}（PDF ${probe.metadata.version}）`;
  }
  return `${probe.message}（记录偏移 ${probe.metadata.record_offset}，MOBI 头长度 ${probe.metadata.header_length ?? "未知"}）`;
}

async function refreshBookImportPreview() {
  if (!bookImportFileName.value) return;

  applyTxtReplacementDraft();
  bookImportBusy.value = true;
  errorMessage.value = "";
  try {
    bookImportPreview.value = await invoke<BookImportPreview>("preview_book_import", {
      fileName: bookImportFileName.value,
      bytes: bookImportBytes.value,
      txtOptions: txtParseOptions.value,
    });
    status.value = "已生成《" + bookImportFileName.value + "》导入预览";
  } catch (error) {
    errorMessage.value = String(error);
    status.value = "TXT 预览失败";
  } finally {
    bookImportBusy.value = false;
  }
}

async function confirmBookImport() {
  const fileName = bookImportFileName.value;
  const preview = bookImportPreview.value;
  if (!fileName || !preview) return;

  applyTxtReplacementDraft();
  isImporting.value = true;
  errorMessage.value = "";
  status.value = "正在导入《" + fileName + "》…";
  try {
    const imported = await invoke<BookSummary>("import_book_with_options", {
      fileName,
      bytes: bookImportBytes.value,
      txtOptions: txtParseOptions.value,
    });
    resetBookImportPreview();
    await loadBooks();
    await openBook(imported.id);
  } catch (error) {
    errorMessage.value = String(error);
    status.value = "导入失败";
  } finally {
    isImporting.value = false;
  }
}

async function cancelBookImportPreview() {
  if (imageCacheBusy.value) await cancelImageCache();
  resetBookImportPreview();
  errorMessage.value = "";
  status.value = "已取消文件预览";
}

function fallbackHashBytes(bytes: Uint8Array) {
  let hash = 2166136261;
  for (const byte of bytes) {
    hash ^= byte;
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  return hash.toString(16).padStart(8, "0");
}

async function digestBytes(bytes: Uint8Array) {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) return fallbackHashBytes(bytes);
  try {
    const digest = await subtle.digest("SHA-256", bytes.buffer as ArrayBuffer);
    return Array.from(new Uint8Array(digest))
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join("");
  } catch {
    return fallbackHashBytes(bytes);
  }
}

async function buildImageSequenceCacheKey(
  pageDigests: string[],
  preview: ImageSequencePreview,
) {
  const material = JSON.stringify({
    version: 1,
    direction: preview.direction,
    spread: preview.spread,
    page_count: preview.page_count,
    total_pixels: preview.total_pixels,
    total_decoded_bytes: preview.total_decoded_bytes,
    pages: pageDigests,
  });
  const digest = await digestBytes(new TextEncoder().encode(material));
  return `imgseq-v1-${digest}`;
}

function createImageCacheOperationId() {
  return `image-cache-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

async function cacheImageSequenceInputs(
  cacheKey: string,
  pages: ImageSequenceInput[],
  direction: ImageReadingDirection,
  spread: ImageSpreadMode,
  forceRefresh = false,
) {
  const operationId = createImageCacheOperationId();
  imageCacheBusy.value = true;
  imageCacheOperationId.value = operationId;
  try {
    const summary = await invoke<ImageThumbnailCacheSummary>("cache_image_sequence", {
      cacheKey,
      pages,
      direction,
      spread,
      forceRefresh,
      operationId,
    });
    imageThumbnailCache.value = summary;
    return summary;
  } finally {
    if (imageCacheOperationId.value === operationId) {
      imageCacheOperationId.value = null;
      imageCacheBusy.value = false;
    }
  }
}

async function cancelImageCache() {
  const operationId = imageCacheOperationId.value;
  if (!operationId) return;
  try {
    const accepted = await invoke<boolean>("cancel_image_cache", { operationId });
    status.value = accepted ? "正在取消图片缓存…" : "图片缓存任务已结束";
  } catch (error) {
    errorMessage.value = "取消图片缓存失败：" + String(error);
  }
}

function imageSequenceAdjacentPageIndices(index: number, pageCount: number) {
  return [...new Set([index - 1, index, index + 1])]
    .filter((value) => value >= 0 && value < pageCount);
}

async function loadImageSequenceThumbnails(indices: number[]) {
  const location = imageSequenceLocation.value;
  const preview = imageSequencePreview.value;
  if (!location || !preview) return;
  const safeIndices = [...new Set(indices)]
    .filter((value) => value >= 0 && value < preview.page_count)
    .slice(0, 3);
  if (!safeIndices.length) return;

  const requestId = ++imageSequenceThumbnailLoadId;
  try {
    const pages = await invoke<ImageThumbnailPageBytes[]>("read_image_sequence_thumbnails", {
      cacheKey: location.cache_key,
      pageIndices: safeIndices,
    });
    if (requestId !== imageSequenceThumbnailLoadId) return;

    const loadedIndices = new Set(pages.map((page) => page.page_index));
    for (const [key, url] of Object.entries(imageSequenceCachedUrls.value)) {
      if (!loadedIndices.has(Number(key))) URL.revokeObjectURL(url);
    }
    const next: Record<number, string> = {};
    for (const page of pages) {
      next[page.page_index] = URL.createObjectURL(new Blob(
        [new Uint8Array(page.bytes)],
        { type: page.mime || "image/png" },
      ));
    }
    imageSequenceCachedUrls.value = next;
  } catch {
    // 磁盘缓存不可读时继续使用本次导入的临时对象 URL。
  }
}

function imageSequencePageUrl(index: number) {
  if (imageSequencePageStates.value[index] === "missing") return "";
  return imageSequenceCachedUrls.value[index] || imageSequenceUrls.value[index] || "";
}

function imageSequencePageStateLabel(index: number) {
  const state = imageSequencePageStates.value[index];
  if (state === "missing") return "原文件缺失";
  if (state === "stale") return "原文件已变化，等待 SHA-256 复核";
  return "";
}

function loadImageSequenceLocation(cacheKey: string, preview: ImageSequencePreview) {
  try {
    const raw = localStorage.getItem(IMAGE_LOCATION_KEY_PREFIX + cacheKey);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<ImageReadingLocation>;
    if (parsed.cache_key !== cacheKey) return null;
    const direction = parsed.direction === "rtl" || parsed.direction === "vertical"
      ? parsed.direction
      : preview.direction;
    const spread = parsed.spread === "double" || parsed.spread === "long_strip"
      ? parsed.spread
      : preview.spread;
    return {
      cache_key: cacheKey,
      page_index: Math.min(
        Math.max(Number(parsed.page_index) || 0, 0),
        Math.max(preview.page_count - 1, 0),
      ),
      zoom: Math.min(Math.max(Number(parsed.zoom) || 1, 0.5), 3),
      direction,
      spread,
    } satisfies ImageReadingLocation;
  } catch {
    return null;
  }
}

function persistImageSequenceLocation() {
  const location = imageSequenceLocation.value;
  if (!location) return;
  try {
    localStorage.setItem(
      IMAGE_LOCATION_KEY_PREFIX + location.cache_key,
      JSON.stringify(location),
    );
  } catch {
    // localStorage 不可用时保持当前会话位置，不阻断预览。
  }
  void savePersistedImageSequenceProgress();
}

async function savePersistedImageSequenceProgress() {
  const bookId = imageSequenceBookId.value;
  const location = imageSequenceLocation.value;
  if (!bookId || !location || !imageSequencePreview.value) return;

  try {
    const saved = await invoke<ImageSequenceRecordSummary>("save_image_sequence_progress", {
      bookId,
      currentPage: location.page_index,
      zoom: location.zoom,
      direction: location.direction,
      spread: location.spread,
    });
    const bookIndex = books.value.findIndex((book) => book.id === saved.book_id);
    if (bookIndex >= 0) {
      books.value[bookIndex] = {
        ...books.value[bookIndex],
        current_chapter: saved.current_page,
        progress: saved.progress,
        updated_at: saved.updated_at,
      };
    }
  } catch (error) {
    errorMessage.value = "图片阅读进度保存失败：" + String(error);
  }
}

function selectImageSequencePage(index: number) {
  if (!imageSequenceLocation.value || !imageSequencePreview.value) return;
  imageSequenceLocation.value = {
    ...imageSequenceLocation.value,
    page_index: Math.min(
      Math.max(index, 0),
      Math.max(imageSequencePreview.value.page_count - 1, 0),
    ),
  };
  persistImageSequenceLocation();
  void loadImageSequenceThumbnails(
    imageSequenceAdjacentPageIndices(
      imageSequenceLocation.value.page_index,
      imageSequencePreview.value.page_count,
    ),
  );
}

function moveImageSequencePage(delta: number) {
  const current = imageSequenceLocation.value?.page_index ?? 0;
  selectImageSequencePage(current + delta);
}

function updateImageSequenceZoom(value: number) {
  if (!imageSequenceLocation.value) return;
  imageSequenceLocation.value = {
    ...imageSequenceLocation.value,
    zoom: Math.min(Math.max(value, 0.5), 3),
  };
  persistImageSequenceLocation();
}

function handleImageSequenceZoom(event: Event) {
  const target = event.target as HTMLInputElement | null;
  if (target) updateImageSequenceZoom(Number(target.value));
}

function imageDirectionLabel(direction: ImageReadingDirection) {
  return direction === "rtl" ? "从右到左" : direction === "vertical" ? "纵向长图" : "从左到右";
}

function imageSpreadLabel(spread: ImageSpreadMode) {
  return spread === "double" ? "双页" : spread === "long_strip" ? "长图" : "单页";
}

function imageSequenceStateLabel(state: string) {
  if (state === "needs_relink") return "目录需要重新关联";
  if (state === "missing") return "存在缺失页";
  if (state === "stale") return "检测到文件变化";
  if (state === "ready") return "文件状态正常";
  return "尚未检测";
}

function imageSequenceHealthLabel(book: BookSummary) {
  if (book.content_kind !== "image_sequence" || !book.image_sequence_state) return "";
  if (book.image_sequence_state === "needs_relink") return "目录需重新关联";
  if (book.image_sequence_state === "missing") {
    return book.image_sequence_missing_pages > 0
      ? `${book.image_sequence_missing_pages} 页缺失`
      : "存在缺页";
  }
  if (book.image_sequence_state === "stale") {
    return book.image_sequence_stale_pages > 0
      ? `${book.image_sequence_stale_pages} 页待复核`
      : "有变化待复核";
  }
  return "图片正常";
}

function imageSequenceHealthClass(book: BookSummary) {
  return book.image_sequence_state === "ready" ? "ready"
    : book.image_sequence_state === "stale" ? "stale"
      : "missing";
}

function coverStateLabel(book: BookSummary) {
  if (!book.cover_state || book.cover_state === "ready") return "";
  if (book.cover_state === "stale") return "封面待刷新";
  if (book.cover_state === "blocked") return "封面已阻止";
  return "使用占位图";
}

function coverStateClass(book: BookSummary) {
  return book.cover_state === "stale" ? "stale"
    : book.cover_state === "blocked" ? "blocked"
      : "missing";
}

function imageRelinkAssignmentLabel(assignment: ImageRelinkAssignment) {
  if (assignment.status === "missing") return "缺失";
  if (assignment.match_kind === "basename_size") return "按文件名和大小候选，待复核";
  if (assignment.status === "changed") return "路径匹配但文件特征变化";
  return "路径匹配";
}

function normalizeNativeDirectorySelection(selection: string | string[] | null) {
  if (typeof selection === "string") return selection.trim();
  if (Array.isArray(selection)) return (selection[0] ?? "").trim();
  return "";
}

async function chooseImageSequenceRoot() {
  try {
    const selection = await openNativeDialog({
      directory: true,
      multiple: false,
      title: "选择图片序列根目录",
    });
    const rootPath = normalizeNativeDirectorySelection(selection);
    if (!rootPath) return;
    imageSequenceRootPath.value = rootPath;
    status.value = "已选择图片序列根目录，请继续检查并保存";
  } catch (error) {
    errorMessage.value = "打开目录选择器失败：" + String(error);
  }
}

async function previewImageSequenceRelink(newRootPath: string) {
  const bookId = imageSequenceBookId.value;
  const rootPath = newRootPath.trim();
  if (!bookId || !rootPath) return;
  const operationId = "image-relink-" + Date.now();
  imageRelinkBusy.value = true;
  imageRelinkOperationId.value = operationId;
  imageRelinkPreview.value = null;
  errorMessage.value = "";
  status.value = "正在扫描新目录并生成重新关联差异…";
  try {
    imageRelinkPreview.value = await invoke<ImageRelinkPreview>(
      "preview_image_sequence_relink",
      { bookId, newRootPath: rootPath, operationId },
    );
    status.value = "重新关联差异已生成，请确认后再更新书架记录";
  } catch (error) {
    const message = String(error);
    const canceled = message.includes("取消");
    const timedOut = message.includes("超时") || message.includes("时间上限");
    errorMessage.value = canceled
      ? "重新关联扫描已取消：" + message
      : timedOut
        ? "重新关联扫描超时：" + message
        : "重新关联扫描失败：" + message;
    status.value = canceled
      ? "重新关联扫描已取消，旧目录仍保持不变"
      : timedOut
        ? "重新关联扫描超时，旧目录仍保持不变"
        : "重新关联扫描失败，旧目录仍保持不变";
  } finally {
    if (imageRelinkOperationId.value === operationId) {
      imageRelinkOperationId.value = null;
    }
    imageRelinkBusy.value = false;
  }
}

async function cancelImageRelinkScan() {
  const operationId = imageRelinkOperationId.value;
  if (!operationId || !imageRelinkBusy.value) return;
  try {
    const accepted = await invoke<boolean>("cancel_image_sequence_relink", { operationId });
    status.value = accepted ? "正在取消重新关联扫描…" : "重新关联扫描任务已结束";
  } catch (error) {
    errorMessage.value = "取消重新关联扫描失败：" + String(error);
  }
}

async function chooseImageSequenceRelinkRoot() {
  if (!imageSequenceBookId.value || imageRelinkBusy.value || imageRelinkApplying.value) return;
  try {
    const selection = await openNativeDialog({
      directory: true,
      multiple: false,
      title: "选择新的图片序列根目录",
    });
    const rootPath = normalizeNativeDirectorySelection(selection);
    if (rootPath) await previewImageSequenceRelink(rootPath);
  } catch (error) {
    errorMessage.value = "打开目录选择器失败：" + String(error);
  }
}

async function applyImageSequenceRelink() {
  const preview = imageRelinkPreview.value;
  if (!preview || imageRelinkApplying.value) return;
  imageRelinkApplying.value = true;
  errorMessage.value = "";
  status.value = "正在事务化更新图片目录关联…";
  try {
    await invoke<ImageSequenceRecordDetail>("apply_image_sequence_relink", {
      bookId: preview.book_id,
      newRootPath: preview.new_root_path,
      assignments: preview.assignments,
    });
    imageRelinkPreview.value = null;
    await loadBooks();
    await openPersistedImageSequenceBook(preview.book_id);
    status.value = "图片目录已重新关联；变化页仍需内容校验后才会标为正常";
  } catch (error) {
    errorMessage.value = "重新关联失败：" + String(error);
    status.value = "重新关联未应用，旧目录和缓存仍保持不变";
  } finally {
    imageRelinkApplying.value = false;
  }
}

async function verifyImageSequenceDigests() {
  const bookId = imageSequenceBookId.value;
  if (!bookId || imageDigestBusy.value) return;

  imageDigestBusy.value = true;
  errorMessage.value = "";
  status.value = "正在对变化页执行 SHA-256 复核…";
  try {
    const verified = await invoke<ImageSequenceRecordDetail>(
      "verify_image_sequence_digests",
      { bookId },
    );
    const counts = imageSequencePageStateCounts(verified.pages);
    await openPersistedImageSequenceBook(bookId);
    if (counts.stale > 0) {
      status.value = "SHA-256 复核完成，仍有 " + counts.stale + " 页保持待复核";
    } else if (counts.missing > 0) {
      status.value = "SHA-256 复核完成，仍有 " + counts.missing + " 页缺失";
    } else {
      status.value = "SHA-256 复核完成，变化页已恢复";
    }
  } catch (error) {
    errorMessage.value = "SHA-256 复核失败：" + String(error);
    status.value = "变化页复核失败，原有状态未被清除";
  } finally {
    imageDigestBusy.value = false;
  }
}

function imageSequencePageStateCounts(pages: ImageSequenceRecordPage[]) {
  return pages.reduce(
    (counts, page) => {
      if (page.state === "missing") counts.missing += 1;
      else if (page.state === "stale") counts.stale += 1;
      else counts.ready += 1;
      return counts;
    },
    { ready: 0, missing: 0, stale: 0 },
  );
}

function updateImageSequenceLayout() {
  if (!imageSequencePreview.value) return;
  imageSequencePreview.value = {
    ...imageSequencePreview.value,
    direction: imageSequenceDirection.value,
    spread: imageSequenceSpread.value,
  };
  if (imageSequenceLocation.value) {
    imageSequenceLocation.value = {
      ...imageSequenceLocation.value,
      direction: imageSequenceDirection.value,
      spread: imageSequenceSpread.value,
    };
    persistImageSequenceLocation();
  }
  status.value = `图片序列已切换为${imageDirectionLabel(imageSequenceDirection.value)} · ${imageSpreadLabel(imageSequenceSpread.value)}`;
}

async function importImageSequence(files: File[]) {
  if (files.length > 2048) {
    errorMessage.value = "图片序列最多支持 2048 页";
    status.value = "图片序列过大";
    return;
  }
  const totalBytes = files.reduce((total, file) => total + file.size, 0);
  if (files.some((file) => file.size > MAX_IMAGE_FILE_BYTES)) {
    errorMessage.value = "单张图片不能超过 64 MB";
    status.value = "图片序列包含超限文件";
    return;
  }
  if (totalBytes > MAX_IMAGE_SEQUENCE_INPUT_BYTES) {
    errorMessage.value = "图片序列原始输入不能超过 256 MB";
    status.value = "图片序列过大";
    return;
  }

  resetBookImportPreview();
  isImporting.value = true;
  errorMessage.value = "";
  status.value = `正在验证 ${files.length} 张图片…`;
  try {
    const pages: ImageSequenceInput[] = [];
    const pageDigests: string[] = [];
    for (const file of files) {
      const raw = new Uint8Array(await file.arrayBuffer());
      pages.push({
        file_name: file.webkitRelativePath || file.name,
        bytes: Array.from(raw),
      });
      pageDigests.push(await digestBytes(raw));
    }
    const preview = await invoke<ImageSequencePreview>("preview_image_sequence", {
      pages,
      direction: imageSequenceDirection.value,
      spread: imageSequenceSpread.value,
    });
    const cacheKey = await buildImageSequenceCacheKey(pageDigests, preview);
    imageSequenceInputs = pages;
    imageSequencePageDigests.value = pageDigests;
    imageSequenceBookTitle.value =
      imageSequenceBookTitle.value.trim() ||
      (files[0]?.name || "图片序列") + " 等 " + files.length + " 页";
    try {
      await cacheImageSequenceInputs(
        cacheKey,
        pages,
        preview.direction,
        preview.spread,
        false,
      );
    } catch (error) {
      imageThumbnailCache.value = null;
      errorMessage.value = "缩略图缓存未写入：" + String(error);
      status.value = "图片序列已验证，可稍后重试缓存";
    }
    imageSequencePreview.value = preview;
    imageSequenceLocation.value = loadImageSequenceLocation(cacheKey, preview) ?? {
      cache_key: cacheKey,
      page_index: 0,
      zoom: 1,
      direction: preview.direction,
      spread: preview.spread,
    };
    imageSequenceDirection.value = imageSequenceLocation.value.direction;
    imageSequenceSpread.value = imageSequenceLocation.value.spread;
    persistImageSequenceLocation();
    imageSequenceUrls.value = files
      .slice(0, MAX_IMAGE_THUMBNAILS)
      .map((file) => URL.createObjectURL(file));
    bookImportFileName.value = `${files.length} 个图片文件`;
    void loadImageSequenceThumbnails(
      imageSequenceAdjacentPageIndices(
        imageSequenceLocation.value.page_index,
        preview.page_count,
      ),
    );
    status.value = `图片序列已通过受限解码，可预览 ${files.length} 页，已恢复第 ${imageSequenceLocation.value.page_index + 1} 页`;
  } catch (error) {
    errorMessage.value = String(error);
    status.value = "图片序列预览失败";
  } finally {
    isImporting.value = false;
  }
}

async function saveImageSequenceToLibrary() {
  const preview = imageSequencePreview.value;
  const location = imageSequenceLocation.value;
  const rootPath = imageSequenceRootPath.value.trim();
  if (!preview || !location || !imageSequenceInputs.length) {
    errorMessage.value = "请先选择并验证图片序列";
    return;
  }
  if (!rootPath) {
    errorMessage.value = "请填写图片根目录绝对路径";
    return;
  }
  if (imageSequencePageDigests.value.length !== preview.pages.length) {
    errorMessage.value = "图片摘要尚未完成，请重新导入图片序列";
    return;
  }
  if (imageCacheBusy.value) {
    errorMessage.value = "请等待缩略图缓存任务结束";
    return;
  }

  const title = imageSequenceBookTitle.value.trim() || "未命名图片序列";
  status.value = "正在保存图片序列到书架…";
  errorMessage.value = "";
  try {
    const saved = await invoke<ImageSequenceRecordSummary>("save_image_sequence", {
      write: {
        book_id: null,
        title,
        author: null,
        root_path: rootPath,
        cache_key: location.cache_key,
        direction: preview.direction,
        spread: preview.spread,
        page_count: preview.page_count,
        total_pixels: preview.total_pixels,
        total_decoded_bytes: preview.total_decoded_bytes,
        current_page: location.page_index,
        zoom: location.zoom,
        pages: preview.pages.map((page, index) => ({
          page_index: page.index,
          relative_path: page.file_name,
          file_size: imageSequenceInputs[index]?.bytes.length ?? 0,
          modified_at_ns: null,
          content_digest: imageSequencePageDigests.value[index] || null,
          digest_version: 1,
          mime: page.mime,
          width: page.width,
          height: page.height,
        })),
      },
    });
    imageSequenceBookId.value = saved.book_id;
    imageSequenceBookTitle.value = saved.title;
    imageSequenceRootPath.value = saved.root_path;
    imageSequenceRecordState.value = saved.state;
    imageSequenceReadyPages.value = preview.page_count;
    imageSequenceMissingPages.value = 0;
    imageSequenceStalePages.value = 0;
    imageSequenceLocation.value = {
      ...location,
      page_index: saved.current_page,
      zoom: saved.zoom,
      direction: saved.direction === "rtl" || saved.direction === "vertical"
        ? saved.direction
        : "ltr",
      spread: saved.spread === "double" || saved.spread === "long_strip"
        ? saved.spread
        : "single",
    };
    imageSequenceDirection.value = imageSequenceLocation.value.direction;
    imageSequenceSpread.value = imageSequenceLocation.value.spread;
    persistImageSequenceLocation();
    await loadBooks();
    status.value = "图片序列已保存到书架，之后可从书架恢复阅读";
  } catch (error) {
    errorMessage.value = "保存图片序列失败：" + String(error);
    status.value = "图片序列保存失败";
  }
}

async function retryImageSequenceCache() {
  const preview = imageSequencePreview.value;
  const location = imageSequenceLocation.value;
  if (!preview || !location || !imageSequenceInputs.length || imageCacheBusy.value) return;

  errorMessage.value = "";
  status.value = "正在重试图片缩略图缓存…";
  try {
    await cacheImageSequenceInputs(
      location.cache_key,
      imageSequenceInputs,
      preview.direction,
      preview.spread,
      false,
    );
    await loadImageSequenceThumbnails(
      imageSequenceAdjacentPageIndices(location.page_index, preview.page_count),
    );
    status.value = "图片缩略图缓存已重试";
  } catch (error) {
    imageThumbnailCache.value = null;
    errorMessage.value = "缩略图缓存重试失败：" + String(error);
    status.value = "图片序列缓存仍未完成";
  }
}

async function importFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const files = Array.from(input.files ?? []);
  if (!files.length) return;

  if (files.length > 1) {
    try {
      await importImageSequence(files);
    } finally {
      input.value = "";
    }
    return;
  }

  const file = files[0];
  if (file.size > MAX_IMAGE_FILE_BYTES) {
    errorMessage.value = "文件超过 64 MB 限制";
    input.value = "";
    return;
  }

  errorMessage.value = "";
  try {
    const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
    resetBookImportPreview();

    const probe = await invoke<BookFormatProbe>("probe_book_format", {
      fileName: file.name,
      bytes,
    });
    const probeMessage = describeBookFormatProbe(probe);
    if (probe.format === "image" && probe.support === "probe_only") {
      isImporting.value = true;
      status.value = "正在验证图片《" + file.name + "》…";
      try {
        const preview = await invoke<ImageDocumentPreview>("preview_image_document", {
          fileName: file.name,
          bytes,
        });
        imagePreview.value = preview;
        bookImportFileName.value = file.name;
        bookImportBytes.value = bytes;
        imagePreviewUrl.value = URL.createObjectURL(new Blob(
          [new Uint8Array(bytes)],
          { type: preview.mime },
        ));
        status.value = "图片已通过受限解码，可进行单页预览";
      } catch (error) {
        errorMessage.value = String(error);
        status.value = "图片预览失败";
      } finally {
        isImporting.value = false;
      }
      return;
    }
    if (probe.support !== "importable") {
      errorMessage.value = probeMessage;
      status.value = probeMessage;
      return;
    }

    const isTxt = file.name.toLowerCase().endsWith(".txt");
    if (isTxt) {
      bookImportFileName.value = file.name;
      bookImportBytes.value = bytes;
      status.value = "正在生成《" + file.name + "》导入预览…";
      await refreshBookImportPreview();
      return;
    }

    isImporting.value = true;
    status.value = "正在解析《" + file.name + "》…";
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

async function openPersistedImageSequenceBook(bookId: string) {
  const loaded = await invoke<ImageSequenceRecordDetail>("refresh_image_sequence_state", { bookId });
  const sequence = loaded.sequence;
  const pageStateCounts = imageSequencePageStateCounts(loaded.pages);
  const bookIndex = books.value.findIndex((book) => book.id === sequence.book_id);
  if (bookIndex >= 0) {
    books.value[bookIndex] = {
      ...books.value[bookIndex],
      current_chapter: sequence.current_page,
      progress: sequence.progress,
      updated_at: sequence.updated_at,
      image_sequence_state: sequence.state,
      image_sequence_missing_pages: pageStateCounts.missing,
      image_sequence_stale_pages: pageStateCounts.stale,
    };
  }
  const direction: ImageReadingDirection =
    sequence.direction === "rtl" || sequence.direction === "vertical"
      ? sequence.direction
      : "ltr";
  const spread: ImageSpreadMode =
    sequence.spread === "double" || sequence.spread === "long_strip"
      ? sequence.spread
      : "single";

  resetBookImportPreview();
  imageSequenceBookId.value = sequence.book_id;
  imageSequenceBookTitle.value = sequence.title;
  imageSequenceRootPath.value = sequence.root_path;
  imageSequenceRecordState.value = sequence.state;
  imageSequenceReadyPages.value = pageStateCounts.ready;
  imageSequenceMissingPages.value = pageStateCounts.missing;
  imageSequenceStalePages.value = pageStateCounts.stale;
  imageSequenceDirection.value = direction;
  imageSequenceSpread.value = spread;
  imageSequencePreview.value = {
    direction,
    spread,
    page_count: sequence.page_count,
    total_pixels: sequence.total_pixels,
    total_decoded_bytes: sequence.total_decoded_bytes,
    pages: loaded.pages.map((page) => ({
      index: page.page_index,
      file_name: page.relative_path,
      mime: page.mime,
      width: page.width,
      height: page.height,
      decoded_bytes: page.file_size,
    })),
  };
  imageSequenceLocation.value = {
    cache_key: sequence.cache_key,
    page_index: sequence.current_page,
    zoom: Math.min(Math.max(sequence.zoom, 0.5), 3),
    direction,
    spread,
  };
  bookImportFileName.value = sequence.title;
  imageSequenceUrls.value = [];
  imageSequencePageDigests.value = loaded.pages.map((page) => page.content_digest || "");
  imageSequencePageStates.value = loaded.pages.map((page) => page.state);
  detail.value = null;
  chapter.value = null;
  view.value = "library";
  errorMessage.value = "";
  status.value = sequence.state === "ready"
    ? "已从书架恢复图片序列，可继续阅读"
    : "图片序列已恢复，但文件状态为 " + sequence.state + "，后续需要重新关联目录";
  await loadImageSequenceThumbnails(
    imageSequenceAdjacentPageIndices(sequence.current_page, sequence.page_count),
  );
}

async function openBook(bookId: string) {
  try {
    const loaded = await invoke<BookDetail>("get_book_detail", { bookId });
    if (loaded.book.content_kind === "image_sequence") {
      await openPersistedImageSequenceBook(bookId);
      return;
    }
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

provide("open-reader-context", { SETTINGS_KEY, SETTINGS_VERSION, DEFAULT_READER_SETTINGS, readerFontStacks, view, books, recentBooks, continueBook, detail, chapter, fileInput, sourceImportInput, status, errorMessage, isImporting, imagePreview, imagePreviewUrl, settings, sourceBusy, sourceValidation, sources, filteredSources, sourceGroupFilter, sourceGroupDraft, sourceWeightDraft, sourceOrderDraft, sourceExploreDraft, sourceCommentDraft, selectedSourceIds, sourceBatchBusy, sourceBatchGroup, sourceBatchWeight, sourceBatchComment, allFilteredSourcesSelected, sourceId, sourceListBusy, sourcePipelineBusy, sourceKeyword, sourcePipeline, searchKeyword, searchPageLimit, searchBusy, searchResult, sourceTransferBusy, sourceTransferMessage, sourceImportUrl, sourceImportPreview, sourceImportPayload, sourceImportLabel, sourceImportStrategy, sourceSnapshots, sourceImportSnapshotId, retryingSourceId, sourceAuditBusy, sourceAudit, sourceCacheBusy, sourceCacheStatus, sourceFailureHistory, sourceFailureHistoryBusy, sourceFailureStats, sourceRequestMetrics, sourceRuleMetrics, sourceMetrics, remoteBusy, remoteBook, remoteChapter, remoteChapterRef, nextPagePolicy, remoteNextPageStatus, sourceJson, chapterParagraphs, chapterBlocks, chapterLinks, scrollToFragment, openContentLink, remoteChapterParagraphs, readerStyle, themeLabels, parseContentBlocks, contentBlockTag, clampNumber, normalizeHex, isRecord, loadSettings, loadNextPagePolicy, loadBooks, openSources, openSettings, closeSettings, resetSettings, resetNextPagePolicy, loadSources, loadSourceSnapshots, runSourceAudit, refreshSourceCacheStatus, loadSourceFailureHistory, clearSourceFailureHistory, loadSourceFailureStats, loadSourceRequestMetrics, loadSourceRuleMetrics, formatBytes, formatPercent, selectSource, newSourceDraft, saveSource, saveSourceMetadata, toggleSource, toggleSourceExplore, toggleSourceSelection, toggleSelectAllSources, applySourceBatch, reorderSource, beginSourceDrag, dropSourceDrag, clearSourceDrag, deleteSource, searchSources, retrySourceSearch, cancelSearch, clearSearch, finishSourceImport, exportSources, openSourceImportPicker, showSourceImportPreview, clearSourceImportPreview, confirmSourceImport, restoreSourceSnapshot, importSourceUrl, importSourceFile, openRemoteBook, loadRemoteChapter, cancelRemoteOperation, remoteChapterIndex, goToRemoteChapter, previousRemoteChapter, nextRemoteChapter, runSourcePipeline, cancelSourcePipeline, exportSourceDiagnostics, exportSourceFailureReport, validateSource, openFilePicker, importFile, openBook, loadChapter, saveProgress, continueReading, closeReader, cycleTheme, formatProgress, currentChapterIndex, goToChapter, previousChapter, nextChapter });
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
        <button class="nav-item" :class="{ active: view === 'library' }" type="button" @click="closeReader">
          <span class="nav-icon" aria-hidden="true">▦</span>
          <span class="nav-label">书架</span>
          <span class="nav-meta">LIBRARY</span>
        </button>
        <button class="nav-item" :class="{ active: view === 'sources' }" type="button" @click="openSources">
          <span class="nav-icon" aria-hidden="true">◇</span>
          <span class="nav-label">书源</span>
          <span class="nav-meta">SOURCES</span>
        </button>
        <button class="nav-item" :class="{ active: view === 'settings' }" type="button" @click="openSettings">
          <span class="nav-icon" aria-hidden="true">⚙</span>
          <span class="nav-label">设置</span>
          <span class="nav-meta">SETTINGS</span>
        </button>
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
        multiple
        accept=".txt,.epub,.png,.jpg,.jpeg,.gif,.webp,text/plain,application/epub+zip,image/png,image/jpeg,image/gif,image/webp"
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
            {{ isImporting ? "解析中…" : "导入 TXT / EPUB / 图片" }}
          </button>
        </div>
      </header>

      <div class="status-banner" role="status">
        <span class="status-dot"></span>
        <span>{{ status }}</span>
        <span v-if="errorMessage" class="error-text">{{ errorMessage }}</span>
      </div>

      <div class="library-filter-bar" aria-label="书架筛选与排序">
        <label class="library-filter-field">
          <span>书架搜索</span>
          <input v-model="libraryQuery" type="search" placeholder="搜索书名、作者或标签" />
        </label>
        <label class="library-filter-field">
          <span>分组</span>
          <select v-model="libraryGroupFilter">
            <option value="">全部分组</option>
            <option v-for="group in bookGroups" :key="group" :value="group">{{ group }}</option>
          </select>
        </label>
        <label class="library-filter-field">
          <span>排序</span>
          <select v-model="librarySort">
            <option value="updated_at">最近更新</option>
            <option value="title">书名</option>
            <option value="author">作者</option>
            <option value="progress">阅读进度</option>
            <option value="custom_order">自定义顺序</option>
          </select>
        </label>
        <button class="secondary-button" type="button" @click="librarySortDescending = !librarySortDescending">
          {{ librarySortDescending ? "降序" : "升序" }}
        </button>
        <button
          v-if="libraryQuery || libraryGroupFilter || librarySort !== 'updated_at' || !librarySortDescending"
          class="text-button"
          type="button"
          @click="libraryQuery = ''; libraryGroupFilter = ''; librarySort = 'updated_at'; librarySortDescending = true"
        >
          清除筛选
        </button>
        <button class="secondary-button" type="button" @click="toggleVisibleBookSelection">
          {{ allVisibleBooksSelected ? "取消全选" : "全选当前" }}
        </button>
        <span v-if="selectedBookIds.length" class="library-selection-count">
          已选 {{ selectedBookIds.length }} 本
        </span>
        <button class="secondary-button" type="button" @click="toggleDuplicatePanel">
          {{ duplicatePanelOpen ? "隐藏重复书" : "检查重复书" }}
        </button>
      </div>

      <section
        v-if="selectedBookIds.length"
        class="batch-metadata-panel"
        aria-label="批量编辑书籍元数据"
      >
        <div class="batch-metadata-heading">
          <div>
            <span class="eyebrow">BATCH EDIT</span>
            <h2>批量设置分组和标签</h2>
          </div>
          <span>空值会清空对应字段</span>
        </div>
        <div class="batch-metadata-fields">
          <label>
            <span>分组</span>
            <input
              v-model="batchMetadataGroupDraft"
              type="text"
              maxlength="128"
              placeholder="例如：待读 / 收藏"
            />
          </label>
          <label>
            <span>标签</span>
            <input
              v-model="batchMetadataTagsDraft"
              type="text"
              placeholder="用逗号分隔多个标签"
            />
          </label>
        </div>
        <div class="batch-metadata-actions">
          <button
            class="secondary-button"
            type="button"
            :disabled="batchMetadataBusy"
            @click="saveBatchBookMetadata"
          >
            {{ batchMetadataBusy ? "保存中…" : "保存批量修改" }}
          </button>
          <button
            class="text-button"
            type="button"
            :disabled="batchMetadataBusy"
            @click="clearBookSelection"
          >
            取消选择
          </button>
        </div>
      </section>

      <section
        v-if="duplicatePanelOpen"
        class="duplicate-books-panel"
        aria-label="重复书候选"
      >
        <div class="duplicate-books-heading">
          <div>
            <span class="eyebrow">DUPLICATE REVIEW</span>
            <h2>重复书候选</h2>
          </div>
          <button
            class="text-button"
            type="button"
            :disabled="duplicateBusy"
            @click="loadDuplicateGroups"
          >
            {{ duplicateBusy ? "检查中…" : "重新检查" }}
          </button>
        </div>
        <p v-if="!duplicateBusy && !duplicateGroups.length" class="duplicate-books-empty">
          暂未发现同名、同作者、同格式的重复记录。
        </p>
        <div v-else class="duplicate-books-list">
          <article v-for="group in duplicateGroups" :key="group.key" class="duplicate-book-group">
            <div class="duplicate-book-group-heading">
              <strong>{{ group.books[0].title }}</strong>
              <span>
                {{ group.books[0].author || "未知作者" }} ·
                {{ group.books[0].format.toUpperCase() }} ·
                {{ group.books.length }} 条记录
              </span>
            </div>
            <ul>
              <li v-for="book in group.books" :key="book.id">
                <span>{{ book.id }} · {{ book.chapter_count }} 章 · {{ formatProgress(book.progress) }}</span>
                <span>{{ book.shelf_group || "未分组" }}</span>
                <button
                  class="text-button duplicate-preview-button"
                  type="button"
                  :disabled="duplicatePreviewBusy"
                  @click.stop="previewDuplicateMerge(group, book.id)"
                >
                  {{ duplicatePreviewBusy ? "预览中…" : "保留并预览" }}
                </button>
              </li>
            </ul>
          </article>
        </div>
        <section v-if="duplicatePreview" class="duplicate-preview-panel" aria-live="polite">
          <div class="duplicate-preview-heading">
            <div>
              <span class="eyebrow">MERGE PREVIEW</span>
              <h3>只读合并预览</h3>
            </div>
            <button
              class="text-button"
              type="button"
              :disabled="duplicatePreviewBusy"
              @click="revalidateDuplicateMerge"
            >
              {{ duplicatePreviewBusy ? "验证中…" : "重新验证" }}
            </button>
            <button class="text-button" type="button" @click="clearDuplicatePreview">关闭</button>
          </div>
          <p class="duplicate-preview-note">
            保留书籍 ID：{{ duplicatePreview.canonical_book_id }}
            · 预览有效期约 5 分钟 · 尚未写入任何数据
          </p>
          <div class="duplicate-preview-books">
            <div v-for="book in duplicatePreview.books" :key="book.id" class="duplicate-preview-book">
              <strong>{{ book.id }}</strong>
              <span>{{ book.chapter_count }} 章 · {{ formatProgress(book.progress) }} · {{ book.shelf_group || "未分组" }}</span>
              <small v-if="book.id === duplicatePreview.canonical_book_id">保留项</small>
              <small v-else>将归档（预览）</small>
            </div>
          </div>
          <div class="duplicate-preview-stats">
            <span>相同章节 {{ duplicatePreview.identical_chapter_count }}</span>
            <span>可追加章节 {{ duplicatePreview.append_candidates.length }}</span>
            <span>标题冲突 {{ duplicatePreview.chapter_conflicts.length }}</span>
            <span>标签建议 {{ duplicatePreview.suggested_tags.length }}</span>
          </div>
          <ul v-if="duplicatePreview.conflicts.length" class="duplicate-preview-conflicts">
            <li v-for="conflict in duplicatePreview.conflicts" :key="conflict">{{ conflict }}</li>
          </ul>
          <ul v-if="duplicatePreview.blocked_reasons.length" class="duplicate-preview-blocked">
            <li v-for="reason in duplicatePreview.blocked_reasons" :key="reason">{{ reason }}</li>
          </ul>
          <p v-if="duplicatePreview.append_candidates.length" class="duplicate-preview-note">
            可追加：{{ duplicatePreview.append_candidates.map((chapter) => chapter.title).join("、") }}
          </p>
          <p v-if="duplicatePreview.chapter_conflicts.length" class="duplicate-preview-note">
            冲突章节：{{ duplicatePreview.chapter_conflicts.map((chapter) => chapter.title).join("、") }}
          </p>
        </section>
        <p class="duplicate-books-note">
          当前只提供候选预览，不会自动删除或覆盖书籍；确认保留项后再进入合并操作。
        </p>
      </section>

      <section v-if="bookImportPreview" class="book-import-preview" aria-live="polite">
        <div class="book-import-preview-heading">
          <div>
            <span class="eyebrow">TXT IMPORT PREVIEW</span>
            <h2>导入前确认</h2>
          </div>
          <span class="book-import-preview-count">{{ bookImportPreview.chapter_count }} 章</span>
        </div>
        <div class="book-import-preview-meta">
          <span>文件：{{ bookImportFileName }}</span>
          <span>编码：{{ bookImportPreview.encoding || "自动识别" }}</span>
          <span>格式：{{ bookImportPreview.format.toUpperCase() }}</span>
          <span v-if="bookImportPreview.first_chapter_title">首章：{{ bookImportPreview.first_chapter_title }}</span>
        </div>
        <div class="book-import-preview-controls">
          <label class="book-import-preview-field">
            <span>章节识别</span>
            <select v-model="txtParseOptions.chapter_rule" @change="ensureTxtPattern">
              <option value="auto">自动识别</option>
              <option value="disabled">不拆分章节</option>
              <option value="regex">自定义正则</option>
            </select>
          </label>
          <label class="book-import-preview-field">
            <span>章节标题正则</span>
            <input
              v-model="txtParseOptions.custom_pattern"
              type="text"
              :disabled="txtParseOptions.chapter_rule !== 'regex'"
              placeholder="例如：^第\s*\d+章"
              @keyup.enter="refreshBookImportPreview"
            />
          </label>
          <label class="book-import-preview-check">
            <input v-model="txtParseOptions.normalize_full_width_space" type="checkbox" />
            <span>将全角空格归一化为普通空格</span>
          </label>
          <label class="book-import-preview-field book-import-preview-wide">
            <span>文本替换（可选，每行一条：旧词 =&gt; 新词）</span>
            <textarea
              v-model="txtReplacementDraft"
              rows="2"
              placeholder="例如：旧书名 =&gt; 新书名"
              @keyup.ctrl.enter="refreshBookImportPreview"
            ></textarea>
          </label>
        </div>
        <ul v-if="bookImportPreview.warnings.length" class="book-import-preview-warnings">
          <li v-for="warning in bookImportPreview.warnings" :key="warning">{{ warning }}</li>
        </ul>
        <div class="book-import-preview-actions">
          <span>修改识别规则后点击“更新预览”，确认无误再导入。</span>
          <button class="secondary-button" type="button" :disabled="bookImportBusy" @click="refreshBookImportPreview">
            {{ bookImportBusy ? "预览中…" : "更新预览" }}
          </button>
          <button class="import-button" type="button" :disabled="bookImportBusy || isImporting" @click="confirmBookImport">
            {{ isImporting ? "导入中…" : "确认导入" }}
          </button>
          <button class="text-button" type="button" :disabled="bookImportBusy || isImporting" @click="cancelBookImportPreview">
            取消
          </button>
        </div>
      </section>


      <section v-if="imagePreview" class="image-import-preview" aria-live="polite">
        <div class="book-import-preview-heading">
          <div>
            <span class="eyebrow">IMAGE SINGLE-PAGE PREVIEW</span>
            <h2>图片预览</h2>
          </div>
          <span class="book-import-preview-count">{{ imagePreview.width }}×{{ imagePreview.height }}</span>
        </div>
        <div class="book-import-preview-meta">
          <span>文件：{{ imagePreview.file_name }}</span>
          <span>格式：{{ imagePreview.mime }}</span>
          <span>色彩：{{ imagePreview.color_type }}</span>
          <span>解码缓冲：{{ formatBytes(imagePreview.decoded_bytes) }}</span>
        </div>
        <div class="image-import-preview-media">
          <img v-if="imagePreviewUrl" :src="imagePreviewUrl" :alt="imagePreview.file_name" />
        </div>
        <div class="book-import-preview-actions">
          <span>当前为单页、只读预览，不写入书架。</span>
          <button class="text-button" type="button" @click="cancelBookImportPreview">关闭预览</button>
        </div>
      </section>

      <section v-if="imageSequencePreview" class="image-sequence-preview" aria-live="polite">
        <div class="book-import-preview-heading">
          <div>
            <span class="eyebrow">IMAGE SEQUENCE PREVIEW</span>
            <h2>图片序列预览</h2>
          </div>
          <span class="book-import-preview-count">{{ imageSequencePreview.page_count }} 页</span>
        </div>
        <div class="book-import-preview-meta">
          <span>文件：{{ bookImportFileName }}</span>
          <span>总像素：{{ imageSequencePreview.total_pixels.toLocaleString() }}</span>
          <span>解码内存：{{ formatBytes(imageSequencePreview.total_decoded_bytes) }}</span>
          <span>已显示缩略图：{{ Math.min(imageSequencePreview.page_count, MAX_IMAGE_THUMBNAILS) }}</span>
          <span v-if="imageSequenceBookId">
            原文件：{{ imageSequenceStateLabel(imageSequenceRecordState) }} · 可用 {{ imageSequenceReadyPages }} 页 · 变化 {{ imageSequenceStalePages }} 页 · 缺失 {{ imageSequenceMissingPages }} 页
          </span>
          <span v-if="imageSequenceLocation">缓存键：{{ imageSequenceLocation.cache_key.slice(0, 16) }}…</span>
          <span v-if="imageThumbnailCache">
            磁盘缓存：命中 {{ imageThumbnailCache.cache_hits }} 页、写入 {{ imageThumbnailCache.cache_writes }} 页 · {{ formatBytes(imageThumbnailCache.cache_bytes) }}
          </span>
          <span v-if="imageThumbnailCache?.cleaned_temp_files">
            已清理崩溃临时文件：{{ imageThumbnailCache.cleaned_temp_files }} 个
          </span>
        </div>
        <div class="book-import-preview-controls image-sequence-controls">
          <label class="book-import-preview-field">
            <span>书架标题</span>
            <input
              v-model="imageSequenceBookTitle"
              :disabled="Boolean(imageSequenceBookId)"
              type="text"
              placeholder="例如：我的漫画"
            />
          </label>
          <label class="book-import-preview-field">
            <span>图片根目录（绝对路径）</span>
            <input
              v-model="imageSequenceRootPath"
              :disabled="Boolean(imageSequenceBookId)"
              type="text"
              placeholder="例如：C:/Books/MyComic"
            />
            <button
              v-if="!imageSequenceBookId"
              class="text-button"
              type="button"
              :disabled="imageCacheBusy || isImporting"
              @click="chooseImageSequenceRoot"
            >
              选择目录
            </button>
          </label>
          <label class="book-import-preview-field">
            <span>阅读方向</span>
            <select v-model="imageSequenceDirection" @change="updateImageSequenceLayout">
              <option value="ltr">从左到右</option>
              <option value="rtl">从右到左</option>
              <option value="vertical">纵向长图</option>
            </select>
          </label>
          <label class="book-import-preview-field">
            <span>排版模式</span>
            <select v-model="imageSequenceSpread" @change="updateImageSequenceLayout">
              <option value="single">单页</option>
              <option value="double">双页</option>
              <option value="long_strip">长图</option>
            </select>
          </label>
        </div>
        <div v-if="imageRelinkPreview" class="image-relink-panel">
          <div class="image-relink-heading">
            <div>
              <span class="eyebrow">重新关联预览</span>
              <strong>{{ imageRelinkPreview.new_root_path }}</strong>
            </div>
            <span>{{ imageRelinkPreview.matched_pages }} 页路径匹配 · {{ imageRelinkPreview.changed_pages }} 页待复核 · {{ imageRelinkPreview.missing_pages }} 页缺失 · {{ imageRelinkPreview.added_files }} 个新增文件</span>
          </div>
          <p v-if="imageRelinkPreview.reordered" class="image-relink-warning">检测到新目录的文件名顺序与原页面顺序不同；应用后仍保留原页码顺序。</p>
          <p class="image-relink-note">只有确认后才会更新数据库。待复核页会保留为“文件变化”，不会把仅凭文件名和大小找到的候选误报为完全一致。</p>
          <ul class="image-relink-list">
            <li v-for="assignment in imageRelinkPreview.assignments.slice(0, 8)" :key="assignment.page_index">
              <span>#{{ assignment.page_index + 1 }} · {{ assignment.old_relative_path }}</span>
              <strong>{{ imageRelinkAssignmentLabel(assignment) }}</strong>
              <span v-if="assignment.new_relative_path">→ {{ assignment.new_relative_path }}</span>
            </li>
          </ul>
          <div class="book-import-preview-actions">
            <button class="text-button" type="button" :disabled="imageRelinkApplying" @click="applyImageSequenceRelink">
              {{ imageRelinkApplying ? "正在应用…" : "确认重新关联" }}
            </button>
            <button class="text-button" type="button" :disabled="imageRelinkApplying" @click="imageRelinkPreview = null">取消</button>
          </div>
        </div>
        <div v-if="imageSequenceLocation" class="image-sequence-reader">
          <div class="image-sequence-reader-toolbar">
            <button class="text-button" type="button" :disabled="imageSequenceLocation.page_index <= 0" @click="moveImageSequencePage(-1)">上一页</button>
            <span>第 {{ imageSequenceLocation.page_index + 1 }} / {{ imageSequencePreview.page_count }} 页</span>
            <button class="text-button" type="button" :disabled="imageSequenceLocation.page_index >= imageSequencePreview.page_count - 1" @click="moveImageSequencePage(1)">下一页</button>
            <label>
              <span>缩放</span>
              <input
                type="range"
                min="0.5"
                max="3"
                step="0.1"
                :value="imageSequenceLocation.zoom"
                @input="handleImageSequenceZoom"
              />
              <span>{{ Math.round(imageSequenceLocation.zoom * 100) }}%</span>
            </label>
          </div>
          <div class="image-sequence-reader-media">
            <div
              v-if="imageSequencePageStateLabel(imageSequenceLocation.page_index)"
              class="image-sequence-page-warning"
              role="status"
            >
              <strong>{{ imageSequencePageStateLabel(imageSequenceLocation.page_index) }}</strong>
              <span v-if="imageSequencePageStates[imageSequenceLocation.page_index] === 'missing'">
                已隐藏可能过期的缩略图，选择新目录后可重新关联。
              </span>
              <span v-else>当前缓存仅供参考，确认目录后再继续阅读。</span>
              <div class="image-sequence-page-warning-actions">
                <button
                  class="text-button"
                  type="button"
                  :disabled="imageRelinkBusy || imageRelinkApplying || imageCacheBusy"
                  @click="chooseImageSequenceRelinkRoot"
                >重新关联目录</button>
                <button
                  v-if="imageSequencePageStates[imageSequenceLocation.page_index] === 'stale'"
                  class="text-button"
                  type="button"
                  :disabled="imageDigestBusy || imageRelinkBusy || imageRelinkApplying || imageCacheBusy"
                  @click="verifyImageSequenceDigests"
                >{{ imageDigestBusy ? "正在复核…" : "复核变化页" }}</button>
              </div>
            </div>
            <img
              v-if="imageSequencePageUrl(imageSequenceLocation.page_index)"
              :src="imageSequencePageUrl(imageSequenceLocation.page_index)"
              :alt="imageSequencePreview.pages[imageSequenceLocation.page_index]?.file_name || '图片页面'"
              :style="{ transform: `scale(${imageSequenceLocation.zoom})` }"
            />
            <p v-else>当前页面未生成临时缩略图；先选择前 {{ MAX_IMAGE_THUMBNAILS }} 页可预览。</p>
          </div>
        </div>
        <div class="image-sequence-grid">
          <figure
            v-for="page in imageSequencePreview.pages.slice(0, MAX_IMAGE_THUMBNAILS)"
            :key="page.index"
            class="image-sequence-thumb"
            :class="{ selected: imageSequenceLocation?.page_index === page.index }"
            tabindex="0"
            @click="selectImageSequencePage(page.index)"
            @keydown.enter="selectImageSequencePage(page.index)"
          >
            <img
              v-if="imageSequencePageUrl(page.index)"
              :src="imageSequencePageUrl(page.index)"
              :alt="page.file_name"
            />
            <figcaption>#{{ page.index + 1 }} · {{ page.file_name }}</figcaption>
          </figure>
        </div>
        <div class="book-import-preview-actions">
          <span v-if="imageSequenceBookId">
            已恢复书架记录：{{ imageSequenceStateLabel(imageSequenceRecordState) }}；可用“选择新目录并扫描”生成差异预览。
          </span>
          <span v-else>填写图片根目录绝对路径后保存到书架；位置和缩略图会按内容摘要键保存在本机。</span>
          <button
            v-if="imageSequenceBookId"
            class="text-button"
            type="button"
            :disabled="imageRelinkBusy || imageRelinkApplying || imageCacheBusy"
            @click="chooseImageSequenceRelinkRoot"
          >
            {{ imageRelinkBusy ? "正在扫描…" : "选择新目录并扫描" }}
          </button>
          <button
            v-if="imageRelinkBusy"
            class="text-button"
            type="button"
            @click="cancelImageRelinkScan"
          >
            取消扫描
          </button>
          <button
            v-if="imageSequenceBookId && imageSequenceStalePages > 0"
            class="text-button"
            type="button"
            :disabled="imageDigestBusy || imageRelinkBusy || imageRelinkApplying || imageCacheBusy"
            @click="verifyImageSequenceDigests"
          >
            {{ imageDigestBusy ? "正在复核…" : "复核变化页" }}
          </button>
          <button
            v-if="!imageSequenceBookId"
            class="text-button"
            type="button"
            :disabled="imageCacheBusy || isImporting || !imageSequenceRootPath.trim()"
            @click="saveImageSequenceToLibrary"
          >
            保存到书架
          </button>
          <button v-if="imageCacheBusy" class="text-button" type="button" @click="cancelImageCache">取消缓存</button>
          <button
            v-else-if="imageSequenceInputs.length"
            class="text-button"
            type="button"
            @click="retryImageSequenceCache"
          >
            重试缓存
          </button>
          <button class="text-button" type="button" @click="cancelBookImportPreview">关闭预览</button>
        </div>
      </section>

      <LibraryOverview
        :books="books"
        :continue-book="continueBook"
        :recent-books="recentBooks"
        @continue="continueReading"
        @import="openFilePicker"
        @sources="openSources"
      />

      <section
        v-if="searchResult"
        class="search-results-panel online-search-section"
        aria-live="polite"
        aria-labelledby="online-search-heading"
      >
        <div class="search-results-heading">
          <div>
            <span class="eyebrow">MULTI-SOURCE SEARCH</span>
            <h2 id="online-search-heading">在线搜索结果</h2>
          </div>
          <button class="source-link-button" type="button" @click="clearSearch">清除</button>
        </div>
        <p class="search-results-context">
          当前关键词：{{ searchKeyword }}
        </p>
        <p v-if="!searchResult.results.length" class="search-results-empty">没有找到匹配书籍。</p>
        <div v-else class="search-results-list">
          <button
            v-for="item in searchResult.results"
            :key="item.source_id + '-' + item.title + '-' + (item.author || '')"
            class="search-result-row"
            :class="{ clickable: Boolean(item.book_url), loading: remoteBusy && Boolean(item.book_url) }"
            :disabled="!item.book_url || remoteBusy"
            type="button"
            :aria-label="item.book_url ? '打开 ' + (item.title || '未命名书籍') : (item.title || '未命名书籍') + ' 没有可用链接'"
            @click="openRemoteBook(item)"
          >
            <span class="search-result-copy">
              <strong>{{ item.title || "未命名书籍" }}</strong>
              <span>{{ item.author || "作者未知" }}</span>
            </span>
            <span class="search-result-actions">
              <span class="search-source-badge">{{ item.source_name }}</span>
              <span class="search-open-label">{{ item.book_url ? (remoteBusy ? "加载中…" : "打开") : "无链接" }}</span>
            </span>
          </button>
        </div>
        <div v-if="searchResult.failures.length" class="search-failures">
          <strong>部分书源暂不可用</strong>
          <div v-for="failure in searchResult.failures" :key="failure.source_id" class="search-failure-row">
            <p>{{ failure.source_name }}</p>
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
      </section>

      <section class="local-shelf-section" aria-labelledby="local-shelf-heading">
        <div class="library-section-heading">
          <div>
            <span class="eyebrow">LOCAL SHELF</span>
            <h2 id="local-shelf-heading">本地书架</h2>
          </div>
          <span class="library-section-caption">{{ books.length }} 本已保存到本机</span>
        </div>
      <section v-if="books.length" class="library-grid" aria-label="本地书架">
        <article
          v-for="book in books"
          :key="book.id"
          class="book-card"
          :class="{ selected: selectedBookIds.includes(book.id) }"
          tabindex="0"
          @click="continueReading(book)"
          @keydown.enter="continueReading(book)"
        >
          <label class="book-select-control" @click.stop>
            <input
              type="checkbox"
              :checked="selectedBookIds.includes(book.id)"
              :aria-label="'选择 ' + book.title"
              @change="toggleBookSelection(book.id)"
            />
            <span>选择</span>
          </label>
          <div class="book-cover" :class="`format-${book.format}`">
            <span>{{ book.format.toUpperCase() }}</span>
            <small
              v-if="coverStateLabel(book)"
              class="book-cover-status"
              :class="coverStateClass(book)"
            >{{ coverStateLabel(book) }}</small>
          </div>
          <div class="book-card-body">
            <span class="book-format">{{ book.chapter_count }} 章 · {{ formatProgress(book.progress) }}</span>
            <h2>{{ book.title }}</h2>
            <p>{{ book.author || "本地导入" }}</p>
            <div v-if="book.shelf_group || book.tags.length" class="book-metadata-chips" aria-label="书籍分组和标签">
              <span v-if="book.shelf_group" class="book-metadata-chip">{{ book.shelf_group }}</span>
              <span v-for="tag in book.tags.slice(0, 4)" :key="tag" class="book-metadata-chip tag">{{ tag }}</span>
            </div>
            <span
              v-if="imageSequenceHealthLabel(book)"
              class="book-health-badge"
              :class="imageSequenceHealthClass(book)"
            >{{ imageSequenceHealthLabel(book) }}</span>
            <div class="progress-track"><span :style="{ width: `${book.progress * 100}%` }"></span></div>
            <button class="text-button book-edit-button" type="button" @click.stop="beginBookMetadataEdit(book)">编辑分组和标签</button>
            <div v-if="editingBookId === book.id" class="book-metadata-editor" @click.stop>
              <label>
                <span>分组</span>
                <input v-model="metadataGroupDraft" type="text" maxlength="128" placeholder="例如：待读 / 收藏" />
              </label>
              <label>
                <span>标签</span>
                <input v-model="metadataTagsDraft" type="text" placeholder="用逗号分隔多个标签" />
              </label>
              <label>
                <span>顺序</span>
                <input v-model="metadataOrderDraft" type="number" step="1" />
              </label>
              <div class="book-metadata-editor-actions">
                <button class="secondary-button" type="button" :disabled="metadataBusy" @click="saveBookMetadata(book)">
                  {{ metadataBusy ? "保存中…" : "保存" }}
                </button>
                <button class="text-button" type="button" :disabled="metadataBusy" @click="cancelBookMetadataEdit">取消</button>
              </div>
            </div>
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
    </section>


    <SourceView v-else-if="view === 'sources'" />

    
    <section v-else-if="view === 'settings'" class="content settings-content" id="settings">
      <header class="topbar settings-topbar">
        <div>
          <span class="eyebrow">APP & READER SETTINGS</span>
          <h1>设置</h1>
        </div>
        <div class="settings-topbar-status">
          <span class="settings-save-dot"></span>
          <span>本机自动保存</span>
          <button class="secondary-button" type="button" @click="closeSettings">返回阅读</button>
        </div>
      </header>

      <div class="settings-layout">
        <aside class="settings-nav" aria-label="设置分类">
          <span class="settings-nav-title">SETTINGS</span>
          <button type="button" :class="{ active: settingsSection === 'reader' }" @click="settingsSection = 'reader'">
            <span>阅读排版</span>
            <small>字号与版心</small>
          </button>
          <button type="button" :class="{ active: settingsSection === 'appearance' }" @click="settingsSection = 'appearance'">
            <span>主题与界面</span>
            <small>颜色与字体</small>
          </button>
          <button type="button" :class="{ active: settingsSection === 'network' }" @click="settingsSection = 'network'">
            <span>远端阅读</span>
            <small>追链安全</small>
          </button>
          <div class="settings-nav-tip">
            <strong>阅读体验</strong>
            <span>调整会实时应用到当前章节，并同步保存到本机。</span>
          </div>
        </aside>

        <div class="settings-main">
          <div class="settings-intro">
            <span class="eyebrow">PERSONALIZE YOUR DESK</span>
            <strong>把每一页调成适合自己的节奏。</strong>
            <span>从阅读预设开始，再微调字体、版心、主题和远端追链策略。</span>
          </div>

          <ReaderSettingsPanel v-if="settingsSection === 'reader'" v-model="settings" @reset="resetSettings" />

          <section v-else-if="settingsSection === 'appearance'" class="settings-panel appearance-panel">
            <div class="settings-section-heading">
              <div>
                <span class="eyebrow">INTERFACE</span>
                <h2>主题与界面</h2>
              </div>
              <span class="settings-section-status">即时预览</span>
            </div>
            <p class="settings-note appearance-note">先选择一个基准主题，再使用自定义颜色做细节调整。阅读页面和书源工作区会共享这套低对比度界面语言。</p>
            <div class="appearance-theme-grid">
              <button class="appearance-theme-card" :class="{ selected: settings.theme === 'night' }" type="button" @click="settings.theme = 'night'">
                <span class="appearance-theme-sample theme-night-sample"><i></i><i></i><i></i></span>
                <strong>夜间</strong>
                <small>低亮度、长时间阅读</small>
              </button>
              <button class="appearance-theme-card" :class="{ selected: settings.theme === 'paper' }" type="button" @click="settings.theme = 'paper'">
                <span class="appearance-theme-sample theme-paper-sample"><i></i><i></i><i></i></span>
                <strong>纸张</strong>
                <small>清晰明亮、适合白天</small>
              </button>
              <button class="appearance-theme-card" :class="{ selected: settings.theme === 'sepia' }" type="button" @click="settings.theme = 'sepia'">
                <span class="appearance-theme-sample theme-sepia-sample"><i></i><i></i><i></i></span>
                <strong>暖色</strong>
                <small>柔和护眼、降低冷感</small>
              </button>
              <button class="appearance-theme-card" :class="{ selected: settings.theme === 'custom' }" type="button" @click="settings.theme = 'custom'">
                <span class="appearance-theme-sample theme-custom-sample"><i></i><i></i><i></i></span>
                <strong>自定义</strong>
                <small>使用自己的颜色方案</small>
              </button>
            </div>
            <div class="settings-grid appearance-controls">
              <label class="settings-field">
                <span>界面字体</span>
                <select v-model="settings.fontFamily">
                  <option value="system">系统无衬线</option>
                  <option value="yahei">微软雅黑</option>
                  <option value="serif">宋体 / 衬线</option>
                  <option value="kai">楷体</option>
                </select>
              </label>
              <label class="settings-field">
                <span>自定义背景色</span>
                <input v-model="settings.customBackground" type="color" :disabled="settings.theme !== 'custom'" />
              </label>
              <label class="settings-field">
                <span>自定义文字色</span>
                <input v-model="settings.customText" type="color" :disabled="settings.theme !== 'custom'" />
              </label>
              <label class="settings-field">
                <span>自定义强调色</span>
                <input v-model="settings.customAccent" type="color" :disabled="settings.theme !== 'custom'" />
              </label>
            </div>
            <div class="appearance-shortcuts">
              <span class="eyebrow">QUICK TIPS</span>
              <p><kbd>Ctrl</kbd><span>+</span><kbd>+</kbd> 放大字号，<kbd>Ctrl</kbd><span>+</span><kbd>-</kbd> 缩小字号，阅读时按 <kbd>T</kbd> 可切换主题。</p>
            </div>
          </section>

          <div v-else class="settings-network-workspace">
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
          </div>
        </div>
      </div>
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

.library-filter-bar {
  display: flex;
  align-items: end;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 18px;
  padding: 14px;
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 12px;
  background: rgba(15, 24, 38, 0.62);
}

.library-selection-count {
  align-self: center;
  color: #b9f6dd;
  font-size: 12px;
  white-space: nowrap;
}

.duplicate-books-panel {
  display: grid;
  gap: 12px;
  margin-top: 14px;
  padding: 16px;
  border: 1px solid rgba(255, 207, 155, 0.28);
  border-radius: 14px;
  background: rgba(74, 52, 31, 0.24);
}

.duplicate-books-heading {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 14px;
}

.duplicate-books-heading h2 {
  margin: 8px 0 0;
  color: #fff0dd;
  font-size: 17px;
}

.duplicate-books-list {
  display: grid;
  gap: 9px;
}

.duplicate-book-group {
  padding: 11px 12px;
  border: 1px solid rgba(255, 207, 155, 0.16);
  border-radius: 10px;
  background: rgba(12, 17, 27, 0.42);
}

.duplicate-book-group-heading {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.duplicate-book-group-heading strong {
  overflow: hidden;
  color: #f7e4ca;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.duplicate-book-group-heading span,
.duplicate-book-group li {
  color: #bfae98;
  font-size: 11px;
}

.duplicate-book-group ul {
  display: grid;
  gap: 5px;
  margin: 9px 0 0;
  padding: 0;
  list-style: none;
}

.duplicate-book-group li {
  display: flex;
  justify-content: space-between;
  gap: 9px;
  padding-top: 5px;
  border-top: 1px solid rgba(255, 207, 155, 0.1);
}

.duplicate-books-empty,
.duplicate-books-note {
  margin: 0;
  color: #bfae98;
  font-size: 12px;
  line-height: 1.55;
}


.duplicate-preview-panel {
  display: grid;
  gap: 10px;
  padding: 13px;
  border: 1px solid rgba(121, 201, 255, 0.26);
  border-radius: 11px;
  background: rgba(28, 48, 74, 0.3);
}

.duplicate-preview-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.duplicate-preview-heading h3 {
  margin: 7px 0 0;
  color: #e7f3ff;
  font-size: 15px;
}

.duplicate-preview-note {
  margin: 0;
  color: #a9bdd3;
  font-size: 11px;
  line-height: 1.55;
}

.duplicate-preview-books {
  display: grid;
  gap: 6px;
}

.duplicate-preview-book {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
  padding: 7px 8px;
  border-radius: 8px;
  background: rgba(12, 17, 27, 0.38);
  color: #b9c8dc;
  font-size: 11px;
}

.duplicate-preview-book strong {
  overflow: hidden;
  color: #dcecff;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.duplicate-preview-book small {
  color: #9be7d8;
  font-size: 10px;
}

.duplicate-preview-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  color: #b9c8dc;
  font-size: 11px;
}

.duplicate-preview-stats span {
  padding: 4px 7px;
  border-radius: 999px;
  background: rgba(121, 201, 255, 0.12);
}

.duplicate-preview-conflicts,
.duplicate-preview-blocked {
  display: grid;
  gap: 4px;
  margin: 0;
  padding-left: 18px;
  color: #ffd39b;
  font-size: 11px;
  line-height: 1.45;
}

.duplicate-preview-blocked {
  color: #ffb0bc;
}


.batch-metadata-panel {
  display: grid;
  gap: 14px;
  margin-top: 14px;
  padding: 16px;
  border: 1px solid rgba(134, 223, 194, 0.28);
  border-radius: 14px;
  background: rgba(22, 57, 54, 0.32);
}

.batch-metadata-heading {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 14px;
}

.batch-metadata-heading h2 {
  margin: 8px 0 0;
  color: #eafbf5;
  font-size: 17px;
}

.batch-metadata-heading > span {
  color: #9fb1c8;
  font-size: 11px;
}

.batch-metadata-fields {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.batch-metadata-fields label {
  display: grid;
  gap: 6px;
  color: #a9bdd3;
  font-size: 12px;
}

.batch-metadata-fields input {
  width: 100%;
  box-sizing: border-box;
  padding: 9px 10px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 8px;
  color: #dce7f7;
  background: #0c111b;
}

.batch-metadata-actions {
  display: flex;
  align-items: center;
  gap: 9px;
}

.library-filter-field {
  display: grid;
  min-width: 150px;
  gap: 6px;
  color: #9fb1c8;
  font-size: 12px;
}

.library-filter-field input,
.library-filter-field select {
  min-width: 150px;
  padding: 9px 10px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 8px;
  color: #dce7f7;
  background: #0c111b;
}

.library-filter-field input:focus,
.library-filter-field select:focus {
  border-color: rgba(139, 183, 255, 0.75);
  outline: none;
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

.search-results-heading,
.library-section-heading {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 18px;
}

.search-results-heading h2,
.library-section-heading h2 {
  margin: 9px 0 0;
  color: #eff5ff;
  font-size: 20px;
}

.search-results-context,
.search-results-empty {
  color: #8391a6;
  font-size: 12px;
  line-height: 1.6;
}

.search-results-context {
  margin: 14px 0 0;
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
  width: 100%;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 13px 14px;
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 10px;
  color: inherit;
  background: rgba(12, 17, 27, 0.52);
  cursor: default;
  font: inherit;
  text-align: left;
  transition: border-color 160ms ease, background 160ms ease, transform 160ms ease;
}

.search-result-row.clickable {
  cursor: pointer;
}

.search-result-row.clickable:hover,
.search-result-row.clickable:focus-visible {
  border-color: rgba(139, 183, 255, 0.7);
  outline: none;
  background: rgba(24, 49, 76, 0.76);
  transform: translateY(-1px);
}

.search-result-row:disabled {
  opacity: 0.76;
}

.search-result-row.loading {
  cursor: wait;
}

.search-result-copy {
  display: grid;
  min-width: 0;
  gap: 5px;
}

.search-result-copy strong {
  overflow: hidden;
  color: #edf5ff;
  font-size: 14px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.search-result-copy > span {
  overflow: hidden;
  color: #8391a6;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.search-failures > strong {
  color: #ffcf9b;
  font-size: 12px;
}

.search-failure-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 0 0;
}

.search-failure-row p {
  margin: 0;
  color: #ffb0bc;
  font-size: 11px;
}

.local-shelf-section {
  margin-top: 30px;
  padding-top: 24px;
  border-top: 1px solid rgba(148, 163, 184, 0.16);
}

.library-section-caption {
  color: #8391a6;
  font-size: 11px;
}

.local-shelf-section .library-grid,
.local-shelf-section .empty-state {
  margin-top: 18px;
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

.source-batch-note {
  margin: 8px 0 0;
  color: #8391a6;
  font-size: 11px;
}

.source-row[draggable="true"] {
  cursor: grab;
}

.source-row[draggable="true"]:active {
  cursor: grabbing;
}

.source-batch-group-input,
.source-batch-metadata-input {
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

@media (max-width: 720px) {
  .batch-metadata-fields {
    grid-template-columns: 1fr;
  }

  .batch-metadata-heading {
    align-items: start;
    flex-direction: column;
  }
}

@media (max-width: 720px) {
  .duplicate-book-group-heading,
  .duplicate-book-group li {
    align-items: start;
    flex-direction: column;
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


.image-import-preview {
  margin-top: 22px;
  padding: 20px;
  border: 1px solid rgba(134, 223, 194, 0.28);
  border-radius: 16px;
  background: rgba(17, 44, 48, 0.72);
}

.image-import-preview-media {
  display: grid;
  min-height: 180px;
  margin-top: 18px;
  place-items: center;
  padding: 18px;
  border: 1px dashed rgba(134, 223, 194, 0.35);
  border-radius: 12px;
  background: rgba(12, 17, 27, 0.52);
}

.image-import-preview-media img {
  display: block;
  max-width: min(100%, 860px);
  max-height: 520px;
  object-fit: contain;
}

.image-sequence-preview {
  margin-top: 22px;
  padding: 20px;
  border: 1px solid rgba(134, 223, 194, 0.28);
  border-radius: 16px;
  background: rgba(17, 44, 48, 0.72);
}

.image-sequence-controls {
  grid-template-columns: repeat(2, minmax(170px, 1fr));
}

.image-sequence-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(132px, 1fr));
  gap: 12px;
  margin-top: 18px;
}

.image-sequence-reader {
  margin-top: 18px;
  padding: 12px;
  border: 1px solid rgba(134, 223, 194, 0.2);
  border-radius: 12px;
  background: rgba(12, 17, 27, 0.42);
}

.image-sequence-reader-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  color: #aebbd0;
  font-size: 11px;
}

.image-sequence-reader-toolbar > span {
  min-width: 120px;
  text-align: center;
}

.image-sequence-reader-toolbar label {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-left: auto;
}

.image-sequence-reader-toolbar input {
  width: 120px;
  accent-color: #86dfc2;
}

.image-sequence-reader-media {
  display: grid;
  min-height: 220px;
  margin-top: 12px;
  place-items: center;
  overflow: auto;
  padding: 16px;
  border: 1px dashed rgba(134, 223, 194, 0.25);
  border-radius: 10px;
  background: rgba(12, 17, 27, 0.72);
}

.image-sequence-reader-media img {
  display: block;
  max-width: 100%;
  max-height: 560px;
  object-fit: contain;
  transform-origin: center center;
  transition: transform 120ms ease;
}

.image-sequence-reader-media p {
  color: #8391a6;
  font-size: 11px;
}

.image-sequence-page-warning {
  display: grid;
  max-width: 520px;
  gap: 8px;
  padding: 14px;
  border: 1px solid rgba(255, 176, 188, 0.3);
  border-radius: 10px;
  color: #ffcf9b;
  background: rgba(139, 90, 34, 0.18);
  font-size: 11px;
  line-height: 1.5;
  text-align: center;
}

.image-sequence-page-warning strong {
  color: #ffe2bb;
  font-size: 13px;
}

.image-sequence-page-warning-actions {
  display: flex;
  justify-content: center;
  flex-wrap: wrap;
  gap: 8px;
}

.image-sequence-thumb {
  min-width: 0;
  margin: 0;
  overflow: hidden;
  border: 1px solid rgba(134, 223, 194, 0.2);
  border-radius: 10px;
  background: rgba(12, 17, 27, 0.58);
}

.image-sequence-thumb img {
  display: block;
  width: 100%;
  height: 150px;
  object-fit: contain;
  background: rgba(12, 17, 27, 0.72);
}

.image-sequence-thumb.selected {
  border-color: rgba(134, 223, 194, 0.86);
  box-shadow: 0 0 0 2px rgba(134, 223, 194, 0.16);
}

.image-sequence-thumb:focus-visible {
  outline: 2px solid rgba(121, 201, 255, 0.82);
  outline-offset: 2px;
}

.image-sequence-thumb figcaption {
  overflow: hidden;
  padding: 8px;
  color: #aebbd0;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-import-preview {
  margin-top: 22px;
  padding: 20px;
  border: 1px solid rgba(121, 201, 255, 0.26);
  border-radius: 16px;
  background: rgba(17, 34, 52, 0.72);
}

.book-import-preview-heading {
  display: flex;
  align-items: start;
  justify-content: space-between;
  gap: 16px;
}

.book-import-preview-heading h2 {
  margin: 8px 0 0;
  font-size: 20px;
}

.book-import-preview-count {
  color: #b9f6dd;
  font-size: 13px;
  font-weight: 750;
}

.book-import-preview-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 16px;
  color: #aebbd0;
  font-size: 11px;
}

.book-import-preview-meta span {
  padding: 5px 8px;
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 999px;
  background: rgba(12, 17, 27, 0.45);
}

.book-import-preview-controls {
  display: grid;
  grid-template-columns: minmax(170px, 0.55fr) minmax(240px, 1.45fr);
  gap: 12px;
  margin-top: 18px;
}

.book-import-preview-field {
  display: grid;
  gap: 7px;
  color: #aebbd0;
  font-size: 12px;
}

.book-import-preview-field input,
.book-import-preview-field select,
.book-import-preview-field textarea {
  width: 100%;
  padding: 10px 11px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 9px;
  color: #dce7f7;
  background: #0c111b;
}

.book-import-preview-field textarea {
  min-height: 54px;
  resize: vertical;
}

.book-import-preview-field input:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.book-import-preview-check {
  display: flex;
  align-items: center;
  align-self: end;
  gap: 8px;
  min-height: 40px;
  color: #aebbd0;
  font-size: 12px;
}

.book-import-preview-check input {
  width: 16px;
  height: 16px;
  accent-color: #86dfc2;
}

.book-import-preview-wide {
  grid-column: 1 / -1;
}

.book-import-preview-warnings {
  display: grid;
  gap: 6px;
  margin: 16px 0 0;
  padding: 0;
  color: #e3c788;
  font-size: 11px;
  line-height: 1.5;
  list-style: none;
}

.book-import-preview-warnings li {
  padding: 8px 10px;
  border-left: 2px solid #e3c788;
  background: rgba(139, 90, 34, 0.16);
}

.book-import-preview-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 9px;
  margin-top: 18px;
}

.book-import-preview-actions span {
  flex: 1 1 240px;
  color: #8391a6;
  font-size: 11px;
  line-height: 1.5;
}

@media (max-width: 720px) {
  .book-import-preview-controls,
  .image-sequence-controls {
    grid-template-columns: 1fr;
  }

  .image-sequence-thumb img {
    height: 120px;
  }
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
  position: relative;
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

.book-card.selected {
  border-color: rgba(134, 223, 194, 0.72);
  box-shadow: 0 0 0 1px rgba(134, 223, 194, 0.18);
}

.book-select-control {
  position: absolute;
  z-index: 2;
  top: 11px;
  right: 11px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 7px;
  border: 1px solid rgba(148, 163, 184, 0.28);
  border-radius: 999px;
  color: #e7f8f2;
  background: rgba(7, 13, 23, 0.72);
  cursor: pointer;
  font-size: 10px;
}

.book-select-control input {
  width: 14px;
  height: 14px;
  margin: 0;
  accent-color: #86dfc2;
}

.book-cover {
  display: flex;
  height: 132px;
  flex-direction: column;
  align-items: flex-start;
  justify-content: flex-end;
  gap: 6px;
  padding: 16px;
  color: rgba(255, 255, 255, 0.76);
  background: linear-gradient(145deg, #263c73, #17233e);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.14em;
}

.book-cover-status {
  max-width: 100%;
  overflow: hidden;
  padding: 3px 6px;
  border-radius: 999px;
  color: #ffe0ab;
  background: rgba(77, 49, 20, 0.72);
  font-size: 9px;
  font-weight: 650;
  letter-spacing: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-cover-status.blocked {
  color: #ffbac4;
  background: rgba(94, 30, 46, 0.76);
}

.book-cover-status.missing {
  color: #c9d8ed;
  background: rgba(28, 42, 65, 0.78);
}

.book-cover.format-epub {
  background: linear-gradient(145deg, #6f4a8e, #2c244f);
}

.book-metadata-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 10px;
}

.book-metadata-chip {
  padding: 3px 7px;
  border-radius: 999px;
  color: #c8f6e4;
  background: rgba(47, 128, 104, 0.2);
  font-size: 10px;
}

.book-metadata-chip.tag {
  color: #c6d7ff;
  background: rgba(76, 104, 168, 0.24);
}

.book-edit-button {
  margin-top: 12px;
  padding: 0;
}

.book-metadata-editor {
  display: grid;
  gap: 8px;
  margin-top: 12px;
  padding: 10px;
  border: 1px solid rgba(139, 183, 255, 0.25);
  border-radius: 10px;
  background: rgba(7, 13, 23, 0.58);
}

.book-metadata-editor label {
  display: grid;
  gap: 4px;
  color: #9fb1c8;
  font-size: 11px;
}

.book-metadata-editor input {
  width: 100%;
  box-sizing: border-box;
  padding: 7px 8px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 7px;
  color: #dce7f7;
  background: #0c111b;
}

.book-metadata-editor-actions {
  display: flex;
  align-items: center;
  gap: 8px;
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

.book-health-badge {
  display: inline-flex;
  width: fit-content;
  margin-top: 10px;
  padding: 4px 8px;
  border-radius: 999px;
  color: #ffd39b;
  background: rgba(139, 90, 34, 0.2);
  font-size: 10px;
  line-height: 1.2;
}

.book-health-badge.ready {
  color: #b9f6dd;
  background: rgba(30, 101, 82, 0.24);
}

.book-health-badge.missing {
  color: #ffb0bc;
  background: rgba(188, 59, 83, 0.16);
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


.image-relink-panel {
  display: grid;
  gap: 12px;
  margin-top: 18px;
  padding: 16px;
  border: 1px solid rgba(139, 183, 255, 0.32);
  border-radius: 12px;
  background: rgba(39, 57, 94, 0.24);
}

.image-relink-heading {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 14px;
}

.image-relink-heading strong {
  display: block;
  overflow: hidden;
  margin-top: 5px;
  color: #eaf2ff;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.image-relink-heading > span,
.image-relink-note,
.image-relink-warning {
  margin: 0;
  color: #a7b8d0;
  font-size: 12px;
  line-height: 1.55;
}

.image-relink-warning {
  color: #ffd39b;
}

.image-relink-list {
  display: grid;
  gap: 7px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.image-relink-list li {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) auto minmax(0, 1fr);
  gap: 8px;
  align-items: center;
  padding: 7px 9px;
  border-radius: 8px;
  color: #aab9cf;
  background: rgba(12, 17, 27, 0.42);
  font-size: 11px;
}

.image-relink-list li > span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.image-relink-list li strong {
  color: #c8e7ff;
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}

@media (max-width: 720px) {
  .image-relink-heading {
    align-items: start;
    flex-direction: column;
  }

  .image-relink-list li {
    grid-template-columns: 1fr;
  }
}


/* Visual refresh: ink & parchment reading desk */
.nav-item .nav-icon {
  flex: 0 0 auto;
}

.nav-item .nav-label {
  color: inherit;
}

.nav-item .nav-meta {
  color: inherit;
}

.content {
  isolation: isolate;
}

.content > .topbar h1,
.source-content .topbar h1,
.settings-content .topbar h1 {
  text-wrap: balance;
}

.secondary-button,
.import-button,
.text-button,
.source-link-button,
.book-edit-button,
.toolbar-button {
  transition: transform 180ms ease, border-color 180ms ease, color 180ms ease, background 180ms ease, box-shadow 180ms ease;
}

.secondary-button {
  border: 1px solid rgba(232, 182, 111, 0.32) !important;
  border-radius: 11px !important;
  color: var(--gold-soft) !important;
  background: rgba(232, 182, 111, 0.08) !important;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

.secondary-button:hover:not(:disabled) {
  border-color: rgba(232, 182, 111, 0.7) !important;
  background: rgba(232, 182, 111, 0.15) !important;
  box-shadow: 0 10px 24px rgba(232, 182, 111, 0.1);
  transform: translateY(-1px);
}

.import-button {
  border: 1px solid rgba(243, 211, 157, 0.7) !important;
  border-radius: 11px !important;
  color: #111927 !important;
  background: linear-gradient(135deg, #f3d39d, #e8b66f) !important;
  box-shadow: 0 10px 25px rgba(232, 182, 111, 0.18);
}

.import-button:hover:not(:disabled) {
  box-shadow: 0 14px 30px rgba(232, 182, 111, 0.25);
  transform: translateY(-1px);
}

.text-button {
  border: 1px solid transparent !important;
  border-radius: 10px !important;
  color: var(--text-soft) !important;
  background: transparent !important;
}

.text-button:hover:not(:disabled) {
  border-color: rgba(232, 182, 111, 0.24) !important;
  color: var(--gold-soft) !important;
  background: rgba(232, 182, 111, 0.08) !important;
}

.source-link-button {
  color: var(--blue) !important;
}

.source-link-button:hover:not(:disabled) {
  color: var(--gold-soft) !important;
}

.library-actions,
.source-toolbar-actions,
.source-debug-controls,
.source-row-actions,
.batch-metadata-actions,
.book-import-preview-actions {
  gap: 10px;
}

.library-filter-bar,
.settings-panel,
.source-library,
.source-editor,
.source-result,
.source-metadata-panel,
.source-audit-panel,
.source-debug,
.image-import-preview,
.image-sequence-preview,
.batch-metadata-panel,
.duplicate-books-panel,
.book-metadata-editor,
.image-relink-panel {
  border-color: rgba(211, 224, 241, 0.13) !important;
  border-radius: 18px !important;
  background: linear-gradient(145deg, rgba(17, 30, 48, 0.86), rgba(10, 20, 34, 0.78)) !important;
  box-shadow: 0 18px 45px rgba(1, 8, 18, 0.22);
}

.library-filter-bar {
  align-items: end;
}

.library-filter-field,
.source-meta-field,
.search-page-limit,
.reader-controls label {
  color: var(--muted-strong) !important;
}

.library-search-input,
.source-url-input,
.source-meta-field input:not([type="checkbox"]),
.source-editor textarea,
.search-page-limit select,
.library-filter-field select,
.library-filter-field input,
.settings-field select,
.settings-field input[type="color"] {
  border-color: rgba(211, 224, 241, 0.17) !important;
  border-radius: 10px !important;
  color: var(--text) !important;
  background: rgba(6, 15, 27, 0.84) !important;
}

.library-search-input:focus,
.source-url-input:focus,
.source-meta-field input:focus,
.source-editor textarea:focus,
.search-page-limit select:focus,
.library-filter-field select:focus,
.library-filter-field input:focus,
.settings-field select:focus {
  border-color: rgba(232, 182, 111, 0.68) !important;
  box-shadow: 0 0 0 3px rgba(232, 182, 111, 0.12) !important;
  outline: none !important;
}

.source-editor textarea {
  min-height: 270px;
  line-height: 1.65;
}

.source-hint,
.source-inline-error,
.source-inline-success,
.source-list-empty,
.search-results-note,
.duplicate-books-note,
.image-relink-note,
.settings-note {
  color: var(--muted) !important;
}

.source-row,
.search-result-row,
.search-failure-row,
.book-import-preview,
.duplicate-book-group,
.duplicate-preview-book,
.recent-book,
.chapter-item {
  border-color: rgba(211, 224, 241, 0.12) !important;
  background: rgba(8, 18, 31, 0.62) !important;
}

.source-row:hover,
.search-result-row:hover,
.book-card:hover,
.recent-book:hover,
.chapter-item:hover {
  border-color: rgba(232, 182, 111, 0.48) !important;
  background: rgba(232, 182, 111, 0.07) !important;
}

.source-row.selected,
.search-result-row.selected,
.chapter-item.selected {
  border-color: rgba(232, 182, 111, 0.62) !important;
  background: linear-gradient(110deg, rgba(232, 182, 111, 0.14), rgba(134, 223, 194, 0.06)) !important;
  box-shadow: inset 3px 0 0 var(--gold);
}

.book-card {
  border-color: rgba(211, 224, 241, 0.13) !important;
  border-radius: 18px !important;
  background: linear-gradient(150deg, rgba(19, 34, 53, 0.9), rgba(9, 20, 34, 0.84)) !important;
  box-shadow: 0 16px 32px rgba(1, 8, 18, 0.18);
}

.book-card:hover,
.book-card:focus-visible {
  border-color: rgba(232, 182, 111, 0.52) !important;
  box-shadow: 0 22px 38px rgba(1, 8, 18, 0.28);
}

.book-card.selected {
  border-color: rgba(134, 223, 194, 0.58) !important;
  box-shadow: 0 0 0 1px rgba(134, 223, 194, 0.2), 0 18px 36px rgba(1, 8, 18, 0.24) !important;
}

.book-cover,
.recent-book-cover {
  border-color: rgba(232, 182, 111, 0.28) !important;
  background:
    linear-gradient(145deg, rgba(232, 182, 111, 0.2), rgba(134, 223, 194, 0.08)),
    #172a40 !important;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.04);
}

.book-cover-status,
.book-format,
.book-metadata-chip,
.book-health-badge,
.source-search-badge {
  border-color: rgba(211, 224, 241, 0.14) !important;
}

.progress-track {
  background: rgba(211, 224, 241, 0.12) !important;
}

.progress-track span {
  background: linear-gradient(90deg, var(--mint), var(--gold)) !important;
  box-shadow: 0 0 12px rgba(134, 223, 194, 0.34);
}

.settings-intro {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 14px;
  margin-top: 20px;
  padding: 16px 20px;
  border: 1px solid rgba(232, 182, 111, 0.2);
  border-radius: 16px;
  background: linear-gradient(100deg, rgba(232, 182, 111, 0.1), rgba(134, 223, 194, 0.055));
  box-shadow: 0 14px 32px rgba(1, 8, 18, 0.16);
}

.settings-intro strong {
  color: var(--text);
  font-family: "Noto Serif CJK SC", "Source Han Serif SC", "Microsoft YaHei", serif;
  font-size: 15px;
  letter-spacing: -0.02em;
}

.settings-intro > span:last-child {
  color: var(--muted);
  font-size: 12px;
}

.settings-panel {
  margin-top: 16px !important;
}

.settings-section-heading h2,
.source-section-heading h2,
.batch-metadata-heading h3,
.duplicate-books-heading h3 {
  font-family: "Noto Serif CJK SC", "Source Han Serif SC", "Microsoft YaHei", serif;
  letter-spacing: -0.025em;
}

.settings-field input[type="range"] {
  accent-color: var(--gold) !important;
}

.settings-field input[type="color"] {
  min-height: 42px !important;
  border-color: rgba(232, 182, 111, 0.26) !important;
}

.reader-toolbar {
  margin-bottom: 18px !important;
  padding: 12px 14px;
  border: 1px solid rgba(211, 224, 241, 0.13);
  border-radius: 16px;
  background: linear-gradient(145deg, rgba(17, 30, 48, 0.86), rgba(10, 20, 34, 0.78));
  box-shadow: 0 18px 45px rgba(1, 8, 18, 0.22);
}

.reader-heading strong {
  color: var(--text) !important;
  font-family: "Noto Serif CJK SC", "Source Han Serif SC", "Microsoft YaHei", serif;
  letter-spacing: -0.02em;
}

.reader-heading span,
.reader-meta {
  color: var(--muted) !important;
}

.reader-meta {
  color: var(--gold-soft) !important;
  letter-spacing: 0.04em;
}

.toolbar-button {
  border-color: rgba(211, 224, 241, 0.16) !important;
  border-radius: 10px !important;
  color: var(--text-soft) !important;
  background: rgba(211, 224, 241, 0.055) !important;
}

.toolbar-button:hover:not(:disabled) {
  border-color: rgba(232, 182, 111, 0.5) !important;
  color: var(--gold-soft) !important;
  background: rgba(232, 182, 111, 0.1) !important;
}

.reader-layout {
  gap: 16px !important;
}

.chapter-panel {
  border-color: rgba(211, 224, 241, 0.13) !important;
  border-radius: 16px !important;
  background: rgba(10, 20, 34, 0.74) !important;
}

.reader-page {
  border-color: rgba(211, 224, 241, 0.12) !important;
  border-radius: 20px !important;
  box-shadow: 0 28px 80px rgba(1, 8, 18, 0.28);
}

.reader-page h2,
.reader-page h3 {
  font-family: "Noto Serif CJK SC", "Source Han Serif SC", "Microsoft YaHei", serif;
}

.reader-page a {
  color: var(--gold-soft) !important;
}

.status-banner {
  border-color: rgba(134, 223, 194, 0.16) !important;
  background: rgba(134, 223, 194, 0.055) !important;
}

.status-banner.error,
.status-banner.warning {
  border-color: rgba(242, 154, 170, 0.24) !important;
  background: rgba(242, 154, 170, 0.07) !important;
}

@media (max-width: 760px) {
  .settings-intro {
    grid-template-columns: 1fr;
    align-items: start;
    gap: 8px;
  }

  .settings-intro > span:last-child {
    line-height: 1.6;
  }
}

@media (prefers-reduced-motion: reduce) {
  .secondary-button:hover:not(:disabled),
  .import-button:hover:not(:disabled),
  .book-card:hover {
    transform: none;
  }
}

</style>
