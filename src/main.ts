import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import 'virtual:uno.css'
import { createI18n } from 'vue-i18n'
import { locales } from './locales'

export type SupportedLocales = keyof typeof locales
export type MessageSchema = (typeof locales)['zh-CN']

export const i18n = createI18n<[MessageSchema], SupportedLocales>({
  locale: 'zh-CN',
  fallbackLocale: 'zh-CN',
  globalInjection: true,
  legacy: false,
  messages: locales,
})

const pinia = createPinia()
const app = createApp(App)

app.use(pinia).use(i18n).mount('#app')
