import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useToast } from './useToast'

export interface BalanceInfo {
  balance: number
  recharge_amount: number
  give_amount: number
  total_spend_amount: number
  frozen_balance: number
  available_balance: number
}

export interface CodingPlanInfo {
  level: string
  hour5_percentage: number
  hour5_next_reset: number
  weekly_percentage: number
  weekly_next_reset: number
  mcp_total: number
  mcp_used: number
  mcp_remaining: number
  mcp_next_reset: number
}

interface CacheEntry<T> {
  timestamp: number
  data: T
}

function getCacheTTL(): number {
  const val = Number(localStorage.getItem('zhipu_cache_duration'))
  return val > 0 ? val * 1000 : 60_000
}
const KEY_BALANCE = 'zhipu_cache_balance'
const KEY_PLAN = 'zhipu_cache_plan'
const KEY_LAST_REFRESH = 'zhipu_last_refresh'

const balance = ref<BalanceInfo | null>(null)
const codingPlan = ref<CodingPlanInfo | null>(null)
const loading = ref(false)

const toast = useToast()

function writeCache(data: { balance?: BalanceInfo; plan?: CodingPlanInfo }) {
  const now = Date.now()
  if (data.balance) {
    localStorage.setItem(KEY_BALANCE, JSON.stringify({ timestamp: now, data: data.balance }))
    balance.value = data.balance
  }
  if (data.plan) {
    localStorage.setItem(KEY_PLAN, JSON.stringify({ timestamp: now, data: data.plan }))
    codingPlan.value = data.plan
  }
  localStorage.setItem(KEY_LAST_REFRESH, String(now))
}

function syncTrayData() {
  invoke('update_tray_data', {
    balance: balance.value,
    codingPlan: codingPlan.value,
  }).catch(() => {})
}

/** 真正查询（调 Rust invoke），结果写入缓存 */
export async function fetchFresh(apiKey: string, endpoint: string) {
  if (!apiKey) {
    toast.showWarning('请先在设置中配置 API Key')
    return
  }
  if (loading.value) return

  loading.value = true

  const errors: string[] = []
  const results = await Promise.allSettled([
    invoke<BalanceInfo>('query_balance', { apiKey, endpoint }),
    invoke<CodingPlanInfo>('query_coding_plan', { apiKey, endpoint }),
  ])

  const bResult = results[0]
  const pResult = results[1]

  let newBalance: BalanceInfo | undefined
  let newPlan: CodingPlanInfo | undefined

  if (bResult.status === 'fulfilled') {
    newBalance = bResult.value
  } else {
    errors.push(`${bResult.reason}`)
  }

  if (pResult.status === 'fulfilled') {
    newPlan = pResult.value
  } else {
    errors.push(`${pResult.reason}`)
  }

  if (newBalance || newPlan) {
    writeCache({ balance: newBalance, plan: newPlan })
    syncTrayData()
  } else {
    toast.showError(errors.join('；'))
  }

  loading.value = false
}

/** 读缓存：<= 1 分钟直接返回，否则调 fetchFresh */
export async function fetchCached(apiKey: string, endpoint: string) {
  try {
    const rawBalance = localStorage.getItem(KEY_BALANCE)
    const rawPlan = localStorage.getItem(KEY_PLAN)
    const now = Date.now()

    if (rawBalance && rawPlan) {
      const cachedBalance: CacheEntry<BalanceInfo> = JSON.parse(rawBalance)
      const cachedPlan: CacheEntry<CodingPlanInfo> = JSON.parse(rawPlan)

      const ttl = getCacheTTL()
      if (now - cachedBalance.timestamp < ttl && now - cachedPlan.timestamp < ttl) {
        balance.value = cachedBalance.data
        codingPlan.value = cachedPlan.data
        return
      }
    }
  } catch {
    // cache corrupt, fall through to fetchFresh
  }

  await fetchFresh(apiKey, endpoint)
}

/** 同步恢复缓存数据到 refs（不检查 TTL，用于组件挂载时立即显示） */
export function restoreFromCache() {
  try {
    const rawBalance = localStorage.getItem(KEY_BALANCE)
    const rawPlan = localStorage.getItem(KEY_PLAN)

    if (rawBalance) {
      balance.value = (JSON.parse(rawBalance) as CacheEntry<BalanceInfo>).data
    }
    if (rawPlan) {
      codingPlan.value = (JSON.parse(rawPlan) as CacheEntry<CodingPlanInfo>).data
    }
  } catch {
    // ignore
  }
}

/** 注册 Rust 后端事件监听，返回 unsubscribe 函数 */
export async function setupListeners(): Promise<() => void> {
  const unlistenBalance = await listen<Record<string, unknown>>('balance-update', (e) => {
    const d = e.payload
    const b: BalanceInfo = {
      balance: (d.balance as number) ?? 0,
      recharge_amount: (d.rechargeAmount as number) ?? 0,
      give_amount: (d.giveAmount as number) ?? 0,
      total_spend_amount: (d.totalSpendAmount as number) ?? 0,
      frozen_balance: (d.frozenBalance as number) ?? 0,
      available_balance: (d.availableBalance as number) ?? 0,
    }
    writeCache({ balance: b })
    syncTrayData()
  })

  const unlistenPlan = await listen<Record<string, unknown>>('plan-update', (e) => {
    const data = e.payload
    const limits = (data.limits as Array<Record<string, unknown>>) ?? []
    const level = (data.level as string) ?? 'unknown'

    let hour5_percentage = 0, hour5_next_reset = 0
    let weekly_percentage = 0, weekly_next_reset = 0
    let mcp_total = 0, mcp_used = 0, mcp_remaining = 0, mcp_next_reset = 0
    let tokensIdx = 0

    for (const lim of limits) {
      const t = (lim.type as string) ?? ''
      const pct = (lim.percentage as number) ?? 0
      const nrt = (lim.nextResetTime as number) ?? 0

      if (t === 'TIME_LIMIT') {
        mcp_total = (lim.usage as number) ?? 0
        mcp_used = (lim.currentValue as number) ?? 0
        mcp_remaining = (lim.remaining as number) ?? 0
        mcp_next_reset = nrt
      } else if (t === 'TOKENS_LIMIT') {
        if (tokensIdx === 0) { hour5_percentage = pct; hour5_next_reset = nrt }
        else { weekly_percentage = pct; weekly_next_reset = nrt }
        tokensIdx++
      }
    }

    const plan: CodingPlanInfo = {
      level, hour5_percentage, hour5_next_reset,
      weekly_percentage, weekly_next_reset,
      mcp_total, mcp_used, mcp_remaining, mcp_next_reset,
    }
    writeCache({ plan })
    syncTrayData()
  })

  return () => {
    unlistenBalance()
    unlistenPlan()
  }
}

export function useBalanceCache() {
  return {
    balance,
    codingPlan,
    loading,
    fetchFresh,
    fetchCached,
    restoreFromCache,
    setupListeners,
  }
}
