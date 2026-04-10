<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import TokenCalculator from './components/TokenCalculator.vue'
import BalanceQuery from './components/BalanceQuery.vue'
import SettingsView from './components/SettingsView.vue'
import ClaudeConfig from './components/ClaudeConfig.vue'

type Tab = 'balance' | 'calculator' | 'claude' | 'settings'

const activeTab = ref<Tab>('balance')
const transitionName = ref('fade')

const tabOrder: Tab[] = ['balance', 'calculator', 'claude', 'settings']

function switchTab(tab: Tab) {
  const from = tabOrder.indexOf(activeTab.value)
  const to = tabOrder.indexOf(tab)
  transitionName.value = to > from ? 'slide-left' : 'slide-right'
  activeTab.value = tab
}
const apiKey = ref('')
const endpoint = ref('https://open.bigmodel.cn')

// 关闭确认弹窗
const showCloseDialog = ref(false)
let unlistenClose: UnlistenFn | null = null

onMounted(async () => {
  apiKey.value = localStorage.getItem('zhipu_api_key') || ''
  endpoint.value = localStorage.getItem('zhipu_endpoint') || 'https://open.bigmodel.cn'

  unlistenClose = await listen('confirm-minimize-to-tray', () => {
    showCloseDialog.value = true
  })
})

onBeforeUnmount(() => {
  if (unlistenClose) unlistenClose()
})

function handleMinimize() {
  showCloseDialog.value = false
  invoke('confirm_minimize_to_tray', { minimize: true })
}

function handleExit() {
  showCloseDialog.value = false
  invoke('exit_app')
}

function handleDrag(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (target.closest('button, input, textarea, select, a, label, [data-no-drag]')) return
  // 如果点击在 page-content 内部，但不在 page-content-drag 拖拽区域，则不触发拖拽
  if (target.closest('.page-content') && !target.closest('.page-content-drag')) return
  invoke('start_window_drag').catch(() => {})
}
</script>

<template>
  <!-- 关闭确认弹窗 -->
  <div v-if="showCloseDialog" class="dialog-overlay" @click.self="showCloseDialog = false">
    <div class="dialog-box">
      <h3>关闭窗口</h3>
      <p>是否最小化到系统托盘继续运行？</p>
      <div class="dialog-actions">
        <button class="dialog-btn secondary" @click="handleExit">直接退出</button>
        <button class="dialog-btn primary" @click="handleMinimize">最小化到托盘</button>
      </div>
    </div>
  </div>

  <aside class="sidebar" @mousedown="handleDrag">
    <div class="sidebar-brand">
      <svg class="brand-icon" width="28" height="28" viewBox="0 0 24 24" fill="#3859FF" fill-rule="evenodd">
        <path d="M11.991 23.503a.24.24 0 00-.244.248.24.24 0 00.244.249.24.24 0 00.245-.249.24.24 0 00-.22-.247l-.025-.001zM9.671 5.365a1.697 1.697 0 011.099 2.132l-.071.172-.016.04-.018.054c-.07.16-.104.32-.104.498-.035.71.47 1.279 1.186 1.314h.366c1.309.053 2.338 1.173 2.286 2.523-.052 1.332-1.152 2.38-2.478 2.327h-.174c-.715.018-1.274.64-1.239 1.368 0 .124.018.23.053.337.209.373.54.658.96.8.75.23 1.517-.125 1.9-.782l.018-.035c.402-.64 1.17-.96 1.92-.711.854.284 1.378 1.226 1.099 2.167a1.661 1.661 0 01-2.077 1.102 1.711 1.711 0 01-.907-.711l-.017-.035c-.2-.323-.463-.58-.851-.711l-.056-.018a1.646 1.646 0 00-1.954.746 1.66 1.66 0 01-1.065.764 1.677 1.677 0 01-1.989-1.279c-.209-.906.332-1.83 1.257-2.043a1.51 1.51 0 01.296-.035h.018c.68-.071 1.151-.622 1.116-1.333a1.307 1.307 0 00-.227-.693 2.515 2.515 0 01-.366-1.403 2.39 2.39 0 01.366-1.208c.14-.195.21-.444.227-.693.018-.71-.506-1.261-1.186-1.332l-.07-.018a1.43 1.43 0 01-.299-.07l-.05-.019a1.7 1.7 0 01-1.047-2.114 1.68 1.68 0 012.094-1.101zm-5.575 10.11c.26-.264.639-.367.994-.27.355.096.633.379.728.74.095.362-.007.748-.267 1.013-.402.41-1.053.41-1.455 0a1.062 1.062 0 010-1.482zm14.845-.294c.359-.09.738.024.992.297.254.274.344.665.237 1.025-.107.36-.396.634-.756.718-.551.128-1.1-.22-1.23-.781a1.05 1.05 0 01.757-1.26zm-.064-4.39c.314.32.49.753.49 1.206 0 .452-.176.886-.49 1.206-.315.32-.74.5-1.185.5-.444 0-.87-.18-1.184-.5a1.727 1.727 0 010-2.412 1.654 1.654 0 012.369 0zm-11.243.163c.364.484.447 1.128.218 1.691a1.665 1.665 0 01-2.188.923c-.855-.36-1.26-1.358-.907-2.228a1.68 1.68 0 011.33-1.038c.593-.08 1.183.169 1.547.652zm11.545-4.221c.368 0 .708.2.892.524.184.324.184.724 0 1.048a1.026 1.026 0 01-.892.524c-.568 0-1.03-.47-1.03-1.048 0-.579.462-1.048 1.03-1.048zm-14.358 0c.368 0 .707.2.891.524.184.324.184.724 0 1.048a1.026 1.026 0 01-.891.524c-.569 0-1.03-.47-1.03-1.048 0-.579.461-1.048 1.03-1.048zm10.031-1.475c.925 0 1.675.764 1.675 1.706s-.75 1.705-1.675 1.705-1.674-.763-1.674-1.705c0-.942.75-1.706 1.674-1.706zm-2.626-.684c.362-.082.653-.356.761-.718a1.062 1.062 0 00-.238-1.028 1.017 1.017 0 00-.996-.294c-.547.14-.881.7-.752 1.257.13.558.675.907 1.225.783zm0 16.876c.359-.087.644-.36.75-.72a1.062 1.062 0 00-.237-1.019 1.018 1.018 0 00-.985-.301 1.037 1.037 0 00-.762.717c-.108.361-.017.754.239 1.028.245.263.606.377.953.305l.043-.01zM17.19 3.5a.631.631 0 00.628-.64c0-.355-.279-.64-.628-.64a.631.631 0 00-.628.64c0 .355.28.64.628.64zm-10.38 0a.631.631 0 00.628-.64c0-.355-.28-.64-.628-.64a.631.631 0 00-.628.64c0 .355.279.64.628.64zm-5.182 7.852a.631.631 0 00-.628.64c0 .354.28.639.628.639a.63.63 0 00.627-.606l.001-.034a.62.62 0 00-.628-.64zm5.182 9.13a.631.631 0 00-.628.64c0 .355.279.64.628.64a.631.631 0 00.628-.64c0-.355-.28-.64-.628-.64zm10.38.018a.631.631 0 00-.628.64c0 .355.28.64.628.64a.631.631 0 00.628-.64c0-.355-.279-.64-.628-.64zm5.182-9.148a.631.631 0 00-.628.64c0 .354.279.639.628.639a.631.631 0 00.628-.64c0-.355-.28-.64-.628-.64zm-.384-4.992a.24.24 0 00.244-.249.24.24 0 00-.244-.249.24.24 0 00-.244.249c0 .142.122.249.244.249zM11.991.497a.24.24 0 00.245-.248A.24.24 0 0011.99 0a.24.24 0 00-.244.249c0 .133.108.236.223.247l.021.001zM2.011 6.36a.24.24 0 00.245-.249.24.24 0 00-.244-.249.24.24 0 00-.244.249.24.24 0 00.244.249zm0 11.263a.24.24 0 00-.243.248.24.24 0 00.244.249.24.24 0 00.244-.249.252.252 0 00-.244-.248zm19.995-.018a.24.24 0 00-.245.248.24.24 0 00.245.25.24.24 0 00.244-.25.252.252 0 00-.244-.248z"/>
      </svg>
      <span class="brand-text">ZhipuKit</span>
    </div>

    <nav class="sidebar-nav">
      <button
        :class="['nav-item', { active: activeTab === 'balance' }]"
        @click="switchTab('balance')"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <rect x="2" y="5" width="20" height="14" rx="2"/>
          <path d="M2 10h20"/>
        </svg>
        <span>余额查询</span>
      </button>

      <button
        :class="['nav-item', { active: activeTab === 'calculator' }]"
        @click="switchTab('calculator')"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M4 7V4h16v3"/><path d="M9 20h6"/><path d="M12 4v16"/>
        </svg>
        <span>Token计算</span>
      </button>

      <button
        :class="['nav-item', { active: activeTab === 'claude' }]"
        @click="switchTab('claude')"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="4 17 10 11 4 5"/>
          <line x1="12" y1="19" x2="20" y2="19"/>
        </svg>
        <span>Claude Code</span>
      </button>
    </nav>

    <div class="sidebar-bottom">
      <button
        :class="['nav-item', { active: activeTab === 'settings' }]"
        @click="switchTab('settings')"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
        <span>设置</span>
      </button>
    </div>
  </aside>

  <main class="main-area" @mousedown="handleDrag">
    <div class="page-content">
      <div class="page-content-drag" @mousedown="handleDrag"/>
      <Transition :name="transitionName" mode="out-in">
        <BalanceQuery v-if="activeTab === 'balance'" key="balance" :api-key="apiKey" :endpoint="endpoint" />
        <TokenCalculator v-else-if="activeTab === 'calculator'" key="calculator" :api-key="apiKey" :endpoint="endpoint" />
        <ClaudeConfig v-else-if="activeTab === 'claude'" key="claude" :zhipu-api-key="apiKey" />
        <SettingsView v-else key="settings" v-model:api-key="apiKey" v-model:endpoint="endpoint" />
      </Transition>
    </div>
  </main>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.dialog-box {
  background: var(--bg-card);
  border-radius: 12px;
  padding: 24px 28px;
  min-width: 320px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
}

.dialog-box h3 {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 8px;
}

.dialog-box p {
  font-size: 13px;
  color: var(--text-secondary);
  margin-bottom: 20px;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.dialog-btn {
  padding: 7px 18px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.15s;
}

.dialog-btn:hover { opacity: 0.85; }

.dialog-btn.secondary {
  background: var(--border);
  color: var(--text);
}

.dialog-btn.primary {
  background: var(--accent-gradient);
  color: #fff;
}

.sidebar {
  width: 200px;
  min-width: 200px;
  height: 100vh;
  background: var(--bg);
  display: flex;
  flex-direction: column;
  padding: 0;
  user-select: none;
  z-index: 10;
}

.sidebar-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 36px 16px 12px;
}

.brand-icon {
  flex-shrink: 0;
}

.brand-text {
  font-size: 15px;
  font-weight: 600;
  color: var(--sidebar-text-active);
}

.sidebar-nav {
  flex: 1;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sidebar-bottom {
  padding: 12px 8px 10px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 10px 12px;
  background: transparent;
  color: var(--sidebar-text);
  font-size: 13px;
  font-weight: 500;
  border-radius: 8px;
  transition: all 0.15s ease;
  text-align: left;
}

.nav-item:hover {
  background: var(--sidebar-hover);
  color: var(--sidebar-text-active);
}

.nav-item.active {
  background: var(--sidebar-active);
  color: var(--sidebar-text-active);
}

.nav-item.active svg {
  color: var(--accent);
}

.main-area {
  flex: 1;
  overflow: hidden;
  min-width: 0;
  padding: 10px;
  background: var(--bg);
}
.page-content-drag{
  width: 100%;
  height: 28px;
}
.page-content {
  max-width: 760px;
  margin: 0 auto;
  padding: 0 32px 28px;
  height: calc(100vh - 20px);
  overflow-y: auto;
  background: var(--bg-card);
  border-radius: 9px;
  box-shadow: var(--shadow-md);
}

/* Page transition animations */
.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.slide-left-enter-from {
  opacity: 0;
  transform: translateX(20px);
}
.slide-left-leave-to {
  opacity: 0;
  transform: translateX(-20px);
}

.slide-right-enter-from {
  opacity: 0;
  transform: translateX(-20px);
}
.slide-right-leave-to {
  opacity: 0;
  transform: translateX(20px);
}
</style>
