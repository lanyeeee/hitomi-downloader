<script setup lang="tsx">
import { onMounted, ref, computed } from 'vue'
import { useNotification } from 'naive-ui'
import { commands } from './bindings.ts'
import { useStore, Theme } from './store.ts'
import LogViewer from './components/LogViewer.vue'
import { useI18n } from './utils.ts'
import AboutDialog from './components/AboutDialog.vue'
import SearchPane from './panes/SearchPane.vue'
import DownloadedPane from './panes/DownloadedPane.vue'
import ComicPane from './panes/ComicPane.vue'
import DownloadingPane from './panes/DownloadingPane.vue'
import { PhClockCounterClockwise, PhInfo, PhTranslate, PhSun, PhMoon, PhMonitor } from '@phosphor-icons/vue'
import { locales } from './locales'

const { t, locale } = useI18n()
const store = useStore()

const themeLabel = computed(() => {
  const mode = store.theme
  if (mode === 'system') return t('theme.system_mode')
  if (mode === 'dark') return t('theme.dark_mode')
  return t('theme.light_mode')
})

function toggleTheme() {
  const themes: Theme[] = ['light', 'system', 'dark']
  const current = store.theme ?? 'system'
  const nextIndex = (themes.indexOf(current) + 1) % themes.length
  store.setTheme(themes[nextIndex])
}

const notification = useNotification()

const logViewerShowing = ref<boolean>(false)
const aboutDialogShowing = ref<boolean>(false)

const localeOptions = Object.entries(locales).map(([key, value]) => ({
  label: value.name,
  value: key,
}))

const searchPaneRef = ref<InstanceType<typeof SearchPane>>()

const activeDownloads = computed(() => {
  let count = 0
  for (const [, data] of store.progresses.entries()) {
    if (data.state === 'Downloading' || data.state === 'Pending') {
      count++
    }
  }
  let status = `${count > 0 ? `(${count}) ` : ''}${t('downloading_pane.name')}`
  return status
})

onMounted(async () => {
  // block the browser right-click menu
  document.oncontextmenu = (event) => {
    event.preventDefault()
  }
  // get the configuration
  store.config = await commands.getConfig()
  // check the logs directory size
  const result = await commands.getLogsDirSize()
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  if (result.data > 50 * 1024 * 1024) {
    notification.warning({
      title: () => t('app_content.logs_directory_size_too_big'),
      description: () => t('app_content.log_cleanup_reminder'),
      duration: 4000,
      content: () => (
        <>
          <div>
            {t('app_content.click_top_center')}
            <span class="bg-gray-2 px-1">{t('log_viewer.name')}</span>
          </div>
          <div>
            {t('app_content.there_is')}
            <span class="bg-gray-2 px-1">{t('log_viewer.open_logs_directory')}</span>
          </div>
          <div>
            {t('app_content.you_can_also_uncheck')}
            <span class="bg-gray-2 px-1">{t('log_viewer.enable_file_logging')}</span>
          </div>
          <div>{t('app_content.this_will_disable_file_logging')}</div>
        </>
      ),
    })
  }
})
</script>

<template>
  <div v-if="store.config !== undefined" class="h-screen flex flex-col">
    <div class="relative flex flex-1 overflow-hidden p-2">
      <n-tabs class="h-full" v-model:value="store.currentTabName" type="line" size="small" animated>
        <n-tab-pane
          class="h-full overflow-hidden p-0!"
          name="search"
          :tab="t('search_pane.name')"
          display-directive="show">
          <searchPane ref="searchPaneRef" />
        </n-tab-pane>
        <n-tab-pane
          class="h-full overflow-hidden p-0!"
          name="comic"
          :tab="t('comic_pane.name')"
          display-directive="show">
          <comic-pane v-if="searchPaneRef !== undefined" :search="searchPaneRef.search" />
        </n-tab-pane>
        <n-tab-pane
          name="downloading"
          class="h-full overflow-hidden p-0!"
          display-directive="show">
          <template #tab>
            {{ activeDownloads }}
          </template>
          <downloading-pane
            class="h-full overflow-hidden"
            v-if="searchPaneRef !== undefined"
            :search="searchPaneRef.search" />
        </n-tab-pane>
        <n-tab-pane
          class="h-full overflow-hidden p-0!"
          name="downloaded"
          :tab="t('downloaded_pane.name')"
          display-directive="show">
          <downloaded-pane v-if="searchPaneRef !== undefined" :search="searchPaneRef.search" />
        </n-tab-pane>
        <n-tab-pane
          name="settings"
          class="h-full overflow-auto p-0!"
          :tab="t('settings_dialog.name')"
          display-directive="show">
          <div class="mx-auto w-full max-w-4xl box-border p-3">
            <div class="my-2 flex flex-wrap items-center gap-2">
              <n-button @click="logViewerShowing = true">
                <template #icon>
                  <n-icon size="22">
                    <PhClockCounterClockwise />
                  </n-icon>
                </template>
                {{ t('log_viewer.name') }}
              </n-button>
              <n-button @click="aboutDialogShowing = true">
                <template #icon>
                  <n-icon size="22">
                    <PhInfo />
                  </n-icon>
                </template>
                {{ t('about_dialog.name') }}
              </n-button>
              <n-icon size="28">
                <PhTranslate />
              </n-icon>
              <n-select class="w-30" v-model:value="locale" :options="localeOptions" />
            </div>

            <settings-dialog />
          </div>
        </n-tab-pane>
      </n-tabs>
      <n-button
        size="small"
        @click="toggleTheme"
        :title="themeLabel"
        class="absolute right-4 top-3 z-10 shadow-md">
        <template #icon>
          <n-icon size="20" class="flex items-center justify-center">
            <PhSun v-if="store.theme === 'light'" />
            <PhMoon v-else-if="store.theme === 'dark'" />
            <PhMonitor v-else />
          </n-icon>
        </template>
        <span class="hidden md:inline ml-1">{{ themeLabel }}</span>
      </n-button>
    </div>

    <log-viewer v-model:showing="logViewerShowing" />
    <about-dialog v-model:showing="aboutDialogShowing" />
  </div>
</template>
<style scoped>
:global(.n-notification-main__header) {
  @apply break-words;
}

:global(.n-tabs-pane-wrapper) {
  @apply h-full;
}

:deep(.n-tabs-nav) {
  @apply px-2 pr-16 md:pr-32;
}
</style>
