<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const props = defineProps<{
  zhipuApiKey: string
}>()

const localApiKey = ref(localStorage.getItem('zhipu_api_key') || '')

interface ClaudeCodeStatus {
  installed: boolean
  version: string | null
  path: string | null
  config_path: string | null
}

interface ClaudeCodeConfig {
  model: string | null
  anthropic_auth_token: string | null
  anthropic_base_url: string | null
  anthropic_default_haiku_model: string | null
  anthropic_default_sonnet_model: string | null
  anthropic_default_opus_model: string | null
  api_timeout_ms: string | null
  broken_plugins: string[]
}

const status = ref<ClaudeCodeStatus | null>(null)
const loading = ref(false)
const error = ref('')
const saved = ref(false)

// Hook 相关状态
const hookEnabled = ref(false)
const hookLoading = ref(false)
const hookTestResult = ref('')

const model = ref('')
const authToken = ref('')
const showToken = ref(false)
const baseUrl = ref('')
const haikuModel = ref('')
const sonnetModel = ref('')
const opusModel = ref('')
const timeoutMs = ref('')
const brokenPlugins = ref<string[]>([])
const fixingPlugins = ref(false)

async function detect() {
  try {
    status.value = await invoke<ClaudeCodeStatus>('detect_claude_code')
  } catch (e) {
    error.value = String(e)
  }
}

async function loadConfig() {
  try {
    const config = await invoke<ClaudeCodeConfig>('read_claude_config')
    model.value = config.model ?? ''
    authToken.value = config.anthropic_auth_token ?? ''
    baseUrl.value = config.anthropic_base_url ?? ''
    haikuModel.value = config.anthropic_default_haiku_model ?? ''
    sonnetModel.value = config.anthropic_default_sonnet_model ?? ''
    opusModel.value = config.anthropic_default_opus_model ?? ''
    timeoutMs.value = config.api_timeout_ms ?? ''
    brokenPlugins.value = config.broken_plugins ?? []
    error.value = ''
  } catch (e) {
    error.value = String(e)
  }
}

async function saveConfig() {
  try {
    await invoke('save_claude_config', {
      model: model.value || null,
      anthropicAuthToken: authToken.value || null,
      anthropicBaseUrl: baseUrl.value || null,
      anthropicDefaultHaikuModel: haikuModel.value || null,
      anthropicDefaultSonnetModel: sonnetModel.value || null,
      anthropicDefaultOpusModel: opusModel.value || null,
      apiTimeoutMs: timeoutMs.value || null,
    })
    saved.value = true
    setTimeout(() => { saved.value = false }, 2000)
  } catch (e) {
    error.value = String(e)
  }
}

async function fixBrokenPlugins() {
  fixingPlugins.value = true
  try {
    // 通过 setup_claude_hook 触发插件清理（不改变 hook 状态）
    await invoke('setup_claude_hook', { enabled: hookEnabled.value })
    brokenPlugins.value = []
  } catch (e) {
    error.value = String(e)
  }
  fixingPlugins.value = false
}

async function refresh() {
  loading.value = true
  error.value = ''
  await detect()
  if (status.value?.installed) {
    await loadConfig()
    await checkHookStatus()
  }
  loading.value = false
}

async function checkHookStatus() {
  try {
    const result = await invoke<{ installed: boolean }>('check_claude_hook_status')
    hookEnabled.value = result.installed
  } catch {
    hookEnabled.value = false
  }
}

async function installHook() {
  hookLoading.value = true
  hookTestResult.value = ''
  try {
    await invoke('setup_claude_hook', { enabled: true })
    hookEnabled.value = true
  } catch (e) {
    error.value = String(e)
  }
  hookLoading.value = false
}

async function uninstallHook() {
  hookLoading.value = true
  hookTestResult.value = ''
  try {
    await invoke('setup_claude_hook', { enabled: false })
    hookEnabled.value = false
  } catch (e) {
    error.value = String(e)
  }
  hookLoading.value = false
}

async function testHook() {
  hookTestResult.value = '运行中...'
  try {
    const result = await invoke<string>('test_zhipukit_status')
    hookTestResult.value = result
  } catch (e) {
    hookTestResult.value = `错误: ${e}`
  }
}

const copiedPath = ref('')

function copyToClipboard(text: string) {
  navigator.clipboard.writeText(text)
  copiedPath.value = text
  setTimeout(() => { copiedPath.value = '' }, 1500)
}

function fillFromZhipu() {
  const key = props.zhipuApiKey || localApiKey.value
  if (key) {
    authToken.value = key
  }
}

function maskToken(token: string): string {
  if (!token) return ''
  if (token.length <= 8) return '****'
  return token.slice(0, 4) + '****' + token.slice(-4)
}

onMounted(() => refresh())
</script>

<template>
  <div class="claude-config">
    <h2 class="page-title">Claude Code 配置</h2>

    <!-- 安装状态 -->
    <div class="setting-card">
      <div class="card-header">
        <div class="card-icon terminal-icon">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="4 17 10 11 4 5"/>
            <line x1="12" y1="19" x2="20" y2="19"/>
          </svg>
        </div>
        <div>
          <div class="card-title">安装状态</div>
          <div class="card-desc">
            <span v-if="loading">检测中...</span>
            <span v-else-if="status?.installed" class="status-installed">
              已安装{{ status.version ? ` v${status.version}` : '' }}
            </span>
            <span v-else class="status-not-found">未检测到 Claude Code</span>
          </div>
        </div>
        <button class="dev-btn" style="margin-left: auto;" @click="refresh">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M18.52 13.45a8 8 0 0 1-11.06 5.56"/><path d="M5.48 10.55a8 8 0 0 1 11.06-5.56"/>
            <polyline points="15 2 18.54 5.46 15.01 8.99"/><polyline points="9 22 5.46 18.54 8.99 15.01"/>
          </svg>
          重新检测
        </button>
      </div>
      <div v-if="status?.installed && status.path" class="path-display">
        <span class="path-label">可执行路径</span>
        <code class="path-value">{{ status.path }}</code>
        <button class="copy-btn" @click="copyToClipboard(status.path!)" :title="copiedPath === status.path ? '已复制' : '复制'">
          <svg v-if="copiedPath !== status.path" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
          <svg v-else width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
        </button>
      </div>
      <div v-if="status?.config_path" class="path-display" :style="{ marginTop: status?.installed ? '4px' : undefined }">
        <span class="path-label">配置文件</span>
        <code class="path-value">{{ status.config_path }}</code>
        <button class="copy-btn" @click="copyToClipboard(status.config_path!)" :title="copiedPath === status.config_path ? '已复制' : '复制'">
          <svg v-if="copiedPath !== status.config_path" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
          <svg v-else width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
        </button>
      </div>
    </div>

    <!-- 插件异常警告 -->
    <div v-if="brokenPlugins.length > 0 && status?.installed" class="warning-banner">
      <div class="warning-content">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
          <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
        <div>
          <div class="warning-title">检测到无效插件</div>
          <div class="warning-desc">
            以下插件安装不完整，可能导致 SessionStart hook 报错：
            <code v-for="p in brokenPlugins" :key="p" class="broken-plugin-name">{{ p }}</code>
          </div>
        </div>
      </div>
      <button class="fix-btn" @click="fixBrokenPlugins" :disabled="fixingPlugins">
        {{ fixingPlugins ? '修复中...' : '一键修复' }}
      </button>
    </div>

    <!-- 会话注入 -->
    <div v-if="status?.installed" class="setting-card">
      <div class="card-header">
        <div class="card-icon hook-icon">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/>
          </svg>
        </div>
        <div>
          <div class="card-title">会话注入</div>
          <div class="card-desc">在 Claude Code 启动时自动显示智谱套餐信息</div>
        </div>
        <button
          v-if="!hookEnabled"
          class="hook-action-btn install-btn"
          @click="installHook"
          :disabled="hookLoading"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          {{ hookLoading ? '处理中...' : '安装' }}
        </button>
        <button
          v-else
          class="hook-action-btn uninstall-btn"
          @click="uninstallHook"
          :disabled="hookLoading"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
          </svg>
          {{ hookLoading ? '处理中...' : '卸载' }}
        </button>
      </div>
      <div v-if="hookEnabled" class="hook-details">
        <button class="dev-btn" @click="testHook" :disabled="hookTestResult === '运行中...'">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="5 3 19 12 5 21 5 3"/>
          </svg>
          测试输出
        </button>
        <div v-if="hookTestResult" :class="['debug-output', { ok: !hookTestResult.startsWith('错误'), fail: hookTestResult.startsWith('错误') }]">
          <pre>{{ hookTestResult }}</pre>
        </div>
      </div>
    </div>

    <!-- 配置编辑（仅安装后显示） -->
    <template v-if="status?.installed">
      <!-- API 密钥 -->
      <div class="setting-card">
        <div class="card-header">
          <div class="card-icon key-icon">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/>
            </svg>
          </div>
          <div>
            <div class="card-title">API 密钥</div>
            <div class="card-desc">ANTHROPIC_AUTH_TOKEN 配置</div>
          </div>
          <button
            class="fill-btn"
            :disabled="!zhipuApiKey && !localApiKey"
            @click="fillFromZhipu"
            title="将设置中的 API Key 填入此字段"
          >
            一键填入
          </button>
        </div>
        <div class="fields">
          <div class="field-row">
            <label class="field-label">ANTHROPIC_AUTH_TOKEN</label>
            <div class="input-group">
              <input
                :type="showToken ? 'text' : 'password'"
                v-model="authToken"
                class="input-field"
                placeholder="输入 API 密钥"
              />
              <button class="toggle-btn" @click="showToken = !showToken" :title="showToken ? '隐藏' : '显示'">
                <svg v-if="!showToken" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/>
                </svg>
                <svg v-else width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/><line x1="1" y1="1" x2="23" y2="23"/>
                </svg>
              </button>
            </div>
          </div>
          <div v-if="authToken" class="key-preview">{{ maskToken(authToken) }}</div>
        </div>
      </div>

      <!-- 默认模型 -->
      <div class="setting-card">
        <div class="card-header">
          <div class="card-icon model-icon">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="4" y="4" width="16" height="16" rx="2"/>
              <rect x="9" y="9" width="6" height="6"/>
              <path d="M15 2v2"/><path d="M15 20v2"/><path d="M2 15h2"/><path d="M2 9h2"/>
              <path d="M20 15h2"/><path d="M20 9h2"/><path d="M9 2v2"/><path d="M9 20v2"/>
            </svg>
          </div>
          <div>
            <div class="card-title">默认模型</div>
            <div class="card-desc">Claude Code 使用的默认模型和超时设置</div>
          </div>
        </div>
        <div class="fields">
          <div class="field-row">
            <label class="field-label">model</label>
            <input v-model="model" class="input-field" placeholder="opus[1m]" />
          </div>
          <div class="field-row">
            <label class="field-label">API_TIMEOUT_MS</label>
            <input v-model="timeoutMs" class="input-field" placeholder="3000000" />
          </div>
        </div>
      </div>

      <!-- API 端点 -->
      <div class="setting-card">
        <div class="card-header">
          <div class="card-icon globe-icon">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
            </svg>
          </div>
          <div>
            <div class="card-title">API 端点</div>
            <div class="card-desc">ANTHROPIC_BASE_URL 配置</div>
          </div>
        </div>
        <div class="fields">
          <div class="field-row">
            <label class="field-label">ANTHROPIC_BASE_URL</label>
            <input v-model="baseUrl" class="input-field" placeholder="https://api.anthropic.com" />
          </div>
        </div>
      </div>

      <!-- 模型映射 -->
      <div class="setting-card">
        <div class="card-header">
          <div class="card-icon layers-icon">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polygon points="12 2 2 7 12 12 22 7 12 2"/>
              <polyline points="2 17 12 22 22 17"/>
              <polyline points="2 12 12 17 22 12"/>
            </svg>
          </div>
          <div>
            <div class="card-title">模型映射</div>
            <div class="card-desc">自定义各层级使用的模型</div>
          </div>
        </div>
        <div class="fields">
          <div class="field-row">
            <label class="field-label">Haiku</label>
            <input v-model="haikuModel" class="input-field" placeholder="claude-haiku-4-5-20251001" />
          </div>
          <div class="field-row">
            <label class="field-label">Sonnet</label>
            <input v-model="sonnetModel" class="input-field" placeholder="claude-sonnet-4-20250514" />
          </div>
          <div class="field-row">
            <label class="field-label">Opus</label>
            <input v-model="opusModel" class="input-field" placeholder="claude-opus-4-20250514" />
          </div>
        </div>
      </div>

      <button class="save-btn" @click="saveConfig">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="20 6 9 17 4 12"/>
        </svg>
        {{ saved ? '已保存' : '保存配置' }}
      </button>
    </template>

    <!-- 错误提示 -->
    <div v-if="error" class="error-banner">
      <span>{{ error }}</span>
      <button class="error-close" @click="error = ''">&times;</button>
    </div>
  </div>
</template>

<style scoped>
.claude-config {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.page-title {
  font-size: 22px;
  font-weight: 700;
  color: var(--text);
  margin-bottom: 8px;
}

.setting-card {
  padding: 16px 0;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.card-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.terminal-icon {
  background: rgba(139, 92, 246, 0.08);
  color: #8b5cf6;
}

.key-icon {
  background: var(--accent-light);
  color: var(--accent);
}

.model-icon {
  background: var(--accent-light);
  color: var(--accent);
}

.globe-icon {
  background: var(--success-light);
  color: var(--success);
}

.layers-icon {
  background: var(--warning-light);
  color: var(--warning);
}

.card-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}

.card-desc {
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: 2px;
}

.status-installed {
  color: var(--success);
  font-weight: 500;
}

.status-not-found {
  color: var(--text-secondary);
}

.setting-card > :not(.card-header) {
  margin-left: 48px;
}

.path-display {
  display: flex;
  align-items: center;
  gap: 8px;
}

.path-label {
  font-size: 11px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.path-value {
  font-size: 11px;
  font-family: ui-monospace, SFMono-Regular, monospace;
  color: var(--text-secondary);
  background: var(--bg);
  padding: 2px 8px;
  border-radius: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.copy-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: none;
  border: 1px solid transparent;
  border-radius: 4px;
  color: var(--text-secondary);
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.15s;
}

.copy-btn:hover {
  color: var(--accent);
  border-color: var(--accent);
  background: var(--accent-light);
}

.fields {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.field-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.field-label {
  font-size: 11px;
  color: var(--text-secondary);
  width: 180px;
  flex-shrink: 0;
  font-family: ui-monospace, SFMono-Regular, monospace;
}

.input-group {
  display: flex;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.input-field {
  flex: 1;
  min-width: 0;
}

.toggle-btn {
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius-xs);
  color: var(--text-secondary);
  transition: all 0.15s;
  flex-shrink: 0;
}

.toggle-btn:hover {
  color: var(--text);
  border-color: var(--text-secondary);
}

.key-preview {
  margin-top: 4px;
  font-size: 12px;
  color: var(--text-secondary);
  font-family: ui-monospace, monospace;
}

.fill-btn {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 12px;
  background: var(--accent-light);
  border: 1px solid var(--accent);
  border-radius: var(--radius-xs);
  color: var(--accent);
  font-size: 11px;
  font-weight: 600;
  transition: all 0.15s;
  white-space: nowrap;
}

.fill-btn:hover:not(:disabled) {
  background: var(--accent);
  color: #fff;
}

.fill-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.save-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 16px;
  background: var(--accent-gradient);
  color: #fff;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  transition: opacity 0.2s;
  margin-left: 48px;
}

.save-btn:hover { opacity: 0.9; }

.dev-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius-xs);
  color: var(--text);
  font-size: 12px;
  font-weight: 500;
  transition: all 0.15s;
}

.dev-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.error-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  background: var(--danger-light);
  border: 1px solid var(--danger);
  border-radius: var(--radius-xs);
  color: var(--danger);
  font-size: 12px;
}

.error-close {
  background: none;
  border: none;
  color: var(--danger);
  font-size: 16px;
  cursor: pointer;
  padding: 0 4px;
}

.hook-icon {
  background: var(--success-light);
  color: var(--success);
}

.hook-details {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.hook-action-btn {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border-radius: var(--radius-xs);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s;
  flex-shrink: 0;
  white-space: nowrap;
}

.hook-action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.install-btn {
  background: var(--accent);
  border: 1px solid var(--accent);
  color: #fff;
}

.install-btn:hover:not(:disabled) {
  opacity: 0.85;
}

.uninstall-btn {
  background: none;
  border: 1px solid var(--border);
  color: var(--text-secondary);
}

.uninstall-btn:hover:not(:disabled) {
  border-color: var(--danger, #ef4444);
  color: var(--danger, #ef4444);
  background: var(--danger-light, rgba(239, 68, 68, 0.08));
}

.debug-output {
  margin-top: 4px;
  border-radius: var(--radius-xs);
  overflow: hidden;
}

.debug-output pre {
  padding: 12px 14px;
  font-size: 11px;
  line-height: 1.6;
  font-family: ui-monospace, SFMono-Regular, monospace;
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
}

.debug-output.ok {
  background: var(--success-light);
  border: 1px solid var(--success);
}

.debug-output.ok pre { color: var(--success); }

.debug-output.fail {
  background: var(--danger-light);
  border: 1px solid var(--danger);
}

.debug-output.fail pre { color: var(--danger); }

.warning-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
  background: var(--warning-light, rgba(245, 158, 11, 0.08));
  border: 1px solid var(--warning, #f59e0b);
  border-radius: var(--radius-xs, 6px);
  color: var(--warning, #f59e0b);
  gap: 12px;
}

.warning-content {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  flex: 1;
  min-width: 0;
}

.warning-title {
  font-size: 13px;
  font-weight: 600;
}

.warning-desc {
  font-size: 11px;
  margin-top: 2px;
  opacity: 0.85;
  line-height: 1.5;
}

.broken-plugin-name {
  display: inline-block;
  background: rgba(245, 158, 11, 0.15);
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 10px;
  font-family: ui-monospace, SFMono-Regular, monospace;
  margin: 2px 2px 0;
}

.fix-btn {
  flex-shrink: 0;
  padding: 5px 14px;
  background: var(--warning, #f59e0b);
  border: none;
  border-radius: var(--radius-xs, 6px);
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: opacity 0.15s;
  white-space: nowrap;
}

.fix-btn:hover:not(:disabled) { opacity: 0.85; }
.fix-btn:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
