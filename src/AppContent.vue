<script setup lang="ts">
import { h, onMounted, ref, watch } from 'vue'
import { useMessage, useNotification } from 'naive-ui'
import { commands } from './bindings.ts'
import { useStore } from './store.ts'
import LogViewer from './components/LogViewer.vue'
import { useI18n } from './utils.ts'
import AboutDialog from './components/AboutDialog.vue'

const { t } = useI18n()

const store = useStore()

const message = useMessage()
const notification = useNotification()

const logViewerShowing = ref<boolean>(false)
const aboutDialogShowing = ref<boolean>(false)

watch(
    () => store.config,
    async () => {
      if (store.config === undefined) {
        return
      }
      await commands.saveConfig(store.config)
      message.success(() => t('app_content.save_config_success'))
    },
    { deep: true },
)

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
      content: () => [
        h('div', [t('app_content.click_top_center'), h('span', { class: 'bg-gray-2 px-1' }, t('log_viewer.name'))]),
        h('div', [
          t('app_content.there_is'),
          h('span', { class: 'bg-gray-2 px-1' }, t('log_viewer.open_logs_directory')),
        ]),
        h('div', [
          t('app_content.you_can_also_uncheck'),
          h('span', { class: 'bg-gray-2 px-1' }, t('log_viewer.enable_file_logging')),
        ]),
        h('div', t('app_content.this_will_disable_file_logging')),
      ],
    })
  }
})
</script>

<template>
  <div v-if="store.config !== undefined" class="h-screen flex flex-col">
    <div class="flex">
      <n-button @click="logViewerShowing = true">{{ t('log_viewer.name') }}</n-button>
      <n-button @click="aboutDialogShowing = true">{{ t('about_dialog.name') }}</n-button>
    </div>
    <log-viewer v-model:showing="logViewerShowing" />
    <about-dialog v-model:showing="aboutDialogShowing" />
  </div>
</template>
<style scoped>
:global(.n-notification-main__header) {
  @apply break-words;
}
</style>
