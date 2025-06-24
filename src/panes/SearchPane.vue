<script setup lang="tsx">
import { ref, watch } from 'vue'
import { commands } from '../bindings.ts'
import { useMessage, useNotification } from 'naive-ui'
import ComicCard from '../components/ComicCard.vue'
import { useStore } from '../store.ts'
import { useI18n } from '../utils.ts'
import { SearchOutline, ArrowForwardOutline } from '@vicons/ionicons5'
import FloatLabelInput from '../components/FloatLabelInput.vue'

const { t } = useI18n()

const store = useStore()

const message = useMessage()
const notification = useNotification()

const searchInput = ref<string>('')
const searchInputRef = ref<InstanceType<typeof FloatLabelInput>>()
const comicIdInput = ref<string>('')
const currentPage = ref<number>(1)
const comicCardContainerRef = ref<HTMLElement>()

const searching = ref<boolean>(false)

watch(
  () => store.searchResult,
  () => {
    if (comicCardContainerRef.value !== undefined) {
      comicCardContainerRef.value.scrollTo({ top: 0, behavior: 'instant' })
    }
  },
)

async function search(query: string, pageNum: number) {
  if (searching.value) {
    message.warning(() => t('search_pane.searching_warning'))
    return
  }

  searchInput.value = query
  currentPage.value = pageNum
  store.currentTabName = 'search'
  searching.value = true

  // TODO: support sort by popularity
  const result = await commands.search(query, pageNum, false)
  if (result.status === 'error') {
    console.error(result.error)
    searching.value = false
    return
  }
  store.searchResult = result.data

  searching.value = false
}

async function handlePageChange(pageNum: number) {
  if (store.searchResult === undefined) {
    return
  }

  currentPage.value = pageNum

  const result = await commands.getPage(store.searchResult.ids, pageNum)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }

  store.searchResult = result.data
}

function getComicIdFromComicIdInput(): number | undefined {
  const comicIdString = comicIdInput.value.trim()
  // if it is a number, return it directly
  const comicId = parseInt(comicIdString)
  if (!isNaN(comicId)) {
    return comicId
  }
  // otherwise, extract it from the url
  const regex = /-(\d+).html/
  const match = comicIdString.match(regex)
  if (match === null || match[1] === null) {
    return
  }
  return parseInt(match[1])
}

async function pickComic() {
  const comicId = getComicIdFromComicIdInput()
  if (comicId === undefined) {
    notification.error({
      title: () => t('search_pane.comic_id_invalid'),
      description: () => t('search_pane.enter_comic_id_or_url'),
    })
    return
  }

  const result = await commands.getComic(comicId)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }

  store.pickedComic = result.data
  store.currentTabName = 'comic'
}

defineExpose({ search })
</script>

<template>
  <div class="h-full flex flex-col gap-2">
    <n-input-group class="box-border px-2 pt-2">
      <FloatLabelInput
        :label="t('search_pane.search_by_query')"
        ref="searchInputRef"
        size="small"
        v-model:value="searchInput"
        clearable
        @keydown.enter="search(searchInput.trim(), 1)" />
      <n-button :loading="searching" type="primary" class="w-15%" size="small" @click="search(searchInput.trim(), 1)">
        <template #icon>
          <n-icon size="22">
            <SearchOutline />
          </n-icon>
        </template>
      </n-button>
    </n-input-group>

    <n-input-group class="box-border px-2">
      <FloatLabelInput
        :label="t('search_pane.search_by_id')"
        class="text-align-left"
        size="small"
        v-model:value="comicIdInput"
        clearable
        @keydown.enter="pickComic" />
      <n-button type="primary" class="w-15%" size="small" @click="pickComic">
        <template #icon>
          <n-icon size="22">
            <ArrowForwardOutline />
          </n-icon>
        </template>
      </n-button>
    </n-input-group>

    <div
      v-if="store.searchResult !== undefined"
      ref="comicCardContainerRef"
      class="flex flex-col gap-row-2 overflow-auto box-border px-2">
      <comic-card
        v-for="(comic, index) in store.searchResult.comics"
        :key="comic.id"
        :search="search"
        v-model:comic="store.searchResult.comics[index]" />
    </div>

    <n-pagination
      v-if="store.searchResult !== undefined"
      class="box-border p-2 pt-0 mt-auto"
      :page-count="store.searchResult.totalPage"
      :page="currentPage"
      @update:page="handlePageChange" />
  </div>
</template>
