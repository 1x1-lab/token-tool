<script setup lang="ts">
defineProps<{
  title: string
  description?: string
  iconVariant?: 'accent' | 'success' | 'warning' | 'danger' | 'purple'
}>()
</script>

<template>
  <div class="setting-card">
    <div class="card-header">
      <div :class="['card-icon', `icon-${iconVariant ?? 'accent'}`]">
        <slot name="icon" />
      </div>
      <div>
        <div class="card-title">{{ title }}</div>
        <div v-if="description || $slots.description" class="card-desc">
          <slot name="description">{{ description }}</slot>
        </div>
      </div>
      <div class="card-action">
        <slot name="action" />
      </div>
    </div>
    <slot />
  </div>
</template>

<style scoped>
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

.icon-accent {
  background: var(--accent-light);
  color: var(--accent);
}

.icon-success {
  background: var(--success-light);
  color: var(--success);
}

.icon-warning {
  background: var(--warning-light);
  color: var(--warning);
}

.icon-danger {
  background: var(--danger-light);
  color: var(--danger);
}

.icon-purple {
  background: rgba(139, 92, 246, 0.08);
  color: #8b5cf6;
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

.setting-card > :slotted(*) {
  margin-left: 48px;
}

.card-action {
  margin-left: auto;
  flex-shrink: 0;
}
</style>
