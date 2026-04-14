<script setup lang="ts">
import { ref, watch, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useBalanceCache } from '../composables/useBalanceCache'

const props = defineProps<{ apiKey: string; endpoint: string }>()

const { balance, codingPlan, loading, fetchFresh, fetchCached, restoreFromCache, setupListeners } = useBalanceCache()

// 自动刷新：启动/停止 Rust 端定时任务
const autoRefresh = ref(localStorage.getItem('zhipu_auto_refresh') === 'true')
const refreshInterval = ref(Number(localStorage.getItem('zhipu_refresh_interval') || '30'))

function startRustAutoRefresh() {
  if (!props.apiKey || !autoRefresh.value) return
  invoke('start_auto_refresh', {
    apiKey: props.apiKey,
    endpoint: props.endpoint,
    intervalSecs: refreshInterval.value,
  }).catch(() => {})
}

// 缓存有效期内不启动 Rust 定时器（避免立即查询），到期后再启动
let autoRefreshTimer: ReturnType<typeof setTimeout> | null = null

function scheduleAutoRefresh() {
  if (!props.apiKey || !autoRefresh.value) return
  const lastRefresh = Number(localStorage.getItem('zhipu_last_refresh') || '0')
  const remaining = Math.max(0, 60_000 - (Date.now() - lastRefresh))
  autoRefreshTimer = setTimeout(startRustAutoRefresh, remaining)
}

// 检查设置变化
const settingsCheck = setInterval(() => {
  const ar = localStorage.getItem('zhipu_auto_refresh') === 'true'
  const ri = Number(localStorage.getItem('zhipu_refresh_interval') || '30')
  if (ar !== autoRefresh.value || ri !== refreshInterval.value) {
    autoRefresh.value = ar
    refreshInterval.value = ri
    if (ar) {
      startRustAutoRefresh()
    } else {
      invoke('stop_auto_refresh').catch(() => {})
    }
  }
}, 3000)

let unsubListeners: (() => void) | null = null

onBeforeUnmount(() => {
  clearInterval(settingsCheck)
  if (autoRefreshTimer) clearTimeout(autoRefreshTimer)
  if (unsubListeners) unsubListeners()
})

function fmt(v: number): string {
  return parseFloat(v.toFixed(4)).toString()
}

function getLevelLabel(level: string): string {
  const map: Record<string, string> = { free: '免费版', starter: '入门版', pro: '专业版', max: '旗舰版' }
  return map[level] || level
}

function getLevelStyle(level: string): string {
  if (level === 'max') return 'level-max'
  return ''
}

function getBarColor(pct: number): string {
  if (pct >= 90) return 'var(--danger)'
  if (pct >= 70) return 'var(--warning)'
  return 'var(--accent)'
}

// 倒计时
const now = ref(Date.now())
setInterval(() => { now.value = Date.now() }, 1000)

function formatCountdown(ms: number): string {
  if (ms <= 0) return '即将重置'
  const totalSec = Math.floor(ms / 1000)
  const d = Math.floor(totalSec / 86400)
  const h = Math.floor((totalSec % 86400) / 3600)
  const m = Math.floor((totalSec % 3600) / 60)
  const s = totalSec % 60
  if (d > 0) return `${d}天${h}时${m}分`
  if (h > 0) return `${h}时${m}分${s}秒`
  if (m > 0) return `${m}分${s}秒`
  return `${s}秒`
}

function getCountdown(resetTime: number): string {
  if (!resetTime) return ''
  return formatCountdown(resetTime - now.value)
}

// 自动查询
watch(() => props.apiKey, (key) => {
  if (key) {
    restoreFromCache()
    fetchCached(props.apiKey, props.endpoint)
    setupListeners().then(unsub => { unsubListeners = unsub })
    if (autoRefresh.value) scheduleAutoRefresh()
  }
}, { immediate: true })
</script>

<template>
  <div class="balance-query">
    <div class="page-header">
      <h2 class="page-title">余额查询</h2>
      <div class="header-actions">
        <button class="query-btn-sm" :disabled="loading || !apiKey" @click="fetchFresh(apiKey, endpoint)">
          <span v-if="loading" class="spinner-sm"></span>
          <svg v-else width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
          </svg>
          <span>查询</span>
        </button>
      </div>
    </div>

    <div v-if="!apiKey" class="hint-banner">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/>
      </svg>
      请先前往 <strong>设置</strong> 页面配置 API Key
    </div>

    <!-- Coding Plan -->
    <div v-if="codingPlan" class="section">
      <div class="section-head">
        <h3 class="section-title">Coding Plan</h3>
        <span :class="['level-badge', getLevelStyle(codingPlan.level)]">{{ getLevelLabel(codingPlan.level) }}</span>
      </div>
      <div class="quota-list">
        <div class="quota-item">
          <div class="quota-row">
            <span class="quota-name">5 小时额度</span>
            <span class="quota-value" :style="{ color: getBarColor(codingPlan.hour5_percentage) }">
              {{ 100 - codingPlan.hour5_percentage }}%
            </span>
          </div>
          <div class="bar-track">
            <div class="bar-fill" :style="{ width: codingPlan.hour5_percentage + '%', background: getBarColor(codingPlan.hour5_percentage) }"></div>
          </div>
          <div class="quota-meta">
            <span>已用 {{ codingPlan.hour5_percentage }}%</span>
            <span v-if="codingPlan.hour5_next_reset" class="countdown">{{ getCountdown(codingPlan.hour5_next_reset) }}</span>
          </div>
        </div>

        <div v-if="codingPlan.weekly_percentage > 0 || codingPlan.weekly_next_reset > 0" class="quota-item">
          <div class="quota-row">
            <span class="quota-name">每周额度</span>
            <span class="quota-value" :style="{ color: getBarColor(codingPlan.weekly_percentage) }">
              {{ 100 - codingPlan.weekly_percentage }}%
            </span>
          </div>
          <div class="bar-track">
            <div class="bar-fill" :style="{ width: codingPlan.weekly_percentage + '%', background: getBarColor(codingPlan.weekly_percentage) }"></div>
          </div>
          <div class="quota-meta">
            <span>已用 {{ codingPlan.weekly_percentage }}%</span>
            <span v-if="codingPlan.weekly_next_reset" class="countdown">{{ getCountdown(codingPlan.weekly_next_reset) }}</span>
          </div>
        </div>

        <div v-if="codingPlan.mcp_total > 0" class="quota-item">
          <div class="quota-row">
            <span class="quota-name">MCP 月度调用</span>
            <span class="quota-value" :style="{ color: getBarColor(codingPlan.mcp_total > 0 ? Math.round(codingPlan.mcp_used / codingPlan.mcp_total * 100) : 0) }">
              {{ codingPlan.mcp_remaining }}次
            </span>
          </div>
          <div class="bar-track">
            <div class="bar-fill" :style="{ width: (codingPlan.mcp_total > 0 ? codingPlan.mcp_used / codingPlan.mcp_total * 100 : 0) + '%', background: getBarColor(codingPlan.mcp_total > 0 ? Math.round(codingPlan.mcp_used / codingPlan.mcp_total * 100) : 0) }"></div>
          </div>
          <div class="quota-meta">
            <span>已用 {{ codingPlan.mcp_used }} / {{ codingPlan.mcp_total }} 次</span>
            <span v-if="codingPlan.mcp_next_reset" class="countdown">{{ getCountdown(codingPlan.mcp_next_reset) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Balance -->
    <div v-if="balance" class="section">
      <h3 class="section-title">账户余额</h3>

      <div class="balance-hero">
        <div class="hero-label">可用余额</div>
        <div class="hero-value">¥{{ fmt(balance.available_balance) }}</div>
      </div>

      <div class="balance-rows">
        <div class="balance-row">
          <div class="row-icon blue">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
          </div>
          <span class="row-label">当前余额</span>
          <span class="row-value">¥{{ fmt(balance.balance) }}</span>
        </div>
        <div class="balance-row">
          <div class="row-icon green">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="23 6 13.5 15.5 8.5 10.5 1 18"/><polyline points="17 6 23 6 23 12"/></svg>
          </div>
          <span class="row-label">累计充值</span>
          <span class="row-value positive">¥{{ fmt(balance.recharge_amount) }}</span>
        </div>
        <div class="balance-row">
          <div class="row-icon green">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>
          </div>
          <span class="row-label">赠送金额</span>
          <span class="row-value positive">¥{{ fmt(balance.give_amount) }}</span>
        </div>
        <div class="balance-row">
          <div class="row-icon amber">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="23 18 13.5 8.5 8.5 13.5 1 6"/><polyline points="17 18 23 18 23 12"/></svg>
          </div>
          <span class="row-label">累计消费</span>
          <span class="row-value warn">¥{{ fmt(balance.total_spend_amount) }}</span>
        </div>
        <div class="balance-row last">
          <div class="row-icon red">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
          </div>
          <span class="row-label">冻结余额</span>
          <span class="row-value danger">¥{{ fmt(balance.frozen_balance) }}</span>
        </div>
      </div>
    </div>

    <div v-if="!balance && !codingPlan && !loading" class="empty">
      <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" opacity="0.2">
        <rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/>
      </svg>
      <p>点击查询按钮获取数据</p>
    </div>
  </div>
</template>

<style scoped>
.balance-query {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.page-title {
  font-size: 22px;
  font-weight: 700;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.query-btn-sm {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 6px 14px;
  background: var(--accent-gradient);
  color: #fff;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  transition: opacity 0.2s;
}

.query-btn-sm:hover:not(:disabled) { opacity: 0.9; }
.query-btn-sm:disabled { opacity: 0.4; cursor: not-allowed; }

.spinner-sm {
  width: 12px; height: 12px;
  border: 2px solid rgba(255,255,255,0.3);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.hint-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  background: var(--accent-light);
  border: 1px solid var(--accent);
  border-radius: var(--radius-sm);
  font-size: 13px;
  color: var(--accent);
}

.section {
  padding-top: 20px;
}

.section-head {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 16px;
}

.section-title {
  font-size: 15px;
  font-weight: 600;
}

.level-badge {
  padding: 2px 10px;
  background: var(--accent-light);
  color: var(--accent);
  border-radius: 20px;
  font-size: 12px;
  font-weight: 600;
}

.level-badge.level-max {
  background: linear-gradient(135deg, #b8860b 0%, #ffd700 25%, #fffacd 50%, #ffd700 75%, #b8860b 100%);
  background-size: 200% 100%;
  animation: goldShimmer 3s ease-in-out infinite;
  color: #5c3d00;
  text-shadow: 0 1px 2px rgba(255, 215, 0, 0.6);
  box-shadow:
    0 0 12px rgba(255, 215, 0, 0.4),
    inset 0 1px 0 rgba(255, 255, 255, 0.4);
  border: 1px solid rgba(255, 215, 0, 0.6);
}

@keyframes goldShimmer {
  0%, 100% { background-position: 100% 0; }
  50% { background-position: 0 0; }
}

.quota-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.quota-item {
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border);
}
.quota-item:last-child { border-bottom: none; padding-bottom: 0; }

.quota-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.quota-name { font-size: 13px; font-weight: 500; }
.quota-value { font-size: 16px; font-weight: 700; font-variant-numeric: tabular-nums; }

.bar-track {
  height: 6px;
  background: var(--border);
  border-radius: 3px;
  overflow: hidden;
  margin-bottom: 6px;
}

.bar-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.6s ease, background 0.3s;
}

.quota-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11px;
  color: var(--text-secondary);
}

.countdown {
  font-variant-numeric: tabular-nums;
  color: var(--text-secondary);
  font-size: 11px;
}

.balance-hero {
  text-align: center;
  padding: 24px 0 20px;
  margin-bottom: 16px;
  border-bottom: 1px solid var(--border);
}

.hero-label {
  font-size: 12px;
  color: var(--text-secondary);
  margin-bottom: 4px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.hero-value {
  font-size: 32px;
  font-weight: 800;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}

.balance-rows {
  display: flex;
  flex-direction: column;
}

.balance-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 0;
  border-bottom: 1px solid var(--border);
}

.balance-row:last-child,
.balance-row.last {
  border-bottom: none;
}

.row-icon {
  width: 28px;
  height: 28px;
  border-radius: 7px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.row-icon.blue { background: var(--accent-light); color: var(--accent); }
.row-icon.green { background: var(--success-light); color: var(--success); }
.row-icon.amber { background: var(--warning-light); color: var(--warning); }
.row-icon.red { background: var(--danger-light); color: var(--danger); }

.row-label {
  flex: 1;
  font-size: 13px;
  color: var(--text-secondary);
}

.row-value {
  font-size: 14px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--text);
}

.row-value.positive { color: var(--success); }
.row-value.warn { color: var(--warning); }
.row-value.danger { color: var(--danger); }

.empty {
  text-align: center;
  padding: 48px 20px;
  color: var(--text-secondary);
}
.empty p { margin-top: 12px; font-size: 13px; }
</style>
