<script setup lang="ts">
import { inject } from "vue";

const context = inject<any>("open-reader-context");
if (!context) throw new Error("Open Reader context is not available.");
const { SETTINGS_KEY, SETTINGS_VERSION, DEFAULT_READER_SETTINGS, readerFontStacks, view, books, recentBooks, continueBook, detail, chapter, fileInput, sourceImportInput, status, errorMessage, isImporting, settings, sourceBusy, sourceValidation, sources, sourceId, sourceListBusy, sourcePipelineBusy, sourceKeyword, sourcePipeline, searchKeyword, searchBusy, searchResult, sourceTransferBusy, sourceTransferMessage, sourceImportUrl, sourceImportPreview, sourceImportPayload, sourceImportLabel, sourceAuditBusy, sourceAudit, sourceCacheBusy, sourceCacheStatus, remoteBusy, remoteBook, remoteChapter, remoteChapterRef, sourceJson, chapterParagraphs, chapterBlocks, chapterLinks, remoteChapterParagraphs, readerStyle, themeLabels, parseContentBlocks, contentBlockTag, clampNumber, normalizeHex, isRecord, loadSettings, loadBooks, openSources, openSettings, closeSettings, resetSettings, loadSources, runSourceAudit, refreshSourceCacheStatus, formatBytes, selectSource, newSourceDraft, saveSource, toggleSource, deleteSource, searchSources, clearSearch, finishSourceImport, exportSources, openSourceImportPicker, showSourceImportPreview, clearSourceImportPreview, confirmSourceImport, importSourceUrl, importSourceFile, openRemoteBook, loadRemoteChapter, refreshRemoteBook, remoteChapterIndex, goToRemoteChapter, previousRemoteChapter, nextRemoteChapter, runSourcePipeline, validateSource, openFilePicker, importFile, openBook, loadChapter, saveProgress, continueReading, closeReader, cycleTheme, formatProgress, currentChapterIndex, goToChapter, previousChapter, nextChapter } = context;
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

        <article class="reader-page">
          <div class="reader-meta">{{ chapter.index + 1 }} / {{ chapter.total }} · {{ detail.book.format.toUpperCase() }}</div>
          <details v-if="chapterLinks.length" class="reader-link-index">
            <summary>本章内部链接 · {{ chapterLinks.length }}</summary>
            <ul>
              <li v-for="link in chapterLinks" :key="link.href + link.label">
                <span>{{ link.label }}</span>
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
</template>
