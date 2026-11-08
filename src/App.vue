<script setup lang="ts">
// This starter template is using Vue 3 <script setup> SFCs
// Check out https://vuejs.org/api/sfc-script-setup.html#script-setup
import { onMounted, watch } from 'vue'
import AppContent from './AppContent.vue'
import { useStore } from './store.ts'
import { GlobalThemeOverrides } from 'naive-ui'

const store = useStore()
const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#6275C0FF',
    primaryColorHover: '#8998CCFF',
    primaryColorPressed: '#465499FF',
    primaryColorSuppl: '#8998CCFF',
  },
}

function updateTheme() {
  const t = store.theme
  const dark =
    t === 'system'
      ? window.matchMedia('(prefers-color-scheme: dark)').matches
      : t === 'dark'
  document.documentElement.classList.toggle('dark', dark)
}

onMounted(() => {
  const saved = localStorage.getItem('theme')
  if (saved && (saved === 'light' || saved === 'dark' || saved === 'system')) {
    store.theme = saved as typeof store.theme
  }
  updateTheme()
  const media = window.matchMedia('(prefers-color-scheme: dark)')
  media.addEventListener('change', updateTheme)
})

watch(() => store.theme, updateTheme)
</script>

<template>
  <n-config-provider :theme-overrides="themeOverrides">
    <n-modal-provider>
      <n-notification-provider placement="bottom-right">
        <n-message-provider>
          <app-content />
        </n-message-provider>
      </n-notification-provider>
    </n-modal-provider>
  </n-config-provider>
</template>
