<script setup lang="ts">
import { getVersion } from '@tauri-apps/api/app'
import { ref, onMounted } from 'vue'
import icon from '../../src-tauri/icons/128x128.png'
import { useI18n } from '../utils.ts'

const { t } = useI18n()

const showing = defineModel<boolean>('showing', { required: true })

const version = ref('')

onMounted(async () => {
  version.value = await getVersion()
})
</script>

<template>
  <n-modal v-model:show="showing">
    <n-dialog :showIcon="false" @close="showing = false">
      <div class="flex flex-col items-center gap-row-6">
        <img :src="icon" alt="icon" class="w-32 h-32" />
        <div class="text-center text-gray-400 text-xs">
          <i18n-t keypath="about_dialog.support_message" tag="div" scope="global">
            <template v-slot:github>
              <n-a href="https://github.com/lanyeeee/hitomi-downloader" target="_blank">GitHub</n-a>
            </template>
          </i18n-t>
          <div class="mt-1">{{ t('about_dialog.motivation_message') }}</div>
        </div>
        <div class="flex flex-col w-full gap-row-3 px-6">
          <div class="flex items-center justify-between py-2 px-4 bg-gray-100 rounded-lg">
            <span class="text-gray-500">{{ t('about_dialog.software_version') }}</span>
            <div class="font-medium">v{{ version }}</div>
          </div>
          <div class="flex items-center justify-between py-2 px-4 bg-gray-100 rounded-lg">
            <span class="text-gray-500">{{ t('about_dialog.source_code_repository') }}</span>
            <n-a href="https://github.com/lanyeeee/hitomi-downloader" target="_blank">GitHub</n-a>
          </div>
          <div class="flex items-center justify-between py-2 px-4 bg-gray-100 rounded-lg">
            <span class="text-gray-500">{{ t('about_dialog.feedback') }}</span>
            <n-a href="https://github.com/lanyeeee/hitomi-downloader/issues" target="_blank">GitHub Issues</n-a>
          </div>
        </div>
        <div class="flex flex-col text-xs text-gray-400">
          <div>
            Copyright © 2025
            <n-a href="https://github.com/lanyeeee" target="_blank">lanyeeee</n-a>
          </div>
          <div>
            Released under
            <n-a href="https://github.com/lanyeeee/hitomi-downloader/blob/main/LICENSE" target="_blank">MIT License</n-a>
          </div>
        </div>
      </div>
    </n-dialog>
  </n-modal>
</template>
