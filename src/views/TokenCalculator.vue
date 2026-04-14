<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useToast } from '../composables/useToast'

const toast = useToast()

interface TokenCountResult {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
}

const props = defineProps<{ apiKey: string; endpoint: string }>()

const models = [
  { id: 'glm-4-plus', name: 'GLM-4-Plus', inputPrice: 50, outputPrice: 50 },
  { id: 'glm-4', name: 'GLM-4', inputPrice: 100, outputPrice: 100 },
  { id: 'glm-4-air', name: 'GLM-4-Air', inputPrice: 1, outputPrice: 1 },
  { id: 'glm-4-airx', name: 'GLM-4-AirX', inputPrice: 10, outputPrice: 10 },
  { id: 'glm-4-long', name: 'GLM-4-Long', inputPrice: 1, outputPrice: 1 },
  { id: 'glm-4-flash', name: 'GLM-4-Flash', inputPrice: 0, outputPrice: 0 },
  { id: 'glm-4.5', name: 'GLM-4.5', inputPrice: 20, outputPrice: 20 },
]

const text = ref('')
const selectedModel = ref('glm-4-flash')
const loading = ref(false)
const apiResult = ref<TokenCountResult | null>(null)

const estimatedTokens = computed(() => {
  if (!text.value) return 0
  let count = 0
  for (const char of text.value) {
    const code = char.charCodeAt(0)
    count += code > 0x7f ? 1 / 1.6 : 1 / 4
  }
  return Math.ceil(count)
})

const estimatedCost = computed(() => {
  const m = models.find(m => m.id === selectedModel.value)
  if (!m || m.inputPrice === 0) return '免费'
  return `¥${((estimatedTokens.value / 1_000_000) * m.inputPrice).toFixed(6)}`
})

const apiCost = computed(() => {
  if (!apiResult.value) return null
  const m = models.find(m => m.id === selectedModel.value)
  if (!m || m.inputPrice === 0) return '免费'
  return `¥${((apiResult.value.prompt_tokens / 1_000_000) * m.inputPrice).toFixed(6)}`
})

const charCount = computed(() => text.value.length)

async function countTokens() {
  if (!text.value.trim()) { toast.showWarning('请输入文本'); return }
  if (!props.apiKey) { toast.showWarning('请先在设置中配置 API Key'); return }

  loading.value = true
  apiResult.value = null

  try {
    apiResult.value = await invoke<TokenCountResult>('count_tokens', {
      apiKey: props.apiKey,
      endpoint: props.endpoint,
      text: text.value,
      model: selectedModel.value,
    })
  } catch (e) {
    toast.showError(String(e))
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="calculator">
    <h2 class="page-title">Token 计算</h2>

    <div class="calc-card">
      <div class="field-group">
        <label class="field-label">模型选择</label>
        <select v-model="selectedModel" class="select-field">
          <option v-for="m in models" :key="m.id" :value="m.id">
            {{ m.name }} — {{ m.inputPrice === 0 ? '免费' : '¥' + m.inputPrice + '/M' }}
          </option>
        </select>
      </div>

      <div class="field-group">
        <label class="field-label">
          输入文本
          <span class="char-count">{{ charCount }} 字</span>
        </label>
        <textarea
          v-model="text"
          class="text-area"
          placeholder="在此输入或粘贴文本..."
          rows="6"
        ></textarea>
      </div>

      <div class="estimate-row">
        <div class="estimate-box">
          <span class="est-label">快速估算</span>
          <span class="est-value">{{ estimatedTokens }}</span>
          <span class="est-unit">tokens</span>
          <span class="est-divider">|</span>
          <span class="est-cost">{{ estimatedCost }}</span>
        </div>
        <button class="calc-btn" :disabled="loading || !apiKey" @click="countTokens">
          <span v-if="loading" class="spinner"></span>
          <span v-else>精确计算</span>
        </button>
      </div>
    </div>

    <div v-if="apiResult" class="result-card">
      <h3 class="section-title">精确结果</h3>
      <div class="result-grid">
        <div class="result-item">
          <div class="result-num highlight">{{ apiResult.prompt_tokens }}</div>
          <div class="result-lbl">输入 Tokens</div>
        </div>
        <div class="result-item">
          <div class="result-num">{{ apiResult.completion_tokens }}</div>
          <div class="result-lbl">输出 Tokens</div>
        </div>
        <div class="result-item">
          <div class="result-num">{{ apiResult.total_tokens }}</div>
          <div class="result-lbl">总计</div>
        </div>
        <div class="result-item">
          <div class="result-num cost">{{ apiCost }}</div>
          <div class="result-lbl">预估费用</div>
        </div>
      </div>
    </div>

    <div class="pricing-card">
      <h3 class="section-title">定价参考</h3>
      <table class="price-table">
        <thead>
          <tr><th>模型</th><th>输入 / M</th><th>输出 / M</th></tr>
        </thead>
        <tbody>
          <tr v-for="m in models" :key="m.id" :class="{ active: m.id === selectedModel }">
            <td>{{ m.name }}</td>
            <td>{{ m.inputPrice === 0 ? '免费' : '¥' + m.inputPrice }}</td>
            <td>{{ m.outputPrice === 0 ? '免费' : '¥' + m.outputPrice }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.calculator {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.page-title { font-size: 22px; font-weight: 700; }

.calc-card {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.field-group {}

.field-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  margin-bottom: 6px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.char-count {
  font-weight: 400;
  text-transform: none;
  letter-spacing: 0;
}

.select-field {
  width: 100%;
}

.text-area {
  width: 100%;
  resize: vertical;
  min-height: 100px;
  line-height: 1.6;
}

.estimate-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.estimate-box {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius-xs);
}

.est-label { font-size: 12px; color: var(--text-secondary); }
.est-value { font-size: 18px; font-weight: 700; color: var(--accent); font-variant-numeric: tabular-nums; }
.est-unit { font-size: 12px; color: var(--text-secondary); }
.est-divider { color: var(--border); }
.est-cost { font-size: 13px; color: var(--text-secondary); font-weight: 500; }

.calc-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 20px;
  background: var(--accent-gradient);
  color: #fff;
  border-radius: var(--radius-sm);
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  transition: opacity 0.2s;
}
.calc-btn:hover:not(:disabled) { opacity: 0.9; }
.calc-btn:disabled { opacity: 0.4; cursor: not-allowed; }

.spinner {
  width: 14px; height: 14px;
  border: 2px solid rgba(255,255,255,0.3);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.result-card {
  padding-top: 16px;
  border-top: 1px solid var(--border);
}

.section-title { font-size: 15px; font-weight: 600; margin-bottom: 16px; }

.result-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}

.result-item { text-align: center; }
.result-num { font-size: 22px; font-weight: 700; font-variant-numeric: tabular-nums; }
.result-num.highlight { color: var(--accent); }
.result-num.cost { color: var(--success); }
.result-lbl { font-size: 11px; color: var(--text-secondary); margin-top: 4px; }

.pricing-card {
  padding-top: 16px;
  border-top: 1px solid var(--border);
}

.price-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
.price-table th, .price-table td {
  padding: 8px 12px;
  text-align: left;
  border-bottom: 1px solid var(--border);
}
.price-table th { font-weight: 500; color: var(--text-secondary); font-size: 12px; }
.price-table tr.active { background: var(--accent-light); }
.price-table tr:last-child td { border-bottom: none; }
</style>
