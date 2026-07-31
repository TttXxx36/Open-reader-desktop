import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const strict = process.argv.includes("--strict");
const requireDist = process.argv.includes("--dist");
const checks = [];
const blockers = [];
const errors = [];

function addCheck(label, detail) {
  checks.push({ label, detail });
}

function addBlocker(label, detail) {
  blockers.push({ label, detail });
}

function addError(label, detail) {
  errors.push({ label, detail });
}

function readJson(relativePath) {
  const absolutePath = resolve(root, relativePath);
  try {
    return JSON.parse(readFileSync(absolutePath, "utf8"));
  } catch (error) {
    addError(relativePath, error instanceof Error ? error.message : String(error));
    return null;
  }
}

function hasExpectedIconSignature(relativePath) {
  const bytes = readFileSync(resolve(root, "src-tauri", relativePath));
  if (relativePath.toLowerCase().endsWith(".ico")) {
    return bytes.length >= 4 && bytes.subarray(0, 4).equals(Buffer.from([0, 0, 1, 0]));
  }

  if (relativePath.toLowerCase().endsWith(".png")) {
    return bytes.length >= 8 && bytes.subarray(0, 8).equals(
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    );
  }

  return false;
}

const packageJson = readJson("package.json");
const tauriConfig = readJson("src-tauri/tauri.conf.json");

if (packageJson && tauriConfig) {
  if (packageJson.version === tauriConfig.version) {
    addCheck("version", packageJson.version);
  } else {
    addError(
      "version",
      `package.json=${packageJson.version}, tauri.conf.json=${tauriConfig.version}`,
    );
  }

  if (tauriConfig.productName === "Open Reader Desktop") {
    addCheck("product name", tauriConfig.productName);
  } else {
    addError("product name", `unexpected value: ${tauriConfig.productName}`);
  }

  const bundle = tauriConfig.bundle ?? {};
  if (bundle.active === true) {
    addCheck("bundle.active", "true");
  } else {
    addBlocker(
      "bundle.active",
      "仍为 false；完成图标、签名和安装回归后再切换为 true",
    );
  }

  const targets = Array.isArray(bundle.targets) ? bundle.targets : [];
  const requiredTargets = ["nsis", "msi"];
  if (requiredTargets.every((target) => targets.includes(target))) {
    addCheck("bundle targets", requiredTargets.join(", "));
  } else {
    addBlocker(
      "bundle targets",
      "需要同时配置 nsis 和 msi，分别生成安装器与 MSI 包",
    );
  }

  const icons = Array.isArray(bundle.icon) ? bundle.icon : [];
  if (!icons.length) {
    addBlocker("bundle icons", "未配置图标路径");
  } else {
    for (const icon of icons) {
      const iconPath = resolve(root, "src-tauri", icon);
      if (!existsSync(iconPath)) {
        addBlocker("bundle icon", `${icon} 不存在`);
        continue;
      }

      const size = statSync(iconPath).size;
      if (size < 1024) {
        addBlocker("bundle icon", `${icon} 只有 ${size} bytes，疑似占位文件`);
      } else if (!hasExpectedIconSignature(icon)) {
        addBlocker("bundle icon", `${icon} 文件头不是有效的 ICO/PNG 格式`);
      } else {
        addCheck("bundle icon", `${icon} (${size} bytes)`);
      }
    }
  }

  if (requireDist) {
    const distPath = resolve(root, "dist", "index.html");
    if (existsSync(distPath)) {
      addCheck("frontend dist", "dist/index.html");
    } else {
      addError("frontend dist", "dist/index.html 不存在，请先运行 npm run build");
    }
  }
}

const ready = errors.length === 0 && blockers.length === 0;
console.log(`release preflight: ${ready ? "READY" : "BLOCKED"}`);
for (const check of checks) {
  console.log(`✓ ${check.label}: ${check.detail}`);
}
for (const blocker of blockers) {
  console.log(`! ${blocker.label}: ${blocker.detail}`);
}
for (const error of errors) {
  console.log(`✗ ${error.label}: ${error.detail}`);
}

if (strict && !ready) {
  process.exitCode = 1;
} else if (errors.length > 0) {
  process.exitCode = 1;
}
