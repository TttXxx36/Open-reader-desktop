<script setup lang="ts">
import { inject } from "vue";

const context = inject<any>("open-reader-context");
if (!context) throw new Error("Open Reader context is not available.");
const { SETTINGS_KEY, SETTINGS_VERSION, DEFAULT_READER_SETTINGS, readerFontStacks, view, books, recentBooks, continueBook, detail, chapter, fileInput, sourceImportInput, status, errorMessage, isImporting, settings, sourceBusy, sourceValidation, sources, sourceId, sourceListBusy, sourcePipelineBusy, sourceKeyword, sourcePipeline, searchKeyword, searchBusy, searchResult, sourceTransferBusy, sourceTransferMessage, sourceImportUrl, sourceImportPreview, sourceImportPayload, sourceImportLabel, sourceAuditBusy, sourceAudit, sourceCacheBusy, sourceCacheStatus, remoteBusy, remoteBook, remoteChapter, remoteChapterRef, sourceJson, chapterParagraphs, chapterBlocks, remoteChapterParagraphs, readerStyle, themeLabels, parseContentBlocks, contentBlockTag, clampNumber, normalizeHex, isRecord, loadSettings, loadBooks, openSources, openSettings, closeSettings, resetSettings, loadSources, runSourceAudit, refreshSourceCacheStatus, formatBytes, selectSource, newSourceDraft, saveSource, toggleSource, deleteSource, searchSources, clearSearch, finishSourceImport, exportSources, openSourceImportPicker, showSourceImportPreview, clearSourceImportPreview, confirmSourceImport, importSourceUrl, importSourceFile, openRemoteBook, loadRemoteChapter, refreshRemoteBook, remoteChapterIndex, goToRemoteChapter, previousRemoteChapter, nextRemoteChapter, runSourcePipeline, validateSource, openFilePicker, importFile, openBook, loadChapter, saveProgress, continueReading, closeReader, cycleTheme, formatProgress, currentChapterIndex, goToChapter, previousChapter, nextChapter } = context;
</script>

<template>
<section
      v-else-if="detail && chapter"
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
