import * as i18n from 'vue-i18n'
import { MessageSchema, SupportedLocales } from './main.ts'

export function useI18n() {
  return i18n.useI18n<{ message: MessageSchema }, SupportedLocales>()
}
