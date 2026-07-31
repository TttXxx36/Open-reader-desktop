<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

type View = "library" | "reader";
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

const SETTINGS_KEY = "open-reader.settings";
const view = ref<View>("library");
const books = ref<BookSummary[]>([]);
const detail = ref<BookDetail | null>(null);
const chapter = ref<ChapterContent | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
const status = ref("正在加载书架…");
const errorMessage = ref("");
const isImporting = ref(false);
const settings = ref<ReaderSettings>(loadSettings());

const chapterParagraphs = computed(() =>
  chapter.value?.content.split(/\n{2,}/).filter(Boolean) ?? [],
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
        <button class="nav-item" type="button" disabled>书源 <span>M3</span></button>
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

      <header class="topbar">
        <div>
          <span class="eyebrow">YOUR LIBRARY</span>
          <h1>书架</h1>
        </div>
        <button class="import-button" type="button" :disabled="isImporting" @click="openFilePicker">
          {{ isImporting ? "解析中…" : "导入 TXT / EPUB" }}
        </button>
      </header>

      <div class="status-banner" role="status">
        <span class="status-dot"></span>
        <span>{{ status }}</span>
        <span v-if="errorMessage" class="error-text">{{ errorMessage }}</span>
      </div>

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
