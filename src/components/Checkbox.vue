<script setup lang="ts">
const checked = defineModel<boolean>({ required: true })
</script>

<template>
  <span :class="['toggle-check', { checked }]" @click.prevent="checked = !checked">
    <svg v-if="checked" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
  </span>
</template>

<style scoped>
.toggle-check {
  width: 14px;
  height: 14px;
  border: 2px solid var(--border);
  border-radius: 3px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.15s;
}

.toggle-check.checked {
  background: var(--accent);
  border-color: var(--accent);
}
</style>
