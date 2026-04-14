<script setup lang="ts">
const props = defineProps<{
  modelValue: number
  options: number[]
  label: string
  appliedValue?: number
  showSaveBtn?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: number]
  save: []
}>()

function formatSec(sec: number): string {
  return sec < 60 ? sec + '秒' : (sec / 60) + '分'
}

function isActive(sec: number): boolean {
  if (props.appliedValue !== undefined) {
    return props.appliedValue === sec && props.modelValue === sec
  }
  return props.modelValue === sec
}

function isPending(sec: number): boolean {
  if (props.appliedValue === undefined) return false
  return props.modelValue === sec && props.appliedValue !== sec
}

function hasPendingChange(): boolean {
  if (props.appliedValue === undefined) return false
  return props.modelValue !== props.appliedValue
}
</script>

<template>
  <div class="interval-row">
    <span class="interval-label">{{ label }}</span>
    <div class="interval-btns">
      <button
        v-for="sec in options"
        :key="sec"
        :class="['interval-btn', { active: isActive(sec), pending: isPending(sec) }]"
        @click="emit('update:modelValue', sec)"
      >
        {{ formatSec(sec) }}
      </button>
    </div>
    <button
      v-if="showSaveBtn && hasPendingChange()"
      class="interval-save-btn"
      @click="emit('save')"
    >
      <slot name="save-text">保存</slot>
    </button>
  </div>
</template>

<style scoped>
.interval-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.interval-label {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
}

.interval-btns {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.interval-btn {
  padding: 4px 10px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 11px;
  font-weight: 500;
  transition: all 0.15s;
  cursor: pointer;
}

.interval-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.interval-btn.active {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}

.interval-btn.pending {
  border-style: dashed;
  border-color: var(--accent);
  color: var(--accent);
  background: transparent;
}

.interval-save-btn {
  margin-left: auto;
  padding: 4px 12px;
  background: var(--accent);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: opacity 0.15s;
  flex-shrink: 0;
}

.interval-save-btn:hover { opacity: 0.85; }
</style>
