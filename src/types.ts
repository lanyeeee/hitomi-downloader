import { DownloadTaskEvent } from './bindings.ts'

export type CurrentTabName = 'search' | 'comic' | 'downloading' | 'downloaded' | 'settings'

export type ProgressData = Extract<DownloadTaskEvent, { event: 'Create' }>['data'] & {
  percentage: number
  indicator: string
}
