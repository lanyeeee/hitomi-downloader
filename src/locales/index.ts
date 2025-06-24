import zhCN from './zh-CN.json'

const _locales = {
  'zh-CN': zhCN,
}

// sort locales by key
export const locales = Object.fromEntries(
  Object.entries(_locales).sort(([keyA], [keyB]) => {
    return keyA.localeCompare(keyB)
  }),
) as typeof _locales
