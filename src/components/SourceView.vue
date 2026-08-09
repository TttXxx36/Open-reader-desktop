<script setup lang="ts">
import { inject } from "vue";

const context = inject<any>("open-reader-context");
if (!context) throw new Error("Open Reader context is not available.");
const formatDebugVariables = (variables: Record<string, string> = {}) =>
  Object.entries(variables)
    .map(([key, value]) => `${key}=${value}`)
    .join(" · ");
const { SETTINGS_KEY, SETTINGS_VERSION, DEFAULT_READER_SETTINGS, readerFontStacks, view, books, recentBooks, continueBook, detail, chapter, fileInput, sourceImportInput, status, errorMessage, isImporting, settings, sourceBusy, sourceValidation, sources, filteredSources, sourceGroupFilter, sourceGroupDraft, sourceWeightDraft, sourceOrderDraft, sourceExploreDraft, sourceCommentDraft, selectedSourceIds, sourceBatchBusy, sourceBatchGroup, allFilteredSourcesSelected, sourceId, sourceListBusy, sourcePipelineBusy, sourceKeyword, sourcePipeline, searchKeyword, searchBusy, searchResult, sourceTransferBusy, sourceTransferMessage, sourceImportUrl, sourceImportPreview, sourceImportPayload, sourceImportLabel, sourceImportStrategy, sourceSnapshots, sourceImportSnapshotId, sourceAuditBusy, sourceAudit, sourceCacheBusy, sourceCacheStatus, sourceFailureHistory, sourceFailureHistoryBusy, sourceFailureStats, sourceMetrics, sourceRuleMetrics, remoteBusy, remoteBook, remoteChapter, remoteChapterRef, sourceJson, chapterParagraphs, chapterBlocks, remoteChapterParagraphs, readerStyle, themeLabels, parseContentBlocks, contentBlockTag, clampNumber, normalizeHex, isRecord, loadSettings, loadBooks, openSources, openSettings, closeSettings, resetSettings, loadSources, runSourceAudit, refreshSourceCacheStatus, loadSourceFailureHistory, clearSourceFailureHistory, loadSourceFailureStats, loadSourceRequestMetrics, loadSourceRuleMetrics, formatBytes, formatPercent, selectSource, newSourceDraft, saveSource, saveSourceMetadata, toggleSource, toggleSourceExplore, toggleSourceSelection, toggleSelectAllSources, applySourceBatch, reorderSource, deleteSource, searchSources, clearSearch, finishSourceImport, exportSources, openSourceImportPicker, showSourceImportPreview, clearSourceImportPreview, confirmSourceImport, restoreSourceSnapshot, importSourceUrl, importSourceFile, openRemoteBook, loadRemoteChapter, refreshRemoteBook, remoteChapterIndex, goToRemoteChapter, previousRemoteChapter, nextRemoteChapter, runSourcePipeline, cancelSourcePipeline, exportSourceDiagnostics, exportSourceFailureReport, validateSource, openFilePicker, importFile, loadChapter, saveProgress, continueReading, closeReader, cycleTheme, formatProgress, currentChapterIndex, goToChapter, previousChapter, nextChapter } = context;
</script>

<template>
<section v-if="view === 'sources'" class="content source-content" id="sources">
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
      <div v-if="sourceSnapshots.length" class="source-snapshot-bar">
        <span>最近快照：{{ sourceSnapshots[0].source_count }} 个书源 · {{ sourceSnapshots[0].created_at }}</span>
        <button class="source-link-button" type="button" :disabled="sourceTransferBusy" @click="restoreSourceSnapshot()">
          恢复最近快照
        </button>
      </div>

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
            <div class="source-preview-main">
              <strong>{{ entry.index + 1 }}. {{ entry.name || "未命名书源" }}</strong>
              <small v-if="entry.valid && entry.changed_fields.length">
                变更：{{ entry.changed_fields.join("、") }}
              </small>
              <div v-if="entry.unsupported_rules.length" class="source-preview-unsupported">
                <span v-for="rule in entry.unsupported_rules" :key="rule.context + rule.value">
                  不执行 · {{ rule.context }}：{{ rule.value }}（{{ rule.reason }}）
                  <small>
                    离线结构：{{ rule.offline_accepted ? "受限语法可解析" : "静态解析拒绝" }} ·
                    {{ rule.offline_steps }} 步 · {{ rule.offline_estimated_work }} work ·
                    {{ rule.offline_elapsed_us }} μs
                  </small>
                </span>
              </div>
            </div>
            <div class="source-preview-status">
              <span v-if="entry.valid">{{ entry.action }} · {{ entry.enabled ? "启用" : "停用" }}</span>
              <span v-else>跳过 · {{ entry.error || "不兼容" }}</span>
            </div>
          </li>
        </ul>
        <div class="source-preview-actions">
          <label class="source-conflict-strategy">
            <span>冲突处理</span>
            <select v-model="sourceImportStrategy" :disabled="sourceTransferBusy">
              <option value="update">更新已有</option>
              <option value="skip-existing">跳过已有</option>
              <option value="new">全部新建</option>
            </select>
          </label>
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
            <div class="source-library-actions">
              <input v-model="sourceGroupFilter" aria-label="按分组筛选书源" placeholder="筛选分组" />
              <button class="source-link-button" type="button" @click="toggleSelectAllSources">
                {{ allFilteredSourcesSelected ? "取消全选" : "全选当前" }}
              </button>
              <button class="source-link-button" type="button" @click="newSourceDraft">新建</button>
            </div>
          </div>
          <div v-if="selectedSourceIds.length" class="source-batch-bar">
            <strong>已选 {{ selectedSourceIds.length }} 个</strong>
            <button class="source-link-button" type="button" :disabled="sourceBatchBusy" @click="applySourceBatch('enable')">启用</button>
            <button class="source-link-button" type="button" :disabled="sourceBatchBusy" @click="applySourceBatch('disable')">停用</button>
            <button class="source-link-button" type="button" :disabled="sourceBatchBusy" @click="applySourceBatch('explore-on')">开启发现</button>
            <button class="source-link-button" type="button" :disabled="sourceBatchBusy" @click="applySourceBatch('explore-off')">关闭发现</button>
            <input v-model="sourceBatchGroup" class="source-batch-group-input" :disabled="sourceBatchBusy" placeholder="目标分组" />
            <button class="source-link-button" type="button" :disabled="sourceBatchBusy || !sourceBatchGroup.trim()" @click="applySourceBatch('group')">移动分组</button>
            <button class="source-link-button danger" type="button" :disabled="sourceBatchBusy" @click="applySourceBatch('delete')">批量删除</button>
          </div>
          <p v-if="errorMessage" class="source-inline-error">{{ errorMessage }}</p>
          <p v-if="sourceListBusy" class="source-list-empty">正在读取…</p>
          <p v-else-if="!filteredSources.length" class="source-list-empty">没有匹配的书源。</p>
          <div v-else class="source-list">
            <article
              v-for="source in filteredSources"
              :key="source.id"
              class="source-row"
              :class="{ selected: source.id === sourceId, checked: selectedSourceIds.includes(source.id) }"
              @click="selectSource(source)"
            >
              <div class="source-row-heading">
                <label class="source-select-control" @click.stop>
                  <input
                    type="checkbox"
                    :checked="selectedSourceIds.includes(source.id)"
                    :aria-label="`选择书源 ${source.name}`"
                    @change="toggleSourceSelection(source.id)"
                  />
                </label>
                <strong>{{ source.name }}</strong>
                <span :class="{ enabled: source.enabled }">{{ source.enabled ? "启用" : "停用" }}</span>
              </div>
              <div class="source-row-meta">
                <span>{{ source.group_name || "未分组" }}</span>
                <span>{{ source.source_type === 0 ? "文本" : "未支持类型" }}</span>
                <span>权重 {{ source.weight }}</span>
                <span>{{ source.enabled_explore ? "允许发现" : "仅搜索" }}</span>
              </div>
              <div class="source-row-actions">
                <button class="source-link-button" type="button" @click.stop="toggleSource(source)">
                  {{ source.enabled ? "停用" : "启用" }}
                </button>
                <button class="source-link-button" type="button" @click.stop="toggleSourceExplore(source)">
                  {{ source.enabled_explore ? "停用发现" : "启用发现" }}
                </button>
                <button class="source-link-button" type="button" :disabled="sourceBatchBusy" @click.stop="reorderSource(source, 'up')">上移</button>
                <button class="source-link-button" type="button" :disabled="sourceBatchBusy" @click.stop="reorderSource(source, 'down')">下移</button>
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

          <section class="source-metadata-panel" aria-label="书源元数据">
            <div class="source-section-heading">
              <div>
                <span class="eyebrow">SOURCE METADATA</span>
                <h2>书源管理</h2>
              </div>
              <span class="source-limit">保存后写入 SQLite</span>
            </div>
            <div class="source-meta-grid">
              <label class="source-meta-field">
                <span>分组</span>
                <input v-model="sourceGroupDraft" :disabled="!sourceId" placeholder="例如：公开测试" />
              </label>
              <label class="source-meta-field">
                <span>权重</span>
                <input v-model="sourceWeightDraft" :disabled="!sourceId" type="number" step="1" />
              </label>
              <label class="source-meta-field">
                <span>自定义顺序</span>
                <input v-model="sourceOrderDraft" :disabled="!sourceId" type="number" step="1" />
              </label>
              <label class="source-meta-field source-meta-checkbox">
                <input v-model="sourceExploreDraft" :disabled="!sourceId" type="checkbox" />
                <span>允许进入发现页</span>
              </label>
              <label class="source-meta-field source-meta-wide">
                <span>备注</span>
                <input v-model="sourceCommentDraft" :disabled="!sourceId" placeholder="记录授权范围或维护说明" />
              </label>
            </div>
            <div class="source-meta-actions">
              <button class="secondary-button" type="button" :disabled="sourceBusy || !sourceId" @click="saveSourceMetadata">
                {{ sourceBusy ? "保存中…" : "保存元数据" }}
              </button>
              <span>{{ sourceId ? "分组、排序和发现开关会影响下次列表读取。" : "先保存书源配置，再编辑元数据。" }}</span>
            </div>
          </section>
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

      <section v-if="sourceAudit || sourceCacheStatus || sourceFailureHistory || sourceFailureHistoryBusy" class="source-audit-panel" aria-live="polite">
        <div class="source-section-heading">
          <div>
            <span class="eyebrow">SECURITY & CACHE</span>
            <h2>安全与缓存状态</h2>
          </div>
          <span class="source-limit">只显示统计与审计摘要，不展示缓存正文</span>
        </div>
        <div class="source-audit-grid">
          <article class="source-audit-card">
            <span class="eyebrow">OBSERVABILITY</span>
            <strong>{{ sourceMetrics.enabled_sources }} / {{ sourceMetrics.total_sources }} 个书源已启用</strong>
            <p>
              失败记录 {{ sourceMetrics.failure_events }} 条 · 缓存 {{ sourceMetrics.cache_entries }} 条
              （{{ formatBytes(sourceMetrics.cache_bytes) }}）
            </p>
            <p v-if="sourceMetrics.audited_sources">
              审计：通过 {{ sourceMetrics.audit_pass }} · 需关注 {{ sourceMetrics.audit_attention }}
            </p>
            <p v-else>尚未运行书源安全审计。</p>
            <template v-if="sourceMetrics.request_metrics">
              <p>
                网络请求 {{ sourceMetrics.request_metrics.total_attempts }} 次 ·
                成功 {{ sourceMetrics.request_metrics.total_successes }} ·
                失败 {{ sourceMetrics.request_metrics.total_failures }}
              </p>
              <p>
                失败率 {{ formatPercent(sourceMetrics.request_metrics.failure_rate) }} ·
                缓存命中 {{ sourceMetrics.request_metrics.total_cache_hits }} 次
                （{{ formatPercent(sourceMetrics.request_metrics.cache_hit_rate) }}）
              </p>
              <div class="source-failure-stats">
                <small v-for="item in sourceMetrics.request_metrics.by_stage" :key="'metric-' + item.stage">
                  {{ item.stage }}：{{ item.attempts }} 次 / 失败 {{ formatPercent(item.failure_rate) }}
                </small>
              </div>
            </template>
            <template v-if="sourceMetrics.rule_metrics">
              <p>
                规则评估 {{ sourceMetrics.rule_metrics.total_attempts }} 次 ·
                成功 {{ sourceMetrics.rule_metrics.total_successes }} ·
                无匹配 {{ sourceMetrics.rule_metrics.total_no_matches }} ·
                失败 {{ sourceMetrics.rule_metrics.total_failures }}
              </p>
              <p>
                规则产出成功率 {{ formatPercent(sourceMetrics.rule_metrics.success_rate) }} ·
                规则错误率 {{ formatPercent(sourceMetrics.rule_metrics.failure_rate) }}
              </p>
              <div class="source-failure-stats">
                <small v-for="item in sourceMetrics.rule_metrics.by_rule" :key="'rule-metric-' + item.stage + '-' + item.rule_key">
                  {{ item.stage }}.{{ item.rule_key }}：{{ item.attempts }} 次 ·
                  成功 {{ formatPercent(item.success_rate) }} ·
                  无匹配 {{ item.no_matches }} · 错误 {{ item.failures }}
                </small>
              </div>
            </template>
          </article>
          <article v-if="sourceCacheStatus" class="source-audit-card">
            <span class="eyebrow">CACHE OBSERVABILITY</span>
            <strong>{{ sourceCacheStatus.entries }} / {{ sourceCacheStatus.max_entries }} 条</strong>
            <p>{{ formatBytes(sourceCacheStatus.bytes) }} / {{ formatBytes(sourceCacheStatus.max_bytes) }} · 过期 {{ sourceCacheStatus.expired_entries }} 条</p>
            <button class="source-link-button" type="button" :disabled="sourceCacheBusy" @click="refreshSourceCacheStatus">
              {{ sourceCacheBusy ? "刷新中…" : "刷新" }}
            </button>
          </article>
          <article v-if="sourceFailureHistory" class="source-audit-card source-audit-list">
            <span class="eyebrow">FAILURE HISTORY</span>
            <p>仅保存本机最近 64 条脱敏失败摘要，不上传关键词、正文或请求头。</p>
            <div v-if="sourceFailureStats" class="source-failure-stats">
              <small>累计 {{ sourceFailureStats.total }} 条</small>
              <small v-for="item in sourceFailureStats.by_reason" :key="'reason-' + item.code">原因 {{ item.code }}：{{ item.count }}</small>
              <small v-for="item in sourceFailureStats.by_stage" :key="'stage-' + item.code">阶段 {{ item.code }}：{{ item.count }}</small>
            </div>
            <p v-if="!sourceFailureHistory.length">没有失败历史。</p>
            <div v-for="failure in sourceFailureHistory" :key="failure.id" class="source-audit-row">
              <div>
                <strong>{{ failure.source_name }}</strong>
                <span>{{ failure.reason_code }}</span>
              </div>
              <p>
                {{ failure.stage }} · {{ failure.created_at }}
                <span v-if="failure.operation_id"> · 任务 {{ failure.operation_id }}</span>
              </p>
              <p>{{ failure.message }}</p>
            </div>
            <div class="source-meta-actions">
              <button class="source-link-button" type="button" :disabled="sourceFailureHistoryBusy" @click="loadSourceFailureHistory(); loadSourceFailureStats()">
                {{ sourceFailureHistoryBusy ? "刷新中…" : "刷新" }}
              </button>
              <button class="source-link-button" type="button" :disabled="sourceFailureHistoryBusy || !sourceFailureHistory.length" @click="exportSourceFailureReport">
                导出报告
              </button>
              <button class="source-link-button danger" type="button" :disabled="sourceFailureHistoryBusy || !sourceFailureHistory.length" @click="clearSourceFailureHistory">
                清空
              </button>
            </div>
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
            <button
              class="secondary-button"
              type="button"
              :disabled="sourcePipelineBusy || (!sourcePipeline && !searchResult)"
              @click="exportSourceDiagnostics"
            >
              导出诊断
            </button>
            <button
              v-if="sourcePipelineBusy"
              class="secondary-button"
              type="button"
              @click="cancelSourcePipeline"
            >
              取消调试
            </button>
          </div>
        </div>
        <p v-if="errorMessage" class="source-inline-error">{{ errorMessage }}</p>
        <p v-if="!sourcePipeline && !sourcePipelineBusy" class="source-debug-empty">
          运行后会显示请求阶段、响应状态、耗时和脱敏 URL；可导出不含正文与请求头的诊断快照。
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
              <small v-if="step.variables && Object.keys(step.variables).length">
                变量：{{ formatDebugVariables(step.variables) }}
              </small>
              <p v-if="step.error">{{ step.error }}</p>
            </li>
          </ol>
        </template>
      </section>
    </section>
</template>
