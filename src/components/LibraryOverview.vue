<script setup lang="ts">
import { computed } from "vue";

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

const props = defineProps<{
  books: BookSummary[];
  continueBook: BookSummary | null;
  recentBooks: BookSummary[];
}>();

const emit = defineEmits<{
  continue: [book: BookSummary];
  import: [];
  sources: [];
}>();

const libraryStats = computed(() => ({
  total: props.books.length,
  reading: props.books.filter((book) => book.progress > 0 && book.progress < 1).length,
  finished: props.books.filter((book) => book.progress >= 1).length,
}));

function formatProgress(progress: number) {
  return `${Math.round(progress * 100)}%`;
}

function coverStateLabel(book: BookSummary) {
  if (!book.cover_state || book.cover_state === "ready") return "";
  if (book.cover_state === "stale") return "封面待刷新";
  if (book.cover_state === "blocked") return "封面已阻止";
  return "使用占位图";
}

function imageHealthLabel(book: BookSummary) {
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

function imageHealthClass(book: BookSummary) {
  if (book.image_sequence_state === "ready") return "ready";
  if (book.image_sequence_state === "stale") return "stale";
  return "missing";
}

function continueWith(book: BookSummary | null) {
  if (book) emit("continue", book);
}
</script>

<template>
  <section class="library-overview" aria-label="书架概览">
    <div class="library-overview-heading">
      <div>
        <span class="eyebrow">READING DASHBOARD</span>
        <h2>今天继续读</h2>
      </div>
      <span class="library-overview-caption">本地数据 · 无需账号</span>
    </div>

    <div class="library-overview-grid">
      <article
        v-if="continueBook"
        class="continue-card"
        tabindex="0"
        @click="continueWith(continueBook)"
        @keydown.enter="continueWith(continueBook)"
      >
        <div>
          <span class="eyebrow">CONTINUE READING</span>
          <h3>{{ continueBook.title }}</h3>
          <p>
            {{ continueBook.author || "本地导入" }} · 已读 {{ formatProgress(continueBook.progress) }}
            <span
              v-if="imageHealthLabel(continueBook)"
              class="book-health"
              :class="imageHealthClass(continueBook)"
            >{{ imageHealthLabel(continueBook) }}</span>
          </p>
        </div>
        <button class="continue-card-action" type="button" @click.stop="continueWith(continueBook)">
          继续阅读 →
        </button>
      </article>

      <article v-else class="continue-card continue-card-empty">
        <div>
          <span class="eyebrow">START READING</span>
          <h3>从第一本书开始</h3>
          <p>导入 TXT / EPUB，阅读进度会自动保存。</p>
        </div>
        <div class="continue-card-actions">
          <button class="continue-card-action" type="button" @click="emit('import')">导入书籍</button>
          <button class="continue-card-link" type="button" @click="emit('sources')">浏览书源</button>
        </div>
      </article>

      <div class="library-stat-grid" aria-label="书架统计">
        <article class="library-stat-card">
          <span class="eyebrow">BOOKS</span>
          <strong>{{ libraryStats.total }}</strong>
          <span>本地书籍</span>
        </article>
        <article class="library-stat-card">
          <span class="eyebrow">READING</span>
          <strong>{{ libraryStats.reading }}</strong>
          <span>正在阅读</span>
        </article>
        <article class="library-stat-card">
          <span class="eyebrow">FINISHED</span>
          <strong>{{ libraryStats.finished }}</strong>
          <span>已读完</span>
        </article>
      </div>
    </div>

    <div v-if="recentBooks.length" class="recent-reading">
      <div class="recent-reading-heading">
        <div>
          <span class="eyebrow">RECENTLY OPENED</span>
          <h3>最近阅读</h3>
        </div>
        <span>{{ recentBooks.length }} 本</span>
      </div>
      <div class="recent-reading-list">
        <button
          v-for="book in recentBooks"
          :key="book.id"
          class="recent-book"
          type="button"
          @click="emit('continue', book)"
        >
          <span class="recent-book-cover" :class="`format-${book.format}`">{{ book.format.toUpperCase() }}</span>
          <span class="recent-book-copy">
            <strong>{{ book.title }}</strong>
            <small>{{ formatProgress(book.progress) }} · {{ book.chapter_count }} 章</small>
            <small v-if="coverStateLabel(book)" class="recent-book-cover-status">{{ coverStateLabel(book) }}</small>
            <small v-if="book.shelf_group || book.tags.length" class="recent-book-metadata">
              {{ book.shelf_group || "未分组" }}<span v-if="book.tags.length"> · {{ book.tags.slice(0, 2).join(" · ") }}</span>
            </small>
            <small
              v-if="imageHealthLabel(book)"
              class="recent-book-health"
              :class="imageHealthClass(book)"
            >{{ imageHealthLabel(book) }}</small>
          </span>
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.library-overview {
  display: grid;
  gap: 18px;
  margin-top: 26px;
}

.library-overview-heading,
.recent-reading-heading {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 16px;
}

.library-overview-heading h2,
.recent-reading-heading h3 {
  margin: 8px 0 0;
  color: #eff5ff;
  font-size: 20px;
}

.library-overview-caption,
.recent-reading-heading > span {
  color: #8391a6;
  font-size: 11px;
}

.library-overview-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(250px, 0.9fr);
  gap: 14px;
}

.continue-card {
  display: flex;
  min-height: 154px;
  flex-direction: column;
  justify-content: space-between;
  gap: 18px;
  padding: 22px;
  border: 1px solid rgba(121, 201, 255, 0.27);
  border-radius: 16px;
  background:
    radial-gradient(circle at 90% 0%, rgba(121, 201, 255, 0.16), transparent 44%),
    linear-gradient(135deg, rgba(32, 59, 87, 0.92), rgba(18, 30, 49, 0.96));
  cursor: pointer;
  transition: transform 160ms ease, border-color 160ms ease;
}

.continue-card:hover,
.continue-card:focus-visible {
  border-color: rgba(139, 183, 255, 0.74);
  outline: none;
  transform: translateY(-2px);
}

.continue-card-empty {
  cursor: default;
}

.continue-card-empty:hover {
  border-color: rgba(121, 201, 255, 0.27);
  transform: none;
}

.continue-card h3 {
  max-width: 28ch;
  margin: 9px 0 5px;
  overflow: hidden;
  color: #f7fbff;
  font-size: 22px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.continue-card p {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 7px;
  margin: 0;
  color: #a9bdd3;
  font-size: 12px;
}

.book-health,
.recent-book-metadata {
  overflow: hidden;
  color: #9fb1c8;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.recent-book-health {
  display: inline-flex;
  align-items: center;
  width: fit-content;
  padding: 3px 7px;
  border-radius: 999px;
  color: #ffd39b;
  background: rgba(139, 90, 34, 0.2);
  font-size: 10px;
  line-height: 1.2;
}

.book-health.ready,
.recent-book-health.ready {
  color: #b9f6dd;
  background: rgba(30, 101, 82, 0.24);
}

.book-health.missing,
.recent-book-health.missing {
  color: #ffb0bc;
  background: rgba(188, 59, 83, 0.16);
}

.continue-card-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.continue-card-action,
.continue-card-link {
  align-self: start;
  padding: 9px 13px;
  border-radius: 9px;
  cursor: pointer;
  font: inherit;
  font-size: 12px;
}

.continue-card-action {
  border: 1px solid rgba(155, 231, 216, 0.4);
  color: #07111f;
  background: linear-gradient(135deg, #9be7d8, #79c9ff);
  font-weight: 750;
}

.continue-card-link {
  border: 1px solid rgba(148, 163, 184, 0.25);
  color: #b9c8dc;
  background: transparent;
}

.library-stat-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.library-stat-card {
  display: grid;
  min-height: 108px;
  align-content: center;
  gap: 5px;
  padding: 15px;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 14px;
  background: rgba(19, 27, 42, 0.72);
}

.library-stat-card strong {
  color: #f2f7ff;
  font-size: 26px;
  line-height: 1;
}

.library-stat-card > span:last-child {
  color: #8391a6;
  font-size: 11px;
}

.recent-reading {
  padding: 18px 20px;
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 14px;
  background: rgba(19, 27, 42, 0.46);
}

.recent-reading-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 9px;
  margin-top: 14px;
}

.recent-book {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 10px;
  padding: 9px;
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 10px;
  color: inherit;
  background: rgba(12, 17, 27, 0.48);
  cursor: pointer;
  text-align: left;
}

.recent-book:hover,
.recent-book:focus-visible {
  border-color: rgba(139, 183, 255, 0.62);
  outline: none;
}

.recent-book-cover {
  display: grid;
  width: 35px;
  height: 45px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.78);
  background: linear-gradient(145deg, #263c73, #17233e);
  font-size: 8px;
  font-weight: 800;
}

.recent-book-cover.format-epub {
  background: linear-gradient(145deg, #6f4a8e, #2c244f);
}

.recent-book-copy {
  display: grid;
  min-width: 0;
  gap: 5px;
}

.recent-book-copy strong {
  overflow: hidden;
  color: #e7eef9;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.recent-book-copy small {
  color: #8391a6;
  font-size: 10px;
}

.recent-book-cover-status {
  color: #ffd39b !important;
}

@media (max-width: 900px) {
  .library-overview-grid {
    grid-template-columns: 1fr;
  }
}
</style>
