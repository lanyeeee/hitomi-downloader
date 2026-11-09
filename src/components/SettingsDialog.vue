<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useStore } from '../store'
import { computed, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'
import { commands } from '../bindings.ts'

const { t } = useI18n()

const store = useStore()
const message = useMessage()

const proxyHost = ref<string>(store.config.proxyHost)
const proxyUsername = ref<string>(store.config.proxyUsername ?? '')
const proxyPassword = ref<string>(store.config.proxyPassword ?? '')
const dirFmt = ref<string>(store.config.dirFmt)

const saving = ref<boolean>(false)
const testing = ref<boolean>(false)

const disableProxyHostAndPort = computed(() => store.config.proxyMode !== 'Custom')
const proxySchemePrefix = computed(() => `${(store.config.proxyProtocol ?? 'Http').toLowerCase()}://`)

// The dialog mounts before `getConfig()` resolves, so sync the editable fields once it loads
watch(
  () => store.config,
  (config) => {
    proxyHost.value = config.proxyHost
    proxyUsername.value = config.proxyUsername ?? ''
    proxyPassword.value = config.proxyPassword ?? ''
    dirFmt.value = config.dirFmt
  },
)

async function saveConfig(): Promise<boolean> {
  const host = proxyHost.value.trim()
  if (store.config.proxyMode === 'Custom') {
    if (host === '') {
      message.warning(() => t('settings_dialog.proxy_host_required'))
      return false
    }
    const port = store.config.proxyPort
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      message.warning(() => t('settings_dialog.proxy_port_invalid'))
      return false
    }
  }

  proxyHost.value = host
  store.config.proxyHost = host
  store.config.proxyUsername = proxyUsername.value.trim()
  store.config.proxyPassword = proxyPassword.value
  store.config.dirFmt = dirFmt.value

  saving.value = true
  const result = await commands.saveConfig(store.config)
  saving.value = false

  if (result.status === 'error') {
    message.error(() => `${t('settings_dialog.save_config_failed')}: ${result.error.err_message}`)
    return false
  }
  message.success(() => t('app_content.save_config_success'))
  return true
}

async function testProxy() {
  if (!(await saveConfig())) {
    return
  }
  testing.value = true
  const result = await commands.testProxy()
  testing.value = false

  if (result.status === 'error') {
    message.error(() => `${t('settings_dialog.proxy_test_failed')}: ${result.error.err_message}`)
    return
  }
  message.success(() => t('settings_dialog.proxy_test_success', { ms: result.data }))
}

async function showConfigInFileManager() {
  const configName = 'config.json'
  const configPath = await path.join(await appDataDir(), configName)
  const result = await commands.showPathInFileManager(configPath)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <div class="flex flex-col gap-row-2">
    <n-radio-group class="flex gap-2" v-model:value="store.config.downloadFormat">
      <span>{{ t('settings_dialog.download_format') }}</span>
      <n-radio value="Webp">webp</n-radio>
      <n-tooltip placement="top" trigger="hover">
        {{ t('settings_dialog.avif_warning') }}
        <template #trigger>
          <n-radio value="Avif">avif</n-radio>
        </template>
      </n-tooltip>
    </n-radio-group>
    <n-radio-group class="flex gap-2" v-model:value="store.config.proxyMode">
      {{ t('settings_dialog.proxy_mode') }}
      <n-radio value="System">{{ t('settings_dialog.system_proxy') }}</n-radio>
      <n-radio value="NoProxy">{{ t('settings_dialog.no_proxy') }}</n-radio>
      <n-radio value="Custom">{{ t('settings_dialog.custom_proxy') }}</n-radio>
    </n-radio-group>
    <n-radio-group class="flex gap-2" :disabled="disableProxyHostAndPort" v-model:value="store.config.proxyProtocol">
      {{ t('settings_dialog.proxy_protocol') }}
      <n-radio value="Http">HTTP</n-radio>
      <n-tooltip placement="top" trigger="hover">
        {{ t('settings_dialog.socks5_tip') }}
        <template #trigger>
          <n-radio value="Socks5">SOCKS5</n-radio>
        </template>
      </n-tooltip>
      <n-tooltip placement="top" trigger="hover" width="340">
        {{ t('settings_dialog.socks5h_tip') }}
        <template #trigger>
          <n-radio value="Socks5h">SOCKS5H</n-radio>
        </template>
      </n-tooltip>
    </n-radio-group>
    <n-input-group>
      <n-input-group-label size="small">{{ proxySchemePrefix }}</n-input-group-label>
      <n-input
        :disabled="disableProxyHostAndPort"
        v-model:value="proxyHost"
        size="small"
        placeholder="" />
      <n-input-group-label size="small">:</n-input-group-label>
      <n-input-number
        :disabled="disableProxyHostAndPort"
        v-model:value="store.config.proxyPort"
        size="small"
        placeholder=""
        :parse="(x: string) => parseInt(x)" />
    </n-input-group>
    <n-input-group>
      <n-input-group-label size="small">{{ t('settings_dialog.proxy_username') }}</n-input-group-label>
      <n-input
        :disabled="disableProxyHostAndPort"
        v-model:value="proxyUsername"
        size="small"
        placeholder="" />
      <n-input-group-label size="small">{{ t('settings_dialog.proxy_password') }}</n-input-group-label>
      <n-input
        :disabled="disableProxyHostAndPort"
        v-model:value="proxyPassword"
        size="small"
        type="password"
        show-password-on="click"
        placeholder="" />
    </n-input-group>
    <p class="m-0 text-xs opacity-60">{{ t('settings_dialog.proxy_credentials_tip') }}</p>
    <n-tooltip placement="top" trigger="hover" width="580">
      <i18n-t keypath="settings_dialog.directory_format.directory_level_tips" tag="div" scope="global">
        <template v-slot:slash>
          <span class="rounded bg-gray-500 px-1 text-white">/</span>
        </template>
      </i18n-t>
      <div class="font-semibold mt-2">{{ t('settings_dialog.directory_format.available_fields') }}</div>
      <div>
        <div>
          <span class="rounded bg-gray-500 px-1">id</span>
          <span class="ml-2">{{ t('settings_dialog.directory_format.id') }}</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1">title</span>
          <span class="ml-2">{{ t('settings_dialog.directory_format.title') }}</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1">type</span>
          <span class="ml-2">{{ t('settings_dialog.directory_format.type') }}</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1">artists</span>
          <span class="ml-2">{{ t('settings_dialog.directory_format.artists') }}</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1">language</span>
          <span class="ml-2">{{ t('settings_dialog.directory_format.language') }}</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1">language_localname</span>
          <span class="ml-2">{{ t('settings_dialog.directory_format.language_localname') }}</span>
        </div>
      </div>
      <div class="font-semibold mt-2">{{ t('settings_dialog.directory_format.for_example') }}</div>
      <div class="bg-gray-200 rounded-md p-1 text-black w-fit">
        {type}/{artists}/[{artists}] {title}({id}) - {language}({language_localname})
      </div>
      <div class="font-semibold">{{ t('settings_dialog.directory_format.directory_result') }}</div>
      <div class="flex gap-1 text-black">
        <div class="bg-gray-200 rounded-md px-2 w-fit">doujinshi</div>
        <span class="rounded bg-gray-500 px-1 text-white">/</span>
        <div class="bg-gray-200 rounded-md px-2 w-fit">mameroku</div>
        <span class="rounded bg-gray-500 px-1 text-white">/</span>
        <div class="bg-gray-200 rounded-md px-2 w-fit">[mameroku] Soushi Souai.(2829145) - chinese(中文)</div>
      </div>
      <template #trigger>
        <n-input-group class="box-border">
          <n-input-group-label size="small">{{ t('settings_dialog.directory_format.name') }}</n-input-group-label>
          <n-input
            v-model:value="dirFmt"
            size="small" />
        </n-input-group>
      </template>
    </n-tooltip>
    <div class="mt-2 flex items-center justify-end gap-2">
      <n-button class="mr-auto" size="small" @click="showConfigInFileManager">
        {{ t('settings_dialog.open_config_directory') }}
      </n-button>
      <n-button size="small" :loading="testing" :disabled="saving" @click="testProxy">
        {{ t('settings_dialog.test_proxy') }}
      </n-button>
      <n-button type="primary" size="small" :loading="saving" :disabled="testing" @click="saveConfig">
        {{ t('settings_dialog.save_config') }}
      </n-button>
    </div>
  </div>
</template>
