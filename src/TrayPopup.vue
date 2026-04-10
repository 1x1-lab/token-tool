<script setup lang="ts">
import { ref, nextTick, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

interface BalanceData {
  balance: number
  available_balance: number
  recharge_amount: number
  give_amount: number
  total_spend_amount: number
  frozen_balance: number
}

interface PlanData {
  hour5_percentage: number
  hour5_next_reset: number
  weekly_percentage: number
  weekly_next_reset: number
  mcp_total: number
  mcp_used: number
  mcp_remaining: number
  mcp_next_reset: number
}

const balance = ref<BalanceData | null>(null)
const plan = ref<PlanData | null>(null)

async function loadData() {
  try {
    const data = await invoke<{ balance: BalanceData | null; coding_plan: PlanData | null }>('get_tray_popup_data')
    balance.value = data.balance
    plan.value = data.coding_plan
  } catch {}
  await autoResize()
}

async function autoResize() {
  await nextTick()
  const el = document.querySelector('.tray-popup')
  if (el) {
    const rect = el.getBoundingClientRect()
    invoke('resize_popup', { width: Math.ceil(rect.width), height: Math.ceil(rect.height) })
  }
}

let unlisten: UnlistenFn | null = null

onMounted(async () => {
  await loadData()
  unlisten = await listen('popup-shown', async () => {
    await loadData()
  })
})

onBeforeUnmount(() => {
  if (unlisten) unlisten()
})

function fmt(v: number): string {
  return parseFloat(v.toFixed(4)).toString()
}

function getBarColor(pct: number): string {
  if (pct >= 90) return 'var(--danger)'
  if (pct >= 70) return 'var(--warning)'
  return 'var(--accent)'
}

function showWindow() {
  invoke('tray_show_main')
}

function quitApp() {
  invoke('exit_app')
}

// Countdown
const now = ref(Date.now())
const timer = setInterval(() => { now.value = Date.now() }, 1000)

function formatCountdown(ms: number): string {
  if (ms <= 0) return '即将重置'
  const totalSec = Math.floor(ms / 1000)
  const d = Math.floor(totalSec / 86400)
  const h = Math.floor((totalSec % 86400) / 3600)
  const m = Math.floor((totalSec % 3600) / 60)
  const s = totalSec % 60
  if (d > 0) return `${d}天${h}时`
  if (h > 0) return `${h}h${m}m`
  if (m > 0) return `${m}m${s}s`
  return `${s}s`
}

function getCountdown(resetTime: number): string {
  if (!resetTime) return ''
  return formatCountdown(resetTime - now.value)
}

onBeforeUnmount(() => {
  clearInterval(timer)
})
</script>

<template>
  <div class="tray-popup">
    <!-- Balance -->
    <div v-if="balance" class="popup-section">
      <div class="balance-row">
        <span class="label">可用余额</span>
        <span class="balance-value">¥{{ fmt(balance.available_balance) }}</span>
      </div>
    </div>

    <!-- Plan -->
    <div v-if="plan" class="popup-section">
      <div class="plan-item">
        <div class="plan-row">
          <span class="label">5小时额度</span>
          <span class="pct-value" :style="{ color: getBarColor(plan.hour5_percentage) }">{{ 100 - plan.hour5_percentage }}%</span>
        </div>
        <div class="bar">
          <div class="bar-fill" :style="{ width: plan.hour5_percentage + '%', background: getBarColor(plan.hour5_percentage) }"></div>
        </div>
        <div class="plan-meta">
          <span>已用 {{ plan.hour5_percentage }}%</span>
          <span v-if="plan.hour5_next_reset" class="countdown">{{ getCountdown(plan.hour5_next_reset) }}</span>
        </div>
      </div>

      <div v-if="plan.weekly_percentage > 0" class="plan-item">
        <div class="plan-row">
          <span class="label">每周额度</span>
          <span class="pct-value" :style="{ color: getBarColor(plan.weekly_percentage) }">{{ 100 - plan.weekly_percentage }}%</span>
        </div>
        <div class="bar">
          <div class="bar-fill" :style="{ width: plan.weekly_percentage + '%', background: getBarColor(plan.weekly_percentage) }"></div>
        </div>
        <div class="plan-meta">
          <span>已用 {{ plan.weekly_percentage }}%</span>
          <span v-if="plan.weekly_next_reset" class="countdown">{{ getCountdown(plan.weekly_next_reset) }}</span>
        </div>
      </div>

      <div v-if="plan.mcp_total > 0" class="plan-item">
        <div class="plan-row">
          <span class="label">MCP月度调用</span>
          <span class="pct-value" :style="{ color: getBarColor(plan.mcp_total > 0 ? Math.round(plan.mcp_used / plan.mcp_total * 100) : 0) }">{{ plan.mcp_remaining }}次</span>
        </div>
        <div class="bar">
          <div class="bar-fill" :style="{ width: (plan.mcp_total > 0 ? plan.mcp_used / plan.mcp_total * 100 : 0) + '%', background: getBarColor(plan.mcp_total > 0 ? Math.round(plan.mcp_used / plan.mcp_total * 100) : 0) }"></div>
        </div>
        <div class="plan-meta">
          <span>已用 {{ plan.mcp_used }} / {{ plan.mcp_total }} 次</span>
          <span v-if="plan.mcp_next_reset" class="countdown">{{ getCountdown(plan.mcp_next_reset) }}</span>
        </div>
      </div>
    </div>

    <!-- Actions -->
    <div class="popup-actions">
      <button class="action-btn" @click="showWindow">显示窗口</button>
      <button class="action-btn quit" @click="quitApp">退出</button>
    </div>
  </div>
</template>

<style scoped>
.tray-popup {
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 12px;
  background: var(--bg-card);
  border-radius: 8px;
  border: none;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  user-select: none;
  width: 200px;
}

.popup-section {
  padding: 2px 0;
}

.popup-section + .popup-section {
  border-top: 1px solid var(--border);
  margin-top: 2px;
  padding-top: 5px;
}

.balance-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 6px;
}

.balance-value {
  font-size: 13px;
  font-weight: 600;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}

.plan-item {
  padding: 3px 6px;
}

.plan-item + .plan-item {
  padding-top: 0;
}

.plan-item + .plan-item::before{
    content: '';
    display: block;
    border-top: 1px dashed var(--border);
    margin: 0 0 1px;
    /* background: var(--border); */
 }

.plan-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 3px;
}

.label {
  color: var(--text-secondary);
  font-size: 11px;
}

.pct-value {
  font-weight: 600;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.bar {
  height: 3px;
  background: var(--border);
  border-radius: 2px;
  overflow: hidden;
  margin-bottom: 2px;
}

.bar-fill {
  height: 100%;
  border-radius: 2px;
  transition: width 0.3s;
}

.plan-meta {
  display: flex;
  justify-content: space-between;
  font-size: 10px;
  color: var(--text-secondary);
}

.countdown {
  font-variant-numeric: tabular-nums;
}

.popup-actions {
  display: flex;
  gap: 6px;
  padding: 6px;
  border-top: 1px solid var(--border);
  margin-top: 2px;
}

.action-btn {
  flex: 1;
  padding: 5px 0;
  font-size: 11px;
  font-weight: 500;
  background: var(--accent-gradient);
  border: none;
  border-radius: 4px;
  color: #fff;
  cursor: pointer;
  transition: opacity 0.15s;
}

.action-btn:hover {
  opacity: 0.85;
}

.action-btn:active {
  opacity: 0.6;
}

.action-btn.quit {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border);
}

.action-btn.quit:hover {
  background: var(--danger-light);
  color: var(--danger);
  border-color: var(--danger);
}

.action-btn.quit:active {
  background: var(--danger);
  color: #fff;
  border-color: var(--danger);
}
</style>

<style>
/* 覆盖 style.css 中 #app 的全局样式，仅影响 popup 页面 */
#app {
  height: auto !important;
  flex-direction: column !important;
}
</style>
