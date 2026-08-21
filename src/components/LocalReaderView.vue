<script setup lang="ts">
import { inject, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

const context = inject<any>("open-reader-context");
  if (!context) throw new Error("Open Reader context is not available.");
const { SETTINGS_KEY, SETTINGS_VERSION, DEFAULT_READER_SETTINGS, readerFontStacks, view, books, recentBooks, continueBook, detail, chapter, fileInput, sourceImportInput, status, errorMessage, isImporting, settings, sourceBusy, sourceValidation, sources, sourceId, sourceListBusy, sourcePipelineBusy, sourceKeyword, sourcePipeline, searchKeyword, searchBusy, searchResult, sourceTransferBusy, sourceTransferMessage, sourceImportUrl, sourceImportPreview, sourceImportPayload, sourceImportLabel, sourceAuditBusy, sourceAudit, sourceCacheBusy, sourceCacheStatus, remoteBusy, remoteBook, remoteChapter, remoteChapterRef, sourceJson, chapterParagraphs, chapterBlocks, chapterLinks, scrollToFragment, openContentLink, remoteChapterParagraphs, readerStyle, themeLabels, parseContentBlocks, contentBlockTag, clampNumber, normalizeHex, isRecord, loadSettings, loadBooks, openSources, openSettings, closeSettings, resetSettings, loadSources, runSourceAudit, refreshSourceCacheStatus, formatBytes, selectSource, newSourceDraft, saveSource, toggleSource, deleteSource, searchSources, clearSearch, finishSourceImport, exportSources, openSourceImportPicker, showSourceImportPreview, clearSourceImportPreview, confirmSourceImport, importSourceUrl, importSourceFile, openRemoteBook, loadRemoteChapter, refreshRemoteBook, remoteChapterIndex, goToRemoteChapter, previousRemoteChapter, nextRemoteChapter, runSourcePipeline, validateSource, openFilePicker, importFile, openBook, loadChapter, saveProgress, continueReading, closeReader, cycleTheme, formatProgress, currentChapterIndex, goToChapter, previousChapter, nextChapter } = context;

const readerPage = ref<HTMLElement | null>(null);
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let restoringPosition = false;

function scrollContainer() {
  return settings.value.readingMode === "paged" ? readerPage.value : null;
}

function currentReadingPosition() {
  const container = scrollContainer();
  return container ? container.scrollTop : window.scrollY;
}

function currentReadingProgress() {
  if (!detail.value || !chapter.value) return 0;
  const container = scrollContainer();
  const maxScroll = container
    ? Math.max(container.scrollHeight - container.clientHeight, 0)
    : Math.max(document.documentElement.scrollHeight - window.innerHeight, 0);
  const offset = maxScroll > 0 ? currentReadingPosition() / maxScroll : 0;
  const index = Math.max(currentChapterIndex(), 0);
  const lastIndex = Math.max(detail.value.chapters.length - 1, 1);
  return Math.min(1, Math.max(0, (index + Math.min(1, Math.max(0, offset))) / lastIndex));
}

function scheduleProgressSave() {
  if (restoringPosition || !detail.value || !chapter.value) return;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    if (!detail.value || !chapter.value) return;
    const progress = currentReadingProgress();
    const readState = progress >= 0.999 ? "finished" : progress > 0 ? "reading" : "unread";
    void saveProgress(
      currentChapterIndex(),
      chapter.value.id,
      currentReadingPosition(),
      progress,
      readState,
    );
  }, 350);
}

function restoreReadingPosition() {
  if (!detail.value || !chapter.value) return;
  const position = Math.max(0, Number(detail.value.reading_state?.position ?? 0));
  restoringPosition = true;
  const container = scrollContainer();
  if (container) {
    container.scrollTop = position;
  } else {
    window.scrollTo({ top: position, behavior: "auto" });
  }
  window.setTimeout(() => {
    restoringPosition = false;
  }, 0);
}

function toggleReadState() {
  if (!detail.value || !chapter.value) return;
  const nextState = detail.value.reading_state?.read_state === "finished" ? "unread" : "finished";
  void saveProgress(
    currentChapterIndex(),
    chapter.value.id,
    currentReadingPosition(),
    nextState === "finished" ? 1 : Math.min(detail.value.book.progress, 0.999),
    nextState,
  );
}

onMounted(() => {
  window.addEventListener("scroll", scheduleProgressSave, { passive: true });
  void nextTick(restoreReadingPosition);
});
onBeforeUnmount(() => {
  window.removeEventListener("scroll", scheduleProgressSave);
  if (saveTimer) clearTimeout(saveTimer);
});
watch([detail, chapter, () => settings.value.readingMode], () => {
  void nextTick(restoreReadingPosition);
}, { flush: "post" });
</script>

<template>
<section
      v-if="detail && chapter"
      class="content reader-content"
      :class="[`theme-${settings.theme}`, `reading-${settings.readingMode}`]"
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

        <article ref="readerPage" class="reader-page" @scroll.passive="scheduleProgressSave">
          <div class="reader-meta">{{ chapter.index + 1 }} / {{ chapter.total }} · {{ detail.book.format.toUpperCase() }}</div>
          <details v-if="chapterLinks.length" class="reader-link-index">
            <summary>本章内部链接 · {{ chapterLinks.length }}</summary>
            <ul>
              <li v-for="link in chapterLinks" :key="link.href + link.label">
                <a
                  v-if="typeof link.targetChapter === 'number'"
                  href="#"
                  @click.prevent="openContentLink(link.targetChapter, link.href)"
                >{{ link.label }}</a>
                <a
                  v-else-if="link.href.startsWith('#')"
                  href="#"
                  @click.prevent="scrollToFragment(link.href)"
                >{{ link.label }}</a>
                <span v-else>{{ link.label }}</span>
                <code>{{ link.href }}</code>
              </li>
            </ul>
          </details>
          <h2>{{ chapter.title }}</h2>
          <template v-if="chapterBlocks.length">
            <template v-for="(block, index) in chapterBlocks" :key="index">
              <div
                v-if="block.kind === 'image'"
                class="reader-rich-image"
                :id="block.anchor || undefined"
                :style="block.style || undefined"
                role="img"
                :aria-label="block.alt || '图片'"
              >
                <img
                  v-if="block.src"
                  :src="block.src"
                  :alt="block.alt || 'EPUB 图片'"
                />
                <span v-else>{{ block.alt ? '[图片：' + block.alt + ']' : '[图片]' }}</span>
              </div>
              <component
                v-else
                :is="contentBlockTag(block)"
                :id="block.anchor || undefined"
                :style="block.style || undefined"
                class="reader-rich-block"
                :class="'reader-rich-' + block.kind"
              >
                <template v-for="(span, spanIndex) in block.spans" :key="spanIndex">
                  <strong v-if="span.emphasis === 'strong'">{{ span.text }}</strong>
                  <em v-else-if="span.emphasis === 'em'">{{ span.text }}</em>
                  <span v-else>{{ span.text }}</span>
                </template>

<style scoped>
.reader-link-index {
  margin: 15px 0 18px;
  padding: 10px 12px;
  border: 1px solid rgba(121, 201, 255, 0.2);
  border-radius: 9px;
  color: #aebbd0;
  background: rgba(12, 17, 27, 0.38);
  font-size: 11px;
}

.reader-link-index a {
  color: #9be7d8;
  text-decoration: none;
}

.reader-link-index a:hover,
.reader-link-index a:focus-visible {
  text-decoration: underline;
}

.reader-link-index summary {
  cursor: pointer;
  color: #b9d9ff;
  font-weight: 650;
}

.reader-link-index ul {
  display: grid;
  gap: 6px;
  margin: 10px 0 0;
  padding: 0;
  list-style: none;
}

.reader-link-index li {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 0;
  border-top: 1px solid rgba(148, 163, 184, 0.1);
}

.reader-progress-status {
  display: inline-flex;
  align-items: center;
  gap: 10px;
}

.reader-link-index code {
  overflow: hidden;
  color: #8391a6;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.theme-paper .reader-link-index {
  border-color: rgba(91, 76, 57, 0.24);
  color: #756b5e;
  background: rgba(255, 255, 255, 0.32);
}

.theme-paper .reader-link-index summary {
  color: #5d7590;
}
</style>
              </component>
            </template>
          </template>
          <template v-else>
            <p v-for="(paragraph, index) in chapterParagraphs" :key="index">{{ paragraph }}</p>
          </template>

          <footer class="chapter-navigation">
            <button class="toolbar-button" type="button" :disabled="currentChapterIndex() <= 0" @click="previousChapter">
              ← 上一章
            </button>
            <span class="reader-progress-status">
              <span>{{ formatProgress(detail.book.progress) }}</span>
              <button class="text-button" type="button" @click="toggleReadState">
                {{ detail.reading_state?.read_state === "finished" ? "标记未读" : "标记已读" }}
              </button>
            </span>
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
</template>
