<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import SettingCard from '../components/SettingCard.vue'
import IntervalSelector from '../components/IntervalSelector.vue'
import ToggleBtn from '../components/ToggleBtn.vue'
import ToggleSwitch from '../components/ToggleSwitch.vue'

const apiKey = defineModel<string>('apiKey')
const endpoint = defineModel<string>('endpoint')

const showKey = ref(false)
const saved = ref(false)
const debugLoading = ref(false)
const debugResult = ref<{ ok: boolean; msg: string; data?: string } | null>(null)
const appInfo = ref('')
const appVersion = ref('')

const autoRefresh = ref(localStorage.getItem('zhipu_auto_refresh') === 'true')
const refreshInterval = ref(Number(localStorage.getItem('zhipu_refresh_interval') || '30'))
// 已生效的间隔（用于倒计时计算，只有保存后才更新）
const appliedInterval = ref(refreshInterval.value)


const autoStart = ref(false)
let autostartModule: typeof import('@tauri-apps/plugin-autostart') | null = null

onMounted(async () => {
  try {
    const info = await invoke<Record<string, string>>('get_app_info')
    appVersion.value = info.version || ''
  } catch {}
  try {
    autostartModule = await import('@tauri-apps/plugin-autostart')
    autoStart.value = await autostartModule.isEnabled()
  } catch (e) {
    console.error('autostart plugin load failed:', e)
  }
})

async function toggleAutoStart() {
  if (!autostartModule) {
    alert('开机启动插件未加载')
    return
  }
  try {
    if (autoStart.value) {
      await autostartModule.disable()
      autoStart.value = false
    } else {
      await autostartModule.enable()
      autoStart.value = true
    }
  } catch (e) {
    alert('开机启动设置失败: ' + e)
  }
}

// 倒计时：基于已生效的间隔计算
const now = ref(Date.now())
setInterval(() => { now.value = Date.now() }, 1000)

function getLastRefresh(): number {
  return Number(localStorage.getItem('zhipu_last_refresh') || '0')
}

function getRemainSec(): number {
  if (!autoRefresh.value) return 0
  const last = getLastRefresh()
  if (last === 0) return 0
  const elapsed = Math.floor((now.value - last) / 1000)
  return Math.max(0, appliedInterval.value - elapsed)
}

watch(autoRefresh, (v) => {
  localStorage.setItem('zhipu_auto_refresh', String(v))
})

// 监听后端刷新事件，更新时间戳（防止切换页面时卡住）
let unlistenBalance: UnlistenFn | null = null
let unlistenPlan: UnlistenFn | null = null

listen('balance-update', () => {
  localStorage.setItem('zhipu_last_refresh', String(Date.now()))
}).then(fn => { unlistenBalance = fn })

listen('plan-update', () => {
  localStorage.setItem('zhipu_last_refresh', String(Date.now()))
}).then(fn => { unlistenPlan = fn })

onBeforeUnmount(() => {
  if (unlistenBalance) unlistenBalance()
  if (unlistenPlan) unlistenPlan()
})

const endpoints = [
  { label: '国内版 (open.bigmodel.cn)', value: 'https://open.bigmodel.cn' },
  { label: '国际版 (api.z.ai)', value: 'https://api.z.ai' },
]

function save() {
  localStorage.setItem('zhipu_api_key', apiKey.value ?? '')
  localStorage.setItem('zhipu_endpoint', endpoint.value ?? '')
  localStorage.setItem('zhipu_refresh_interval', String(refreshInterval.value))
  localStorage.setItem('zhipu_auto_refresh', String(autoRefresh.value))
  appliedInterval.value = refreshInterval.value
  // 同步 endpoint 到 settings.json，供 zhipukit-status 读取
  if (endpoint.value) {
    invoke('save_zhipu_endpoint', { endpoint: endpoint.value }).catch(() => {})
  }
  if (autoRefresh.value && apiKey.value) {
    invoke('start_auto_refresh', {
      apiKey: apiKey.value,
      endpoint: endpoint.value,
      intervalSecs: refreshInterval.value,
    }).catch(() => {})
  } else {
    invoke('stop_auto_refresh').catch(() => {})
  }
  saved.value = true
  setTimeout(() => { saved.value = false }, 2000)
}

function maskKey(key: string): string {
  if (!key) return ''
  if (key.length <= 8) return '****'
  return key.slice(0, 4) + '****' + key.slice(-4)
}

async function runDebug() {
  if (!apiKey.value) {
    debugResult.value = { ok: false, msg: '请先填写 API Key' }
    return
  }
  debugLoading.value = true
  debugResult.value = null

  const results: string[] = []

  // Test 1: Coding Plan
  try {
    const r = await invoke('query_coding_plan', { apiKey: apiKey.value, endpoint: endpoint.value })
    results.push(`[Coding Plan] OK — ${JSON.stringify(r)}`)
  } catch (e) {
    results.push(`[Coding Plan] FAIL — ${e}`)
  }

  // Test 2: Balance
  try {
    const r = await invoke('query_balance', { apiKey: apiKey.value, endpoint: endpoint.value })
    results.push(`[Balance] OK — ${JSON.stringify(r)}`)
  } catch (e) {
    results.push(`[Balance] FAIL — ${e}`)
  }

  // Test 3: Token count
  try {
    const r = await invoke('count_tokens', { apiKey: apiKey.value, endpoint: endpoint.value, text: '你好世界', model: 'glm-4-flash' })
    results.push(`[Token Count] OK — ${JSON.stringify(r)}`)
  } catch (e) {
    results.push(`[Token Count] FAIL — ${e}`)
  }

  const hasOk = results.some(r => r.includes('OK'))
  debugResult.value = {
    ok: hasOk,
    msg: results.join('\n'),
  }
  debugLoading.value = false
}

async function openDevtools() {
  try {
    await invoke('open_devtools')
  } catch (e) {
    alert('DevTools 仅在开发模式下可用')
  }
}

async function loadAppInfo() {
  try {
    const info = await invoke<Record<string, string>>('get_app_info')
    appInfo.value = JSON.stringify(info, null, 2)
  } catch (e) {
    appInfo.value = `Error: ${e}`
  }
}

function clearData() {
  if (confirm('确定要清除所有本地数据吗？（包括 API Key 和设置）')) {
    localStorage.clear()
    apiKey.value = ''
    endpoint.value = 'https://open.bigmodel.cn'
    debugResult.value = null
    appInfo.value = ''
  }
}

watch([apiKey, endpoint], () => {
  saved.value = false
})
</script>

<template>
  <div class="settings">
    <h2 class="page-title">设置</h2>

    <SettingCard title="API Key" description="用于接口鉴权，将保存在本地" icon-variant="accent">
      <template #icon>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/>
        </svg>
      </template>
      <div class="input-group">
        <input
          :type="showKey ? 'text' : 'password'"
          :value="apiKey"
          placeholder="输入 API Key (格式: xxxx.xxxxxxxx)"
          class="input-field"
          @input="apiKey = ($event.target as HTMLInputElement).value"
        />
        <ToggleBtn v-model="showKey" />
      </div>
      <div v-if="apiKey" class="key-preview">{{ maskKey(apiKey) }}</div>
    </SettingCard>

    <SettingCard title="API 端点" description="选择服务区域" icon-variant="success">
      <template #icon>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
        </svg>
      </template>
      <div class="endpoint-options">
        <label
          v-for="ep in endpoints"
          :key="ep.value"
          :class="['endpoint-option', { selected: endpoint === ep.value }]"
        >
          <input type="radio" :value="ep.value" v-model="endpoint" class="radio-hidden" />
          <div class="radio-dot"></div>
          <span>{{ ep.label }}</span>
        </label>
      </div>
    </SettingCard>

    <SettingCard title="自动刷新" description="余额查询页面定时自动刷新数据" icon-variant="accent">
      <template #icon>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M18.52 13.45a8 8 0 0 1-11.06 5.56"/><path d="M5.48 10.55a8 8 0 0 1 11.06-5.56"/>
          <polyline points="15 2 18.54 5.46 15.01 8.99"/><polyline points="9 22 5.46 18.54 8.99 15.01"/>
        </svg>
      </template>
      <template #action>
        <ToggleSwitch v-model="autoRefresh" />
      </template>
      <div v-if="autoRefresh" class="refresh-options">
        <IntervalSelector
          v-model="refreshInterval"
          :options="[10, 30, 60, 120, 300]"
          label="刷新间隔"
          :applied-value="appliedInterval"
        />
        <div class="refresh-status">
          <div class="cache-bar-track">
            <div
              class="cache-bar-fill"
              :style="{ width: getRemainSec() > 0 ? (getRemainSec() / appliedInterval * 100) + '%' : '0%' }"
            ></div>
          </div>
          <span class="cache-text">{{ getRemainSec() > 0 ? `缓存剩余 ${getRemainSec()}s` : '刷新中...' }}</span>
        </div>
      </div>
    </SettingCard>

    <button class="save-btn" @click="save">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="20 6 9 17 4 12"/>
      </svg>
      {{ saved ? '已保存' : '保存设置' }}
    </button>

    <SettingCard title="开机启动" description="系统登录时自动启动 ZhipuKit" icon-variant="success">
      <template #icon>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/>
        </svg>
      </template>
      <template #action>
        <ToggleSwitch :model-value="autoStart" @update:model-value="toggleAutoStart" />
      </template>
    </SettingCard>

    <SettingCard title="调试工具" description="测试 API 连通性" icon-variant="accent">
      <template #icon>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>
        </svg>
      </template>
      <template #action>
        <button class="debug-btn" :disabled="debugLoading || !apiKey" @click="runDebug">
          <span v-if="debugLoading" class="spinner-sm"></span>
          <span v-else>运行测试</span>
        </button>
      </template>
      <div v-if="debugResult" :class="['debug-output', { ok: debugResult.ok, fail: !debugResult.ok }]">
        <pre>{{ debugResult.msg }}</pre>
      </div>
    </SettingCard>

    <SettingCard title="开发者工具" description="调试与诊断" icon-variant="purple">
      <template #icon>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>
        </svg>
      </template>

      <div class="dev-actions">
        <button class="dev-btn" @click="openDevtools">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>
          </svg>
          打开 DevTools
        </button>
        <button class="dev-btn" @click="loadAppInfo">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/>
          </svg>
          应用信息
        </button>
        <button class="dev-btn warn" @click="clearData">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
          </svg>
          清除数据
        </button>
      </div>

      <div v-if="appInfo" class="debug-output ok" style="margin-top: 12px;">
        <pre>{{ appInfo }}</pre>
      </div>
    </SettingCard>

    <SettingCard title="关于" :description="`ZhipuKit v${appVersion}`" icon-variant="warning">
      <template #icon>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/>
        </svg>
      </template>
      <a href="https://github.com/1x1-lab/zhipukit" @click.prevent="invoke('open_url', { url: 'https://github.com/1x1-lab/zhipukit' })" class="repo-link">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0 0 24 12c0-6.63-5.37-12-12-12z"/></svg>
        <span>1x1-lab/zhipukit</span>
      </a>
    </SettingCard>
  </div>
</template>

<style scoped>
.repo-link {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--accent);
  text-decoration: none;
  transition: opacity 0.15s;
}

.repo-link:hover {
  opacity: 0.8;
}

.settings {
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

.input-group {
  display: flex;
  gap: 8px;
  min-width: 0;
}

.input-field {
  flex: 1;
  min-width: 0;
}

.key-preview {
  margin-top: 8px;
  font-size: 12px;
  color: var(--text-secondary);
  font-family: ui-monospace, monospace;
}

.endpoint-options {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.endpoint-option {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-xs);
  cursor: pointer;
  transition: all 0.15s;
  font-size: 13px;
  color: var(--text);
}

.endpoint-option:hover {
  border-color: var(--text-secondary);
}

.endpoint-option.selected {
  border-color: var(--accent);
  background: var(--accent-light);
}

.radio-hidden {
  display: none;
}

.radio-dot {
  width: 16px;
  height: 16px;
  border: 2px solid var(--border);
  border-radius: 50%;
  flex-shrink: 0;
  transition: all 0.15s;
  position: relative;
}

.endpoint-option.selected .radio-dot {
  border-color: var(--accent);
}

.endpoint-option.selected .radio-dot::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 2px;
  width: 8px;
  height: 8px;
  background: var(--accent);
  border-radius: 50%;
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

.refresh-options {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.refresh-status {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cache-bar-track {
  flex: 1;
  height: 4px;
  background: var(--border);
  border-radius: 2px;
  overflow: hidden;
}

.cache-bar-fill {
  height: 100%;
  background: var(--accent);
  border-radius: 2px;
  transition: width 1s linear;
}

.cache-text {
  font-size: 11px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  min-width: 72px;
  text-align: right;
}

.dev-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

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

.dev-btn.warn {
  color: var(--danger);
  border-color: var(--danger);
  opacity: 0.7;
}

.dev-btn.warn:hover {
  opacity: 1;
  background: var(--danger-light);
}

.card-header .debug-btn {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 16px;
  background: var(--accent-gradient);
  color: #fff;
  border-radius: var(--radius-xs);
  font-size: 12px;
  font-weight: 600;
  transition: opacity 0.2s;
  white-space: nowrap;
}

.card-header .debug-btn:hover:not(:disabled) { opacity: 0.9; }
.card-header .debug-btn:disabled { opacity: 0.4; cursor: not-allowed; }

.spinner-sm {
  width: 12px; height: 12px;
  border: 2px solid rgba(255,255,255,0.3);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }

.debug-output {
  margin-top: 12px;
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
</style>
