<script setup lang="ts">
import { inject } from "vue";

const context = inject<any>("open-reader-context");
if (!context) throw new Error("Open Reader context is not available.");
const { SETTINGS_KEY, SETTINGS_VERSION, DEFAULT_READER_SETTINGS, readerFontStacks, view, books, recentBooks, continueBook, detail, chapter, fileInput, sourceImportInput, status, errorMessage, isImporting, settings, sourceBusy, sourceValidation, sources, sourceId, sourceListBusy, sourcePipelineBusy, sourceKeyword, sourcePipeline, searchKeyword, searchBusy, searchResult, sourceTransferBusy, sourceTransferMessage, sourceImportUrl, sourceImportPreview, sourceImportPayload, sourceImportLabel, sourceAuditBusy, sourceAudit, sourceCacheBusy, sourceCacheStatus, remoteBusy, remoteBook, remoteChapter, remoteChapterRef, sourceJson, chapterParagraphs, chapterBlocks, remoteChapterParagraphs, readerStyle, themeLabels, parseContentBlocks, contentBlockTag, clampNumber, normalizeHex, isRecord, loadSettings, loadBooks, openSources, openSettings, closeSettings, resetSettings, loadSources, runSourceAudit, refreshSourceCacheStatus, formatBytes, selectSource, newSourceDraft, saveSource, toggleSource, deleteSource, searchSources, clearSearch, finishSourceImport, exportSources, openSourceImportPicker, showSourceImportPreview, clearSourceImportPreview, confirmSourceImport, importSourceUrl, importSourceFile, openRemoteBook, loadRemoteChapter, refreshRemoteBook, remoteChapterIndex, goToRemoteChapter, previousRemoteChapter, nextRemoteChapter, runSourcePipeline, exportSourceDiagnostics, validateSource, openFilePicker, importFile, openBook, loadChapter, saveProgress, continueReading, closeReader, cycleTheme, formatProgress, currentChapterIndex, goToChapter, previousChapter, nextChapter } = context;
</script>

<template>
<section
      v-if="remoteBook && remoteChapter"
      class="content reader-content"
      :class="['theme-' + settings.theme, 'reading-' + settings.readingMode]"
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
          <button class="toolbar-button" type="button" :disabled="remoteBusy" @click="exportSourceDiagnostics">
            导出诊断
          </button>
          <button class="toolbar-button" type="button" @click="openSettings">阅读设置</button>
          <button class="toolbar-button" type="button" @click="cycleTheme">{{ themeLabels[settings.theme] }}</button>
        </div>
      </header>

      <div
        v-if="remoteBook.stale || remoteBook.chapter_update || remoteChapter.stale || remoteBook.cache_hit || remoteChapter.cache_hit"
        class="reader-notices"
        role="status"
      >
        <p v-if="remoteBook.stale || remoteChapter.stale" class="reader-stale-note">
          刷新失败，正在显示缓存内容：{{ remoteBook.refresh_error || remoteChapter.refresh_error || "未知错误" }}
        </p>
        <p v-if="remoteBook.cache_hit || remoteChapter.cache_hit" class="reader-cache-note">
          本次内容来自本地缓存；点击“刷新内容”可重新请求网络。
        </p>
        <p v-if="!remoteBook.stale && !remoteChapter.stale && remoteBook.chapter_update" class="reader-update-note">
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
</template>
