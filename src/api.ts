import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export interface DesktopSettings {
  browserMode: 'existing' | 'managed'
  maxPostsDefault: number
  maxCommentsDefault: number
  intervalSecondsDefault: number
  archiveRoot: string
}

export interface DesktopTaskRequest {
  mode: 'keyword' | 'detail' | 'creator'
  keyword: string
  forumName: string
  targets: string
  maxPosts: number
  maxComments: number
  intervalSeconds: number
  includeComments: boolean
  includeNested: boolean
  downloadMedia: boolean
  browserMode: 'existing' | 'managed'
}

export interface CrawlerEvent {
  type: 'log' | 'progress' | 'note' | 'comment' | 'attachment' | 'finished' | 'error' | 'browser_status'
  taskId?: string
  time?: string
  level?: string
  message?: string
  status?: string
  browserMode?: 'existing' | 'managed'
  data?: Record<string, unknown>
}

export const isDesktopRuntime = () => typeof window !== 'undefined' && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)

export const desktopApi = {
  getTasks: () => invoke<Record<string, unknown>[]>('get_tasks'),
  getPosts: (query = '') => invoke<Record<string, unknown>[]>('get_posts', { query }),
  getComments: (query = '') => invoke<Record<string, unknown>[]>('get_comments', { query }),
  getAttachments: (query = '') => invoke<Record<string, unknown>[]>('get_attachments', { query }),
  getLogs: () => invoke<Record<string, unknown>[]>('get_logs'),
  getSettings: () => invoke<DesktopSettings>('get_settings'),
  saveSettings: (settings: Partial<DesktopSettings>) => invoke<void>('save_settings', { settings }),
  startTask: (request: DesktopTaskRequest) => invoke<Record<string, unknown>>('start_task', { request }),
  stopTask: (taskId: string) => invoke<void>('stop_task', { taskId }),
  browserAction: (action: 'check' | 'open_login', browserMode: 'existing' | 'managed') => invoke<void>('browser_action', { action, browserMode }),
  openArchive: (taskId?: string) => invoke<void>('open_archive', { taskId }),
  exportData: (scope: 'posts' | 'comments' | 'media' | 'logs', format: 'CSV' | 'Excel' | 'JSON' | 'JSONL', query = '') => invoke<string>('export_data', { scope, format, query }),
  openSourceUrl: (url: string) => invoke<void>('open_source_url', { url }),
  listen: (handler: (event: CrawlerEvent) => void): Promise<UnlistenFn> => listen<CrawlerEvent>('crawler:event', (event) => handler(event.payload)),
  toLocalUrl: (path: string) => convertFileSrc(path),
}
