<script setup lang="ts">
import { LogEvent, LogLevel, events, commands } from '../bindings.ts'
import { useNotification } from 'naive-ui'
import { onMounted, ref, watch, computed } from 'vue'
import { appDataDir } from '@tauri-apps/api/path'
import { path } from '@tauri-apps/api'
import { useStore } from '../store.ts'
import { darkTheme } from 'naive-ui'
import { useI18n } from '../utils.ts'

const { t } = useI18n()

type LogRecord = LogEvent & { id: number; formatedLog: string }

const store = useStore()

const notification = useNotification()

const showing = defineModel<boolean>('showing', { required: true })

let nextLogRecordId = 1

const logRecords = ref<LogRecord[]>([])
const searchText = ref<string>('')
const selectedLevel = ref<LogLevel>('INFO')
const logsDirSize = ref<number>(0)

const formatedLogsDirSize = computed<string>(() => {
  const units = ['B', 'KB', 'MB']
  let size = logsDirSize.value
  let unitIndex = 0

  while (size >= 1024 && unitIndex < 2) {
    size /= 1024
    unitIndex++
  }

  return `${size.toFixed(2)} ${units[unitIndex]}`
})
const filteredLogs = computed<LogRecord[]>(() => {
  return logRecords.value.filter(({ level, formatedLog }) => {
    // Define the priority order of log levels
    const logLevelPriority = {
      TRACE: 0,
      DEBUG: 1,
      INFO: 2,
      WARN: 3,
      ERROR: 4,
    }
    // First filter by log level
    if (logLevelPriority[level] < logLevelPriority[selectedLevel.value]) {
      return false
    }
    // Then filter by search text
    if (searchText.value === '') {
      return true
    }

    return formatedLog.toLowerCase().includes(searchText.value.toLowerCase())
  })
})

watch(showing, async () => {
  if (showing.value) {
    const result = await commands.getLogsDirSize()
    if (result.status === 'error') {
      console.error(result.error)
      return
    }
    logsDirSize.value = result.data
  }
})

onMounted(async () => {
  await events.logEvent.listen(async ({ payload: logEvent }) => {
    const logRecord: LogRecord = {
      ...logEvent,
      id: nextLogRecordId++,
      formatedLog: formatLogEvent(logEvent),
    }
    logRecords.value.push(logRecord)

    const { level, fields } = logEvent
    if (level === 'ERROR') {
      notification.error({
        title: () => fields['err_title'] as string,
        description: () => fields['message'] as string,
        duration: 0,
      })
    }
  })
})

function formatLogEvent(logEvent: LogEvent): string {
  const { timestamp, level, fields, target, filename, line_number } = logEvent
  const fields_str = Object.entries(fields)
    .sort(([key1], [key2]) => key1.localeCompare(key2))
    .map(([key, value]) => `${key}=${value}`)
    .join(' ')
  return `${timestamp} ${level} ${target}: ${filename}:${line_number} ${fields_str}`
}

function getLevelStyles(level: LogLevel) {
  switch (level) {
    case 'TRACE':
      return 'text-gray-400'
    case 'DEBUG':
      return 'text-green-400'
    case 'INFO':
      return 'text-blue-400'
    case 'WARN':
      return 'text-yellow-400'
    case 'ERROR':
      return 'text-red-400'
  }
}

const logLevelOptions = [
  { value: 'TRACE', label: 'TRACE' },
  { value: 'DEBUG', label: 'DEBUG' },
  { value: 'INFO', label: 'INFO' },
  { value: 'WARN', label: 'WARN' },
  { value: 'ERROR', label: 'ERROR' },
]

function clearLogRecords() {
  logRecords.value = []
  nextLogRecordId = 1
}

async function showLogsDirInFileManager() {
  const logsDir = await path.join(await appDataDir(), 'logs')
  const result = await commands.showPathInFileManager(logsDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <n-modal v-model:show="showing" v-if="store.config !== undefined">
    <n-dialog
      :showIcon="false"
      :title="`${t('log_viewer.logs_directory_size')}: ${formatedLogsDirSize}`"
      @close="showing = false"
      style="width: 95%">
      <div class="mb-2 flex flex-wrap gap-2">
        <n-input-group class="w-55%">
          <n-input
            size="small"
            v-model:value="searchText"
            :placeholder="t('log_viewer.search_placeholder')"
            clearable />
          <n-select size="small" v-model:value="selectedLevel" :options="logLevelOptions" style="width: 130px" />
        </n-input-group>

        <div class="flex flex-wrap gap-2 ml-auto items-center">
          <n-button size="small" @click="showLogsDirInFileManager">{{ t('log_viewer.open_logs_directory') }}</n-button>
          <n-checkbox v-model:checked="store.config.enableFileLogger">
            {{ t('log_viewer.enable_file_logging') }}
          </n-checkbox>
        </div>
      </div>

      <n-config-provider :theme="darkTheme" :theme-overrides="{ Scrollbar: { width: '8px' } }">
        <n-virtual-list
          class="h-[calc(100vh-300px)] overflow-hidden bg-gray-900"
          :item-size="42"
          item-resizable
          :hoverable="false"
          :items="filteredLogs"
          :scrollbar-props="{ trigger: 'none' }">
          <template #default="{ item: { level, formatedLog } }: { item: LogRecord }">
            <div :class="['py-1 px-3 hover:bg-white/10 whitespace-pre-wrap mr-4', getLevelStyles(level)]">
              {{ formatedLog }}
            </div>
          </template>
        </n-virtual-list>
      </n-config-provider>
      <div class="pt-1 flex">
        <n-button ghost class="ml-auto" size="small" type="error" @click="clearLogRecords">
          {{ t('log_viewer.clear_log_viewer') }}
        </n-button>
      </div>
    </n-dialog>
  </n-modal>
</template>
