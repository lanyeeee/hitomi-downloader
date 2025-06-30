import zhCN from './zh-CN.json'
import enUS from './en-US.json'


const _locales = {
  'zh-CN': zhCN,
  'en-US': enUS,
}

// sort locales by key
export const locales = Object.fromEntries(
  Object.entries(_locales).sort(([keyA], [keyB]) => {
    return keyA.localeCompare(keyB)
  }),
) as typeof _locales
