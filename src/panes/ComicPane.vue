<script setup lang="ts">
import { useStore } from '../store.ts'
import { Comic, commands } from '../bindings.ts'
import { computed, watch, ref, nextTick } from 'vue'
import { useI18n } from '../utils.ts'
import DownloadButton from '../components/DownloadButton.vue'
import { ReloadOutline } from '@vicons/ionicons5'
import ComicCard from '../components/ComicCard.vue'
import { path } from '@tauri-apps/api'

const { t } = useI18n()

defineProps<{
  search: (query: string, pageNum: number) => Promise<void>
}>()

const store = useStore()

const relatedComics = ref<Comic[]>([])
const containerRef = ref<HTMLElement>()

const cover = computed<string | undefined>(() =>
  store.pickedComic ? store.covers.get(store.pickedComic.id) : undefined,
)

watch(
  () => store.pickedComic,
  () => {
    if (store.pickedComic === undefined) {
      return
    }
    console.log(containerRef.value)
    if (containerRef.value !== undefined) {
      console.log('scrollTo')
      nextTick(() => containerRef.value?.scrollTo({ top: 0, behavior: 'instant' }))
    }

    if (cover.value === undefined) {
      store.loadCover(store.pickedComic.id, store.pickedComic.coverUrl)
    }

    reloadRelatedComics()
  },
)

async function pickComic(id: number) {
  const result = await commands.getComic(id)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  store.pickedComic = result.data
}

async function reloadRelatedComics() {
  console.log('reloadRelatedComics')
  if (store.pickedComic === undefined) {
    return
  }

  relatedComics.value = []

  const promises = store.pickedComic.related.map(async (id) => {
    const result = await commands.getComic(id)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }
    relatedComics.value.push(result.data)
  })

  await Promise.all(promises)
}

async function showComicDownloadDirInFileManager() {
  if (store.pickedComic === undefined || store.config === undefined) {
    return
  }
  const comicDir = await path.join(store.config.downloadDir, store.pickedComic.dirName)
  const result = await commands.showPathInFileManager(comicDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <n-empty
    class="p-2"
    v-if="store.pickedComic === undefined"
    :description="t('comic_pane.empty_description')"></n-empty>
  <div ref="containerRef" v-else class="flex flex-col h-full overflow-auto box-border p-2">
    <span class="font-bold text-xl">{{ store.pickedComic.title }}</span>
    <div class="flex w-full">
      <img v-if="cover !== undefined" class="w-40 object-cover mr-2" :src="cover" alt="" />
      <n-icon v-else size="50" class="w-40 h-full flex items-center justify-center flex-shrink-0">
        <ReloadOutline
          class="cursor-pointer transition-transform duration-500 hover:rotate-360"
          @click="store.loadCover(store.pickedComic.id, store.pickedComic.coverUrl)" />
      </n-icon>
      <div class="flex flex-col w-full gap-1">
        <div>
          <!-- TODO: format the date with i18n -->
          {{ store.pickedComic.date }}
        </div>
        <div class="flex">
          <div>ID: {{ store.pickedComic.id }}</div>
          <div class="ml-auto">{{ store.pickedComic.files.length }}P</div>
        </div>
        <div class="flex flex-col mt-auto mb-2">
          <div class="font-bold">{{ t('comic_pane.other_language') }}</div>
          <div class="flex flex-wrap gap-1">
            <n-button
              v-for="(language, index) in store.pickedComic.languages"
              :key="index"
              size="tiny"
              round
              class="hover:scale-110 transition-transform duration-100"
              @click="pickComic(language.galleryid)">
              {{ language.language_localname }}
            </n-button>
          </div>
        </div>
        <div class="flex flex-col gap-row-2">
          <n-button v-if="store.pickedComic.isDownloaded" size="small" @click="showComicDownloadDirInFileManager">
            {{ t('common.open_directory') }}
          </n-button>
          <download-button
            class="mt-auto"
            size="small"
            type="primary"
            :comic-id="store.pickedComic.id"
            :comic-downloaded="store.pickedComic.isDownloaded === true" />
        </div>
      </div>
    </div>
    <div class="flex flex-col gap-2 mt-1 pb-2">
      <div>
        <div class="font-bold">{{ t('common.artist') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="(artist, index) in store.pickedComic.artists"
            :key="index"
            size="tiny"
            round
            class="hover:scale-110 transition-transform duration-100"
            @click="search(`artist:${artist.replace(' ', '_')}`, 1)">
            {{ artist }}
          </n-button>
        </div>
      </div>
      <div>
        <div class="font-bold">{{ t('common.group') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="(group, index) in store.pickedComic.groups"
            :key="index"
            size="tiny"
            round
            class="hover:scale-110 transition-transform duration-100"
            @click="search(`group:${group.replace(' ', '_')}`, 1)">
            {{ group }}
          </n-button>
        </div>
      </div>
      <div>
        <div class="font-bold">{{ t('common.type') }}</div>
        <n-button
          size="tiny"
          round
          class="hover:scale-110 transition-transform duration-100"
          @click="search(`type:${store.pickedComic.type.replace(' ', '_')}`, 1)">
          {{ store.pickedComic.type }}
        </n-button>
      </div>
      <div>
        <div class="font-bold">{{ t('common.language') }}</div>
        <n-button
          v-if="store.pickedComic.language !== ''"
          size="tiny"
          round
          class="hover:scale-110 transition-transform duration-100"
          @click="search(`language:${store.pickedComic.language.replace(' ', '_')}`, 1)">
          {{ store.pickedComic.languageLocalname }}
        </n-button>
      </div>
      <div>
        <div class="font-bold">{{ t('common.series') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="(series, index) in store.pickedComic.parodys"
            :key="index"
            round
            class="hover:scale-110 transition-transform duration-100"
            size="tiny"
            @click="search(`series:${series.replace(' ', '_')}`, 1)">
            {{ series }}
          </n-button>
        </div>
      </div>
      <div>
        <div class="font-bold">{{ t('common.character') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="(character, index) in store.pickedComic.characters"
            :key="index"
            round
            class="hover:scale-110 transition-transform duration-100"
            size="tiny"
            @click="search(`character:${character.replace(' ', '_')}`, 1)">
            {{ character }}
          </n-button>
        </div>
      </div>
      <div>
        <div class="font-bold">{{ t('common.tag') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="({ tag, female, male }, index) in store.pickedComic.tags"
            :key="index"
            round
            class="hover:scale-110 transition-transform duration-100'"
            :color="female !== 0 ? '#F472B6' : male !== 0 ? '#60A5FA' : undefined"
            size="tiny"
            @click="search(`${female !== 0 ? 'female' : male !== 0 ? 'male' : 'tag'}:${tag.replace(' ', '_')}`, 1)">
            {{ tag }}
          </n-button>
        </div>
      </div>
      <div>
        <div class="font-bold">{{ t('comic_pane.related') }}</div>
        <div class="flex flex-wrap gap-1">
          <ComicCard v-for="comic in relatedComics" :key="comic.id" :comic="comic" :search="search" />
        </div>
      </div>
    </div>
  </div>
</template>
