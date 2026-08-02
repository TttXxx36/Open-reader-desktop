<script setup lang="ts">
import { inject } from "vue";

const context = inject<any>("open-reader-context");
if (!context) throw new Error("Open Reader context is not available.");
const { SETTINGS_KEY, SETTINGS_VERSION, DEFAULT_READER_SETTINGS, readerFontStacks, view, books, recentBooks, continueBook, detail, chapter, fileInput, sourceImportInput, status, errorMessage, isImporting, settings, sourceBusy, sourceValidation, sources, sourceId, sourceListBusy, sourcePipelineBusy, sourceKeyword, sourcePipeline, searchKeyword, searchBusy, searchResult, sourceTransferBusy, sourceTransferMessage, sourceImportUrl, sourceImportPreview, sourceImportPayload, sourceImportLabel, sourceAuditBusy, sourceAudit, sourceCacheBusy, sourceCacheStatus, remoteBusy, remoteBook, remoteChapter, remoteChapterRef, sourceJson, chapterParagraphs, chapterBlocks, remoteChapterParagraphs, readerStyle, themeLabels, parseContentBlocks, contentBlockTag, clampNumber, normalizeHex, isRecord, loadSettings, loadBooks, openSources, openSettings, closeSettings, resetSettings, loadSources, runSourceAudit, refreshSourceCacheStatus, formatBytes, selectSource, newSourceDraft, saveSource, toggleSource, deleteSource, searchSources, clearSearch, finishSourceImport, exportSources, openSourceImportPicker, showSourceImportPreview, clearSourceImportPreview, confirmSourceImport, importSourceUrl, importSourceFile, openRemoteBook, loadRemoteChapter, refreshRemoteBook, remoteChapterIndex, goToRemoteChapter, previousRemoteChapter, nextRemoteChapter, runSourcePipeline, validateSource, openFilePicker, importFile, openBook, loadChapter, saveProgress, continueReading, closeReader, cycleTheme, formatProgress, currentChapterIndex, goToChapter, previousChapter, nextChapter } = context;
</script>

<template>
<section v-if="view === 'sources' class="content source-content" id="sources">
      <header class="topbar">
        <div>
          <span class="eyebrow">SOURCE PROTOCOL</span>
          <h1>书源</h1>
        </div>
        <div class="source-toolbar-actions">
          <input
            v-model="sourceImportUrl"
            class="source-url-input"
            type="url"
            autocomplete="url"
            placeholder="粘贴书源 JSON URL"
            @keyup.enter="importSourceUrl"
          />
          <button
            class="secondary-button"
            type="button"
            :disabled="sourceTransferBusy || !sourceImportUrl.trim()"
            @click="importSourceUrl"
          >
            {{ sourceTransferBusy ? "处理中…" : "从 URL 导入" }}
          </button>
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
          <button class="secondary-button" type="button" :disabled="sourceAuditBusy || sourceTransferBusy" @click="runSourceAudit">
            {{ sourceAuditBusy ? "审计中…" : "安全审计" }}
          </button>
          <button class="secondary-button" type="button" :disabled="sourceCacheBusy || sourceTransferBusy" @click="refreshSourceCacheStatus">
            {{ sourceCacheBusy ? "读取中…" : "缓存状态" }}
          </button>
        </div>
      </header>

      <p v-if="sourceTransferMessage" class="source-inline-success">{{ sourceTransferMessage }}</p>

      <section v-if="sourceImportPreview" class="source-import-preview" aria-live="polite">
        <div class="source-import-preview-heading">
          <div>
            <span class="eyebrow">SOURCE IMPORT REVIEW</span>
            <h2>导入预览 · {{ sourceImportLabel }}</h2>
          </div>
          <span class="source-preview-count">
            {{ sourceImportPreview.valid_count }}/{{ sourceImportPreview.entries.length }} 可导入
          </span>
        </div>
        <p class="source-import-preview-note">
          不兼容条目会自动跳过；预览阶段不会保存配置，也不会执行书源脚本。
        </p>
        <ul class="source-preview-list">
          <li
            v-for="entry in sourceImportPreview.entries"
            :key="entry.index"
            class="source-preview-entry"
            :class="{ invalid: !entry.valid }"
          >
            <strong>{{ entry.index + 1 }}. {{ entry.name || "未命名书源" }}</strong>
            <span v-if="entry.valid">
              {{ entry.enabled ? "可导入 · 启用" : "可导入 · 停用" }}
            </span>
            <span v-else>跳过 · {{ entry.error || "不兼容" }}</span>
          </li>
        </ul>
        <div class="source-preview-actions">
          <button
            class="secondary-button"
            type="button"
            :disabled="sourceTransferBusy || sourceImportPreview.valid_count === 0"
            @click="confirmSourceImport"
          >
            {{ sourceTransferBusy ? "导入中…" : "导入通过项" }}
          </button>
          <button
            class="source-link-button"
            type="button"
            :disabled="sourceTransferBusy"
            @click="clearSourceImportPreview"
          >
            取消
          </button>
        </div>
      </section>

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

      <section v-if="sourceAudit || sourceCacheStatus" class="source-audit-panel" aria-live="polite">
        <div class="source-section-heading">
          <div>
            <span class="eyebrow">SECURITY & CACHE</span>
            <h2>安全与缓存状态</h2>
          </div>
          <span class="source-limit">只显示统计与审计摘要，不展示缓存正文</span>
        </div>
        <div class="source-audit-grid">
          <article v-if="sourceCacheStatus" class="source-audit-card">
            <span class="eyebrow">CACHE OBSERVABILITY</span>
            <strong>{{ sourceCacheStatus.entries }} / {{ sourceCacheStatus.max_entries }} 条</strong>
            <p>{{ formatBytes(sourceCacheStatus.bytes) }} / {{ formatBytes(sourceCacheStatus.max_bytes) }} · 过期 {{ sourceCacheStatus.expired_entries }} 条</p>
            <button class="source-link-button" type="button" :disabled="sourceCacheBusy" @click="refreshSourceCacheStatus">
              {{ sourceCacheBusy ? "刷新中…" : "刷新" }}
            </button>
          </article>
          <article v-if="sourceAudit" class="source-audit-card source-audit-list">
            <span class="eyebrow">SOURCE SECURITY</span>
            <p v-if="!sourceAudit.length">没有可审计书源。</p>
            <div v-for="audit in sourceAudit" :key="audit.source_id" class="source-audit-row">
              <div>
                <strong>{{ audit.source_name }}</strong>
                <span :class="{ enabled: audit.pass && !audit.warnings.length }">
                  {{ audit.pass && !audit.warnings.length ? "通过" : audit.pass ? "需关注" : "需修正" }}
                </span>
              </div>
              <p>权限：{{ audit.permission_status }} · 主机：{{ audit.hosts.join("、") || "无" }}</p>
              <p v-if="audit.errors.length" class="source-inline-error">{{ audit.errors.join("；") }}</p>
              <p v-if="audit.warnings.length" class="source-audit-warning">{{ audit.warnings.join("；") }}</p>
            </div>
          </article>
        </div>
      </section>

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
</template>
