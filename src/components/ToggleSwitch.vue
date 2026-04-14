<script setup lang="ts">
const on = defineModel<boolean>({ required: true })
</script>

<template>
  <button
    :class="['toggle-switch', { on }]"
    @click="on = !on"
    @mousedown.stop
  >
    <span class="toggle-knob"></span>
  </button>
</template>

<style scoped>
.toggle-switch {
  width: 40px;
  height: 22px;
  border-radius: 11px;
  background: var(--border);
  position: relative;
  transition: background 0.2s;
  cursor: pointer;
  flex-shrink: 0;
}

.toggle-switch.on {
  background: var(--accent);
}

.toggle-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #fff;
  transition: transform 0.2s;
}

.toggle-switch.on .toggle-knob {
  transform: translateX(18px);
}
</style>
