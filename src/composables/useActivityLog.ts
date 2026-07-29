import { ref } from 'vue'

export type LogLevel = 'info' | 'success' | 'warning' | 'error'

export interface LogEntry {
  id: number
  time: string
  level: LogLevel
  message: string
}

const MAX_ENTRIES = 300

const entries = ref<LogEntry[]>([])
let nextId = 1

const timestamp = () => {
  const d = new Date()
  const pad = (n: number) => `${n}`.padStart(2, '0')
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

export const useActivityLog = () => {
  const log = (message: string, level: LogLevel = 'info') => {
    entries.value.push({ id: nextId++, time: timestamp(), level, message })
    if (entries.value.length > MAX_ENTRIES) {
      entries.value.splice(0, entries.value.length - MAX_ENTRIES)
    }
  }

  const clear = () => {
    entries.value = []
  }

  return { entries, log, clear }
}
