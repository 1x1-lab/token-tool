import { ref } from 'vue'

export type ToastType = 'error' | 'success' | 'warning' | 'info'

interface Toast {
  id: number
  type: ToastType
  message: string
  duration: number
}

const toasts = ref<Toast[]>([])
let nextId = 0

function addToast(type: ToastType, message: string, duration = 4000) {
  const id = nextId++
  toasts.value.push({ id, type, message, duration })
  if (duration > 0) {
    setTimeout(() => dismiss(id), duration)
  }
}

function dismiss(id: number) {
  const idx = toasts.value.findIndex(t => t.id === id)
  if (idx !== -1) {
    toasts.value.splice(idx, 1)
  }
}

export function useToast() {
  return {
    toasts,
    show: addToast,
    showError: (msg: string, duration?: number) => addToast('error', msg, duration),
    showSuccess: (msg: string, duration?: number) => addToast('success', msg, duration),
    showWarning: (msg: string, duration?: number) => addToast('warning', msg, duration),
    showInfo: (msg: string, duration?: number) => addToast('info', msg, duration),
    dismiss,
  }
}
