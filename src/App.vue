<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

interface LibrarySummary {
  book_count: number;
  last_opened: string | null;
}

const status = ref("正在检查本地数据库…");
const summary = ref<LibrarySummary | null>(null);

onMounted(async () => {
  try {
    summary.value = await invoke<LibrarySummary>("get_library_summary");
    status.value = "SQLite 已连接";
  } catch (error) {
    status.value = `浏览器预览模式：${String(error)}`;
  }
});
</script>

<template>
  <main class="shell">
    <aside class="sidebar">
      <div class="brand">
        <span class="brand-mark">O</span>
        <div>
          <strong>Open Reader</strong>
          <span>Desktop · M1</span>
        </div>
      </div>

      <nav class="nav" aria-label="主导航">
        <a class="nav-item active" href="#library">书架 <span>⌘1</span></a>
        <a class="nav-item" href="#sources">书源 <span>⌘2</span></a>
        <a class="nav-item" href="#settings">设置 <span>⌘,</span></a>
      </nav>

      <div class="sidebar-note">
        <span class="eyebrow">PROJECT STATUS</span>
        <p>本阶段先打通桌面壳、前端入口和本地数据层。</p>
      </div>
    </aside>

    <section class="content" id="library">
      <header class="topbar">
        <div>
          <span class="eyebrow">YOUR LIBRARY</span>
          <h1>书架</h1>
        </div>
        <button class="ghost-button" type="button" disabled>导入书籍 · M2</button>
      </header>

      <section class="hero-card">
        <div>
          <span class="eyebrow">M1 FOUNDATION</span>
          <h2>桌面阅读器的第一块地基。</h2>
          <p>这不是最终阅读界面，而是验证 Tauri、Vue、Rust 和 SQLite 已经连通的工作台。</p>
        </div>
        <div class="status-pill"><i></i>{{ status }}</div>
      </section>

      <section class="stats-grid" aria-label="本地数据概览">
        <article class="stat-card">
          <span class="eyebrow">BOOKS</span>
          <strong>{{ summary?.book_count ?? "—" }}</strong>
          <span>本地书籍</span>
        </article>
        <article class="stat-card">
          <span class="eyebrow">STORAGE</span>
          <strong>SQLite</strong>
          <span>应用数据层</span>
        </article>
        <article class="stat-card">
          <span class="eyebrow">NEXT</span>
          <strong>M2</strong>
          <span>TXT / EPUB 阅读</span>
        </article>
      </section>

      <section class="empty-state">
        <div class="empty-icon">✦</div>
        <h3>书架还是空的</h3>
        <p>下一阶段会加入 TXT / EPUB 导入、目录解析、阅读进度和离线缓存。</p>
        <a href="https://github.com/TttXxx36/Open-reader-desktop/issues/3" target="_blank" rel="noreferrer">查看 M2 Issue →</a>
      </section>
    </section>
  </main>
</template>
