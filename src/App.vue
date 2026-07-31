<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

type View = "library" | "reader" | "sources";
type ReaderTheme = "night" | "paper" | "sepia";

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
  index: number;
  total: number;
}

interface ReaderSettings {
  fontSize: number;
  lineHeight: number;
  theme: ReaderTheme;
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
const view = ref<View>("library");
const books = ref<BookSummary[]>([]);
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
const sourceId = ref<string | null>(null);
const sourceListBusy = ref(false);
const sourcePipelineBusy = ref(false);
const sourceKeyword = ref("demo");
const sourcePipeline = ref<SourcePipelineResult | null>(null);
const searchKeyword = ref("");
const searchBusy = ref(false);
const searchResult = ref<MultiSourceSearchResult | null>(null);
const sourceTransferBusy = ref(false);
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
const remoteChapterParagraphs = computed(() =>
  remoteChapter.value?.content.split(/\n{2,}/).filter(Boolean) ?? [],
);
const readerStyle = computed(() => ({
  "--reader-font-size": `${settings.value.fontSize}px`,
  "--reader-line-height": settings.value.lineHeight,
}));
const themeLabels: Record<ReaderTheme, string> = {
  night: "夜间",
  paper: "纸张",
  sepia: "暖色",
};

onMounted(loadBooks);
watch(settings, (value) => {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(value));
}, { deep: true });

function loadSettings(): ReaderSettings {
  try {
    const saved = JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "");
    return {
      fontSize: Number(saved.fontSize) || 19,
      lineHeight: Number(saved.lineHeight) || 1.9,
      theme: saved.theme === "paper" || saved.theme === "sepia" ? saved.theme : "night",
    };
  } catch {
    return { fontSize: 19, lineHeight: 1.9, theme: "night" };
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
  await loadSources();
}

async function loadSources() {
  sourceListBusy.value = true;
  try {
    sources.value = await invoke<SourceSummary[]>("list_sources");
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceListBusy.value = false;
  }
}

function selectSource(source: SourceSummary) {
  sourceId.value = source.id;
  sourceJson.value = source.config_json;
  sourceValidation.value = null;
  sourcePipeline.value = null;
  errorMessage.value = "";
}

function newSourceDraft() {
  sourceId.value = null;
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
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    sourceTransferBusy.value = false;
  }
}

function openSourceImportPicker() {
  sourceImportInput.value?.click();
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
  errorMessage.value = "";
  try {
    const imported = await invoke<SourceSummary[]>("import_sources", {
      bundleJson: await file.text(),
    });
    await loadSources();
    if (imported[0]) {
      selectSource(imported[0]);
    }
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
</script>

<template>
  <main class="shell">
    <aside class="sidebar">
      <div class="brand">
        <span class="brand-mark">O</span>
        <div>
          <strong>Open Reader</strong>
          <span>Desktop · M2</span>
        </div>
      </div>

      <nav class="nav" aria-label="主导航">
        <button class="nav-item active" type="button" @click="closeReader">书架 <span>⌘1</span></button>
        <button class="nav-item" type="button" :class="{ active: view === 'sources' }" @click="openSources">书源 <span>M3</span></button>
        <button class="nav-item" type="button" disabled>设置 <span>M6</span></button>
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
      <input
        ref="sourceImportInput"
        class="file-input"
        type="file"
        accept=".json,application/json"
        @change="importSourceFile"
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


    <section v-else-if="view === 'sources'" class="content source-content" id="sources">
      <header class="topbar">
        <div>
          <span class="eyebrow">SOURCE PROTOCOL</span>
          <h1>书源</h1>
        </div>
        <div class="source-toolbar-actions">
          <button class="secondary-button" type="button" :disabled="sourceTransferBusy" @click="openSourceImportPicker">
            {{ sourceTransferBusy ? "处理中…" : "导入 JSON" }}
          </button>
          <button class="secondary-button" type="button" :disabled="sourceTransferBusy" @click="exportSources">
            {{ sourceTransferBusy ? "处理中…" : "导出 JSON" }}
          </button>
          <button class="secondary-button" type="button" :disabled="sourceBusy || sourceTransferBusy" @click="saveSource">
            {{ sourceBusy ? "保存中…" : "保存书源" }}
          </button>
          <button class="import-button" type="button" :disabled="sourceBusy || sourceTransferBusy" @click="validateSource">
            {{ sourceBusy ? "校验中…" : "校验 JSON" }}
          </button>
        </div>
      </header>

      <div class="source-grid">
        <section class="source-library">
          <div class="source-section-heading">
            <div>
              <span class="eyebrow">SOURCE LIBRARY</span>
              <h2>已保存书源</h2>
            </div>
            <button class="source-link-button" type="button" @click="newSourceDraft">新建</button>
          </div>
          <p v-if="errorMessage" class="source-inline-error">{{ errorMessage }}</p>
          <p v-if="sourceListBusy" class="source-list-empty">正在读取…</p>
          <p v-else-if="!sources.length" class="source-list-empty">还没有保存书源。</p>
          <div v-else class="source-list">
            <article
              v-for="source in sources"
              :key="source.id"
              class="source-row"
              :class="{ selected: source.id === sourceId }"
              @click="selectSource(source)"
            >
              <div class="source-row-heading">
                <strong>{{ source.name }}</strong>
                <span :class="{ enabled: source.enabled }">{{ source.enabled ? "启用" : "停用" }}</span>
              </div>
              <div class="source-row-actions">
                <button class="source-link-button" type="button" @click.stop="toggleSource(source)">
                  {{ source.enabled ? "停用" : "启用" }}
                </button>
                <button class="source-link-button danger" type="button" @click.stop="deleteSource(source)">删除</button>
              </div>
            </article>
          </div>
        </section>

        <section class="source-editor">
          <div class="source-section-heading">
            <div>
              <span class="eyebrow">LEGADO-INSPIRED</span>
              <h2>粘贴书源配置</h2>
            </div>
            <span class="source-limit">仅校验结构，不访问真实站点</span>
          </div>
          <textarea v-model="sourceJson" spellcheck="false" aria-label="书源 JSON"></textarea>
          <p class="source-hint">支持 searchUrl、bookInfoUrl、tocUrl、contentUrl，以及 search / bookInfo / toc / content 规则别名。</p>
        </section>

        <section class="source-result" aria-live="polite">
          <span class="eyebrow">VALIDATION</span>
          <div v-if="!sourceValidation" class="source-result-empty">
            <div class="empty-icon">✓</div>
            <h3>等待校验</h3>
            <p>先检查 URL、CSS 选择器和正则表达式，再保存配置或运行端到端调试。</p>
          </div>
          <template v-else>
            <div class="validation-state" :class="{ valid: sourceValidation.valid }">
              <span>{{ sourceValidation.valid ? "配置可用" : "配置需要修正" }}</span>
            </div>
            <div v-if="sourceValidation.errors.length" class="validation-list errors">
              <strong>错误</strong>
              <p v-for="error in sourceValidation.errors" :key="error">{{ error }}</p>
            </div>
            <div v-if="sourceValidation.warnings.length" class="validation-list warnings">
              <strong>提示</strong>
              <p v-for="warning in sourceValidation.warnings" :key="warning">{{ warning }}</p>
            </div>
          </template>
        </section>
      </div>

      <section class="source-debug">
        <div class="source-debug-heading">
          <div>
            <span class="eyebrow">DEBUG RUN</span>
            <h2>端到端调试</h2>
          </div>
          <div class="source-debug-controls">
            <input v-model="sourceKeyword" aria-label="搜索关键词" placeholder="搜索关键词" />
            <button
              class="import-button"
              type="button"
              :disabled="sourcePipelineBusy || !sourceKeyword.trim()"
              @click="runSourcePipeline"
            >
              {{ sourcePipelineBusy ? "执行中…" : "运行测试" }}
            </button>
          </div>
        </div>
        <p v-if="errorMessage" class="source-inline-error">{{ errorMessage }}</p>
        <p v-if="!sourcePipeline && !sourcePipelineBusy" class="source-debug-empty">
          运行后会显示请求阶段、响应状态、耗时和脱敏 URL。
        </p>
        <template v-if="sourcePipeline">
          <div class="source-debug-summary">
            <strong>{{ sourcePipeline.book_info.title }}</strong>
            <span>{{ sourcePipeline.search_results.length }} 个搜索结果 · {{ sourcePipeline.chapters.length }} 个章节</span>
          </div>
          <ol class="source-debug-steps">
            <li v-for="step in sourcePipeline.debug_steps" :key="step.stage">
              <div>
                <strong>{{ step.stage }}</strong>
                <span>{{ step.status ?? "失败" }} · {{ step.duration_ms }} ms · {{ step.bytes ?? 0 }} bytes</span>
              </div>
              <code>{{ step.url }}</code>
              <p v-if="step.error">{{ step.error }}</p>
            </li>
          </ol>
        </template>
      </section>
    </section>

    <section
      v-else-if="remoteBook && remoteChapter"
      class="content reader-content"
      :class="'theme-' + settings.theme"
      :style="readerStyle"
    >
      <header class="reader-toolbar">
        <button class="toolbar-button" type="button" @click="closeReader">← 搜索</button>
        <div class="reader-heading">
          <strong>{{ remoteBook.book_info.title }}</strong>
          <span>{{ remoteBook.book_info.author || remoteBook.source_name }}</span>
        </div>
        <div class="reader-controls">
          <label>字号 <input v-model.number="settings.fontSize" type="range" min="15" max="30" step="1" /></label>
          <label>行距 <input v-model.number="settings.lineHeight" type="range" min="1.4" max="2.4" step="0.1" /></label>
          <button class="toolbar-button" type="button" :disabled="remoteBusy" @click="refreshRemoteBook">
            {{ remoteBusy ? "刷新中…" : "刷新内容" }}
          </button>
          <button class="toolbar-button" type="button" @click="cycleTheme">{{ themeLabels[settings.theme] }}</button>
        </div>
      </header>

      <div
        v-if="remoteBook.stale || remoteBook.chapter_update || remoteChapter.stale"
        class="reader-notices"
        role="status"
      >
        <p v-if="remoteBook.stale || remoteChapter.stale" class="reader-stale-note">
          刷新失败，正在显示缓存内容：{{ remoteBook.refresh_error || remoteChapter.refresh_error || "未知错误" }}
        </p>
        <p v-else-if="remoteBook.chapter_update" class="reader-update-note">
          {{ remoteBook.chapter_update.changed ? "目录已更新" : "目录未变化" }}：
          新增 {{ remoteBook.chapter_update.added }} 章，移除 {{ remoteBook.chapter_update.removed }} 章，
          保留 {{ remoteBook.chapter_update.retained }} 章。
        </p>
      </div>

      <div class="reader-layout">
        <aside class="chapter-panel">
          <span class="eyebrow">CONTENTS · {{ remoteBook.chapters.length }} · {{ remoteBook.source_name }}</span>
          <button
            v-for="chapterItem in remoteBook.chapters"
            :key="chapterItem.url"
            class="chapter-item"
            :class="{ selected: chapterItem.url === remoteChapterRef?.url }"
            type="button"
            @click="goToRemoteChapter(chapterItem)"
          >
            <span>{{ chapterItem.index + 1 }}</span>
            <strong>{{ chapterItem.title }}</strong>
          </button>
        </aside>

        <article class="reader-page">
          <div class="reader-meta">{{ remoteChapterIndex() + 1 }} / {{ remoteBook.chapters.length }} · {{ remoteBook.source_name }}</div>
          <h2>{{ remoteChapter.title }}</h2>
          <p v-for="(paragraph, index) in remoteChapterParagraphs" :key="index">{{ paragraph }}</p>

          <footer class="chapter-navigation">
            <button class="toolbar-button" type="button" :disabled="remoteChapterIndex() <= 0 || remoteBusy" @click="previousRemoteChapter">
              ← 上一章
            </button>
            <span>{{ remoteChapterIndex() + 1 }} / {{ remoteBook.chapters.length }}</span>
            <button
              class="toolbar-button"
              type="button"
              :disabled="remoteChapterIndex() >= remoteBook.chapters.length - 1 || remoteBusy"
              @click="nextRemoteChapter"
            >
              下一章 →
            </button>
          </footer>
        </article>
      </div>
    </section>

    <section
      v-else-if="detail && chapter"
      class="content reader-content"
      :class="`theme-${settings.theme}`"
      :style="readerStyle"
    >
      <header class="reader-toolbar">
        <button class="toolbar-button" type="button" @click="closeReader">← 书架</button>
        <div class="reader-heading">
          <strong>{{ detail.book.title }}</strong>
          <span>{{ chapter.title }}</span>
        </div>
        <div class="reader-controls">
          <label>字号 <input v-model.number="settings.fontSize" type="range" min="15" max="30" step="1" /></label>
          <label>行距 <input v-model.number="settings.lineHeight" type="range" min="1.4" max="2.4" step="0.1" /></label>
          <button class="toolbar-button" type="button" @click="cycleTheme">{{ themeLabels[settings.theme] }}</button>
        </div>
      </header>

      <div class="reader-layout">
        <aside class="chapter-panel">
          <span class="eyebrow">CONTENTS · {{ detail.chapters.length }}</span>
          <button
            v-for="chapterItem in detail.chapters"
            :key="chapterItem.id"
            class="chapter-item"
            :class="{ selected: chapterItem.id === chapter.id }"
            type="button"
            @click="goToChapter(chapterItem)"
          >
            <span>{{ chapterItem.index + 1 }}</span>
            <strong>{{ chapterItem.title }}</strong>
          </button>
        </aside>

        <article class="reader-page">
          <div class="reader-meta">{{ chapter.index + 1 }} / {{ chapter.total }} · {{ detail.book.format.toUpperCase() }}</div>
          <h2>{{ chapter.title }}</h2>
          <p v-for="(paragraph, index) in chapterParagraphs" :key="index">{{ paragraph }}</p>

          <footer class="chapter-navigation">
            <button class="toolbar-button" type="button" :disabled="currentChapterIndex() <= 0" @click="previousChapter">
              ← 上一章
            </button>
            <span>{{ formatProgress(detail.book.progress) }}</span>
            <button
              class="toolbar-button"
              type="button"
              :disabled="currentChapterIndex() >= detail.chapters.length - 1"
              @click="nextChapter"
            >
              下一章 →
            </button>
          </footer>
        </article>
      </div>
    </section>
  </main>
</template>

<style scoped>
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
.source-inline-error {
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

.source-row-heading strong {
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
  margin: 0 0 1.3em;
  white-space: pre-wrap;
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
</style>
