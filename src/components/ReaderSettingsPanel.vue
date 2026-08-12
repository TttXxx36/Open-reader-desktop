<script setup lang="ts">
type ReaderTheme = "night" | "paper" | "sepia" | "custom";
type ReaderFont = "system" | "yahei" | "serif" | "kai";
type ReaderTextAlign = "left" | "justify" | "center";
type ReaderMode = "scroll" | "paged";

interface ReaderSettings {
  version: number;
  fontFamily: ReaderFont;
  fontSize: number;
  lineHeight: number;
  letterSpacing: number;
  contentWidth: number;
  marginLeft: number;
  marginRight: number;
  paragraphSpacing: number;
  textIndent: number;
  textAlign: ReaderTextAlign;
  readingMode: ReaderMode;
  theme: ReaderTheme;
  customBackground: string;
  customText: string;
  customAccent: string;
}

const props = defineProps<{ modelValue: ReaderSettings }>();
const emit = defineEmits<{
  "update:modelValue": [value: ReaderSettings];
  reset: [];
}>();

function setValue<K extends keyof ReaderSettings>(key: K, value: ReaderSettings[K]) {
  emit("update:modelValue", { ...props.modelValue, [key]: value });
}

function setNumber(key: keyof ReaderSettings, event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  setValue(key, value as ReaderSettings[typeof key]);
}

function setString(key: keyof ReaderSettings, event: Event) {
  setValue(key, (event.target as HTMLInputElement).value as ReaderSettings[typeof key]);
}
</script>

<template>
  <section class="settings-panel">
    <div class="settings-section-heading">
      <div>
        <span class="eyebrow">READER</span>
        <h2>阅读外观</h2>
      </div>
      <button class="source-link-button" type="button" @click="emit('reset')">恢复默认</button>
    </div>

    <div class="settings-grid">
      <label class="settings-field">
        <span>字体</span>
        <select :value="modelValue.fontFamily" @change="setString('fontFamily', $event)">
          <option value="system">系统无衬线</option>
          <option value="yahei">微软雅黑</option>
          <option value="serif">宋体/衬线</option>
          <option value="kai">楷体</option>
        </select>
      </label>
      <label class="settings-field">
        <span>主题</span>
        <select :value="modelValue.theme" @change="setString('theme', $event)">
          <option value="night">夜间</option>
          <option value="paper">纸张</option>
          <option value="sepia">暖色</option>
          <option value="custom">自定义</option>
        </select>
      </label>
      <label class="settings-field">
        <span>阅读模式</span>
        <select :value="modelValue.readingMode" @change="setString('readingMode', $event)">
          <option value="scroll">连续滚动</option>
          <option value="paged">分页滚动</option>
        </select>
      </label>
      <label class="settings-field">
        <span>文本对齐</span>
        <select :value="modelValue.textAlign" @change="setString('textAlign', $event)">
          <option value="left">左对齐</option>
          <option value="justify">两端对齐</option>
          <option value="center">居中</option>
        </select>
      </label>
      <label class="settings-field">
        <span>字号 <strong>{{ modelValue.fontSize }} px</strong></span>
        <input :value="modelValue.fontSize" type="range" min="15" max="30" step="1" @input="setNumber('fontSize', $event)" />
      </label>
      <label class="settings-field">
        <span>行距 <strong>{{ modelValue.lineHeight.toFixed(1) }}</strong></span>
        <input :value="modelValue.lineHeight" type="range" min="1.4" max="2.4" step="0.1" @input="setNumber('lineHeight', $event)" />
      </label>
      <label class="settings-field">
        <span>字间距 <strong>{{ modelValue.letterSpacing.toFixed(2) }}em</strong></span>
        <input :value="modelValue.letterSpacing" type="range" min="-0.02" max="0.12" step="0.01" @input="setNumber('letterSpacing', $event)" />
      </label>
      <label class="settings-field">
        <span>版心宽度 <strong>{{ modelValue.contentWidth }} px</strong></span>
        <input :value="modelValue.contentWidth" type="range" min="560" max="1100" step="20" @input="setNumber('contentWidth', $event)" />
      </label>
      <label class="settings-field">
        <span>左边距 <strong>{{ modelValue.marginLeft }} px</strong></span>
        <input :value="modelValue.marginLeft" type="range" min="0" max="96" step="4" @input="setNumber('marginLeft', $event)" />
      </label>
      <label class="settings-field">
        <span>右边距 <strong>{{ modelValue.marginRight }} px</strong></span>
        <input :value="modelValue.marginRight" type="range" min="0" max="96" step="4" @input="setNumber('marginRight', $event)" />
      </label>
      <label class="settings-field">
        <span>段间距 <strong>{{ modelValue.paragraphSpacing.toFixed(1) }}em</strong></span>
        <input :value="modelValue.paragraphSpacing" type="range" min="0.4" max="2.4" step="0.1" @input="setNumber('paragraphSpacing', $event)" />
      </label>
      <label class="settings-field">
        <span>首行缩进 <strong>{{ modelValue.textIndent.toFixed(1) }}em</strong></span>
        <input :value="modelValue.textIndent" type="range" min="0" max="2" step="0.5" @input="setNumber('textIndent', $event)" />
      </label>
      <label class="settings-field settings-color-field">
        <span>自定义背景色</span>
        <input :value="modelValue.customBackground" type="color" :disabled="modelValue.theme !== 'custom'" @input="setString('customBackground', $event)" />
      </label>
      <label class="settings-field settings-color-field">
        <span>自定义文字色</span>
        <input :value="modelValue.customText" type="color" :disabled="modelValue.theme !== 'custom'" @input="setString('customText', $event)" />
      </label>
      <label class="settings-field settings-color-field">
        <span>自定义强调色</span>
        <input :value="modelValue.customAccent" type="color" :disabled="modelValue.theme !== 'custom'" @input="setString('customAccent', $event)" />
      </label>
    </div>

    <p class="settings-note">
      设置会按版本保存到本机；升级后会自动迁移旧版本。TXT 保持纯文本回退，EPUB 内容块会保留标题、引用、强调和安全内嵌图片。
    </p>
  </section>
</template>

<style scoped>
.settings-panel {
  margin-top: 28px;
  padding: 26px;
  border: 1px solid rgba(211, 224, 241, 0.13);
  border-radius: 20px;
  background: linear-gradient(145deg, rgba(17, 30, 48, 0.88), rgba(10, 20, 34, 0.8));
  box-shadow: 0 20px 52px rgba(1, 8, 18, 0.24);
}

.settings-section-heading {
  display: flex;
  align-items: start;
  justify-content: space-between;
  gap: 18px;
}

.settings-section-heading h2 {
  margin: 9px 0 0;
  color: #eef3fb;
  font-family: "Noto Serif CJK SC", "Source Han Serif SC", "Microsoft YaHei", serif;
  font-size: 21px;
  letter-spacing: -0.03em;
}

.settings-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px 16px;
  margin-top: 24px;
}

.settings-field {
  display: grid;
  gap: 9px;
  color: #acb9ca;
  font-size: 12px;
}

.settings-field > span {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  letter-spacing: 0.01em;
}

.settings-field > span::before {
  width: 5px;
  height: 5px;
  flex: 0 0 auto;
  margin-right: 5px;
  border-radius: 50%;
  background: #e8b66f;
  box-shadow: 0 0 8px rgba(232, 182, 111, 0.45);
  content: "";
}

.settings-field > span {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
}

.settings-field > span strong {
  justify-self: end;
  color: #f3d39d;
  font-variant-numeric: tabular-nums;
  font-weight: 650;
}

.settings-field select {
  width: 100%;
  min-height: 40px;
  padding: 9px 34px 9px 12px;
  border: 1px solid rgba(211, 224, 241, 0.17);
  border-radius: 10px;
  color: #eef3fb;
  background:
    linear-gradient(45deg, transparent 50%, #e8b66f 50%) calc(100% - 16px) 17px / 5px 5px no-repeat,
    linear-gradient(135deg, #e8b66f 50%, transparent 50%) calc(100% - 12px) 17px / 5px 5px no-repeat,
    rgba(6, 15, 27, 0.84);
  appearance: none;
  cursor: pointer;
  transition: border-color 180ms ease, box-shadow 180ms ease;
}

.settings-field select:hover {
  border-color: rgba(232, 182, 111, 0.42);
}

.settings-field select:focus {
  border-color: rgba(232, 182, 111, 0.68);
  box-shadow: 0 0 0 3px rgba(232, 182, 111, 0.12);
  outline: none;
}

.settings-field input[type="range"] {
  width: 100%;
  height: 5px;
  margin: 10px 0 7px;
  border-radius: 99px;
  background: linear-gradient(90deg, rgba(134, 223, 194, 0.72), rgba(232, 182, 111, 0.82));
  cursor: pointer;
  accent-color: #e8b66f;
}

.settings-field input[type="range"]::-webkit-slider-runnable-track {
  height: 5px;
  border-radius: 99px;
  background: rgba(211, 224, 241, 0.16);
}

.settings-field input[type="range"]::-webkit-slider-thumb {
  width: 15px;
  height: 15px;
  margin-top: -5px;
  border: 2px solid #f3d39d;
  border-radius: 50%;
  background: #102033;
  box-shadow: 0 0 0 4px rgba(232, 182, 111, 0.12);
  appearance: none;
}

.settings-field input[type="range"]:focus-visible {
  outline: 2px solid rgba(232, 182, 111, 0.72);
  outline-offset: 4px;
}

.settings-field input[type="color"] {
  width: 100%;
  min-height: 40px;
  padding: 3px;
  border: 1px solid rgba(232, 182, 111, 0.26);
  border-radius: 10px;
  background: rgba(6, 15, 27, 0.84);
  cursor: pointer;
}

.settings-field input[type="color"]:disabled {
  cursor: not-allowed;
  opacity: 0.42;
}

.settings-note {
  margin: 24px 0 0;
  padding-top: 16px;
  border-top: 1px solid rgba(211, 224, 241, 0.1);
  color: #92a1b6;
  font-size: 12px;
  line-height: 1.75;
}

@media (max-width: 720px) {
  .settings-panel {
    padding: 20px;
  }

  .settings-grid {
    grid-template-columns: 1fr;
  }
}
</style>
