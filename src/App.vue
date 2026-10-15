<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import {
  ArrowDownToLine,
  ArrowRight,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Clock3,
  Database,
  Download,
  ExternalLink,
  FileJson,
  FileSpreadsheet,
  FileText,
  Filter,
  FolderOpen,
  Image,
  Info,
  Link2,
  ListChecks,
  ListFilter,
  LoaderCircle,
  MessageCircle,
  MessageSquare,
  MessageSquareText,
  Monitor,
  MoreHorizontal,
  Play,
  Search,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Square,
  UserRound,
  Video,
  X,
} from 'lucide-vue-next'
import { desktopApi, isDesktopRuntime, type CrawlerEvent, type DesktopSettings } from './api'

type Page = 'tasks' | 'data' | 'logs' | 'settings'
type CrawlMode = 'keyword' | 'detail' | 'creator'
type TaskStatus = '已完成' | '运行中' | '已停止' | '失败'
type LogLevel = '信息' | '成功' | '警告' | '错误'
type DataTab = '帖子' | '评论' | '媒体'

interface MockTask {
  id: string
  name: string
  mode: CrawlMode
  status: TaskStatus
  time: string
  posts: number
  comments: number
  progress: number
}

interface MockPost {
  id: string
  forum: string
  title: string
  body: string
  author: string
  time: string
  replies: number
  media: number
  keyword: string
  color: string
  url?: string
  forumId?: string
}

interface MockComment {
  id: string
  postId: string
  author: string
  body: string
  time: string
  floor: string
  nested: number
  parentId?: string
  url?: string
}

interface MockAttachment {
  id: string
  postId: string
  commentId?: string
  kind: string
  sourceUrl: string
  relativePath: string
  localPath?: string
  previewUrl?: string
  status: string
  fileName?: string
}

interface MockLog {
  id: number
  time: string
  level: LogLevel
  message: string
}

const navItems = [
  { id: 'tasks' as const, label: '采集任务', icon: ListChecks },
  { id: 'data' as const, label: '数据中心', icon: Database },
  { id: 'logs' as const, label: '运行日志', icon: FileText },
  { id: 'settings' as const, label: '设置', icon: Settings },
]

const modeOptions = [
  { id: 'keyword' as const, title: '关键词搜索', detail: '按关键词发现帖子', icon: Search },
  { id: 'detail' as const, title: '指定帖子', detail: '通过链接或 ID 采集', icon: Link2 },
  { id: 'creator' as const, title: '吧主主页', detail: '采集主页公开内容', icon: UserRound },
]

const initialPosts: MockPost[] = [
  {
    id: 'p-10563082417',
    forum: '城市徒步吧',
    title: '周末沿着运河走了一段，发现这条路线特别适合新手',
    body: '从桥西历史街区出发，沿着运河慢慢走到小河公园，全程大约 6 公里。路很好走，沿途有不少可以休息的地方。',
    author: '行***记',
    time: '今天 14:18',
    replies: 36,
    media: 3,
    keyword: '城市徒步',
    color: 'river',
  },
  {
    id: 'p-10563079102',
    forum: '杭州吧',
    title: '第一次去湘湖骑行，环湖路线现在开放到哪里了？',
    body: '想带家人一起骑一圈，求最近去过的朋友分享下路况和停车信息。',
    author: '小***舟',
    time: '今天 13:52',
    replies: 18,
    media: 1,
    keyword: '杭州骑行',
    color: 'lake',
  },
  {
    id: 'p-10563074563',
    forum: '摄影吧',
    title: '想找几处杭州日落拍摄点，最好公共交通能到',
    body: '平时用微单拍风光，最近天气不错，想约朋友周末出去拍日落。',
    author: '胶***片',
    time: '今天 12:37',
    replies: 52,
    media: 2,
    keyword: '杭州日落',
    color: 'sunset',
  },
  {
    id: 'p-10563069128',
    forum: '城市徒步吧',
    title: '北高峰从哪条路上去比较轻松？',
    body: '最近想开始爬山锻炼，第一次去北高峰，想选一条坡度平缓一点的路线。',
    author: '慢***步',
    time: '今天 11:05',
    replies: 24,
    media: 0,
    keyword: '城市徒步',
    color: 'forest',
  },
  {
    id: 'p-10563062203',
    forum: '杭州吧',
    title: '良渚周边有没有适合半天散步的路线推荐？',
    body: '想避开人多的热门景点，找一个可以轻松走走、顺便吃饭的地方。',
    author: '阿***禾',
    time: '昨天 20:42',
    replies: 11,
    media: 1,
    keyword: '杭州徒步',
    color: 'garden',
  },
  {
    id: 'p-10563050771',
    forum: '户外运动吧',
    title: '轻量徒步鞋怎么选？预算五百以内，主要走城市绿道',
    body: '不需要特别专业的款式，希望防滑、透气，平时通勤也能穿。',
    author: '山***风',
    time: '昨天 18:16',
    replies: 67,
    media: 0,
    keyword: '装备推荐',
    color: 'stone',
  },
]

const initialComments: MockComment[] = [
  { id: 'c-01', postId: 'p-10563082417', author: '晴***天', body: '我上周也走了这段，靠近小河直街那边风景很好。', time: '今天 14:26', floor: '2 楼', nested: 3 },
  { id: 'c-02', postId: 'p-10563082417', author: '运***河', body: '请问全程大概需要多长时间？中间有补水点吗？', time: '今天 14:31', floor: '3 楼', nested: 1 },
  { id: 'c-03', postId: 'p-10563079102', author: '骑***鱼', body: '湘湖三期可以正常绕行，靠近越王城那段有施工指示牌。', time: '今天 14:02', floor: '2 楼', nested: 2 },
  { id: 'c-04', postId: 'p-10563074563', author: '光***影', body: '钱塘江城市阳台视野很开阔，地铁过去也方便。', time: '今天 13:04', floor: '4 楼', nested: 0 },
  { id: 'c-05', postId: 'p-10563069128', author: '山***间', body: '建议从灵隐那边上去，台阶比较规整，慢慢走不累。', time: '今天 11:21', floor: '2 楼', nested: 2 },
  { id: 'c-06', postId: 'p-10563082417', author: '沿***路', body: '路线收藏了，感谢分享！周末准备带相机去拍一拍。', time: '今天 14:38', floor: '5 楼', nested: 0 },
]

const initialTasks: MockTask[] = [
  { id: 'TB-20261015-004', name: '城市徒步 · 关键词搜索', mode: 'keyword', status: '已完成', time: '今天 14:32', posts: 18, comments: 236, progress: 100 },
  { id: 'TB-20261015-003', name: '杭州吧 · 关键词搜索', mode: 'keyword', status: '已完成', time: '今天 11:08', posts: 12, comments: 104, progress: 100 },
  { id: 'TB-20261014-002', name: '指定帖子 · 详情采集', mode: 'detail', status: '已停止', time: '昨天 16:41', posts: 3, comments: 28, progress: 42 },
]

const initialLogs: MockLog[] = [
  { id: 7, time: '14:32:48', level: '成功', message: '演示任务已完成，共整理 18 篇帖子和 236 条回复' },
  { id: 6, time: '14:32:41', level: '信息', message: '正在保存“城市徒步吧”第 3 页帖子详情' },
  { id: 5, time: '14:32:34', level: '信息', message: '评论楼层整理完成，正在检查媒体附件' },
  { id: 4, time: '14:32:26', level: '信息', message: '发现 2 个图片附件，开始演示归档' },
  { id: 3, time: '14:32:19', level: '警告', message: '1 个媒体链接不可用，已保留来源链接' },
  { id: 2, time: '14:32:08', level: '信息', message: '已连接演示浏览器会话' },
  { id: 1, time: '14:31:52', level: '信息', message: '任务初始化完成，采集间隔 1.5 秒' },
]

const page = ref<Page>('tasks')
const demoMode = ref(!isDesktopRuntime())
const sidebarCollapsed = ref(true)
const mode = ref<CrawlMode>('keyword')
const dataTab = ref<DataTab>('帖子')
const logFilter = ref<'全部' | LogLevel>('全部')
const keyword = ref('城市徒步')
const forumName = ref('城市徒步吧')
const targetText = ref('')
const maxPosts = ref(20)
const maxComments = ref(50)
const intervalSeconds = ref(1.5)
const includeComments = ref(true)
const includeNested = ref(true)
const downloadMedia = ref(true)
const browserMode = ref<'existing' | 'managed'>('existing')
const connectionState = ref<'unknown' | 'connected' | 'failed'>('unknown')
const archiveRoot = ref('G:\\OS\\TiebaCrawler\\data\\tasks')
const settingsMaxPosts = ref(20)
const settingsMaxComments = ref(50)
const settingsInterval = ref(1.5)
const showDetail = ref(false)
const selectedPost = ref<MockPost | null>(null)
const dataQuery = ref('')
const logQuery = ref('')
const exportMenuOpen = ref(false)
const simulateErrorNext = ref(false)
const toast = reactive({ visible: false, kind: 'info' as 'info' | 'success' | 'warning' | 'error', message: '' })

const posts = ref<MockPost[]>([...initialPosts])
const comments = ref<MockComment[]>([...initialComments])
const tasks = ref<MockTask[]>([...initialTasks])
const logs = ref<MockLog[]>([...initialLogs])
const attachments = ref<MockAttachment[]>([])
const activeTask = ref<MockTask | null>(null)
const taskTimer = ref<number | undefined>()
const toastTimer = ref<number | undefined>()
const refreshTimer = ref<number | undefined>()
let unlistenCrawler: (() => void) | undefined
let nextLogId = 8

const pageTitle = computed(() => navItems.find((item) => item.id === page.value)?.label ?? '采集任务')
const isRunning = computed(() => activeTask.value?.status === '运行中')
const taskSummary = computed(() => [
  { label: '运行中', value: tasks.value.filter((task) => task.status === '运行中').length },
  { label: '已完成', value: tasks.value.filter((task) => task.status === '已完成').length },
  { label: '失败', value: tasks.value.filter((task) => task.status === '失败').length },
  { label: '已停止', value: tasks.value.filter((task) => task.status === '已停止').length },
])
const latestCompleted = computed(() => tasks.value.find((task) => task.status === '已完成'))
const filteredPosts = computed(() => {
  const query = dataQuery.value.trim().toLowerCase()
  if (!query) return posts.value
  return posts.value.filter((post) => [post.title, post.body, post.forum, post.keyword].some((value) => value.toLowerCase().includes(query)))
})
const filteredComments = computed(() => {
  const query = dataQuery.value.trim().toLowerCase()
  if (!query) return comments.value
  return comments.value.filter((comment) => [comment.body, comment.author, comment.postId].some((value) => value.toLowerCase().includes(query)))
})
const filteredLogs = computed(() => {
  const query = logQuery.value.trim().toLowerCase()
  return logs.value.filter((entry) => {
    const matchesLevel = logFilter.value === '全部' || entry.level === logFilter.value
    const matchesQuery = !query || entry.message.toLowerCase().includes(query)
    return matchesLevel && matchesQuery
  })
})
const selectedComments = computed(() => selectedPost.value ? comments.value.filter((comment) => comment.postId === selectedPost.value?.id) : [])
const selectedAttachments = computed(() => selectedPost.value ? attachments.value.filter((attachment) => attachment.postId === selectedPost.value?.id) : [])
const pageTypes = computed(() => [
  { label: '帖子', value: filteredPosts.value.length, icon: FileText },
  { label: '评论', value: filteredComments.value.length, icon: MessageSquareText },
  { label: '媒体', value: demoMode.value ? posts.value.reduce((sum, post) => sum + post.media, 0) : attachments.value.length, icon: Image },
])
const filteredMedia = computed(() => {
  const query = dataQuery.value.trim().toLowerCase()
  return attachments.value.filter((attachment) => {
    if (!query) return true
    const post = posts.value.find((item) => item.id === attachment.postId)
    return [attachment.fileName ?? '', attachment.kind, attachment.sourceUrl, post?.title ?? '', post?.forum ?? ''].some((value) => value.toLowerCase().includes(query))
  })
})

function asText(value: unknown, fallback = '') {
  return typeof value === 'string' ? value : value == null ? fallback : String(value)
}

function asNumber(value: unknown, fallback = 0) {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

function toTask(row: Record<string, unknown>): MockTask {
  return {
    id: asText(row.id), name: asText(row.name), mode: (asText(row.mode, 'keyword') as CrawlMode),
    status: asText(row.status, '运行中') as TaskStatus, time: asText(row.time),
    posts: asNumber(row.posts), comments: asNumber(row.comments), progress: asNumber(row.progress),
  }
}

function toPost(row: Record<string, unknown>): MockPost {
  return {
    id: asText(row.id), forum: asText(row.forum, '贴吧'), title: asText(row.title, '无标题帖子'),
    body: asText(row.body), author: asText(row.author, '匿名用户'), time: asText(row.time),
    replies: asNumber(row.replies), media: asNumber(row.media), keyword: asText(row.keyword),
    color: asText(row.color, 'stone'), url: asText(row.url), forumId: asText(row.forumId),
  }
}

function toComment(row: Record<string, unknown>): MockComment {
  return {
    id: asText(row.id), postId: asText(row.postId), author: asText(row.author, '匿名用户'),
    body: asText(row.body), time: asText(row.time), floor: asText(row.floor, '回复'),
    nested: asNumber(row.nested), parentId: asText(row.parentId), url: asText(row.url),
  }
}

async function refreshDesktopData() {
  if (demoMode.value) return
  const [taskRows, postRows, commentRows, mediaRows, logRows, settings] = await Promise.all([
    desktopApi.getTasks(), desktopApi.getPosts(), desktopApi.getComments(), desktopApi.getAttachments(), desktopApi.getLogs(), desktopApi.getSettings(),
  ])
  tasks.value = taskRows.map(toTask)
  posts.value = postRows.map(toPost)
  comments.value = commentRows.map(toComment)
  attachments.value = mediaRows.map((row) => {
    const localPath = asText(row.localPath)
    return {
      id: asText(row.id), postId: asText(row.postId), commentId: asText(row.commentId),
      kind: asText(row.kind, '图片'), sourceUrl: asText(row.sourceUrl), relativePath: asText(row.relativePath),
      localPath, previewUrl: localPath ? desktopApi.toLocalUrl(localPath) : '',
      status: asText(row.status, '待下载'), fileName: asText(row.fileName),
    }
  })
  logs.value = logRows.map((row) => ({ id: asNumber(row.id), time: asText(row.time), level: asText(row.level, '信息') as LogLevel, message: asText(row.message) }))
  const saved = settings as DesktopSettings
  browserMode.value = saved.browserMode ?? 'existing'
  archiveRoot.value = saved.archiveRoot || archiveRoot.value
  settingsMaxPosts.value = asNumber(saved.maxPostsDefault, 20)
  settingsMaxComments.value = asNumber(saved.maxCommentsDefault, 50)
  settingsInterval.value = asNumber(saved.intervalSecondsDefault, 1.5)
  maxPosts.value = settingsMaxPosts.value
  maxComments.value = settingsMaxComments.value
  intervalSeconds.value = settingsInterval.value
  activeTask.value = tasks.value.find((task) => task.status === '运行中') ?? null
}

function scheduleDesktopRefresh() {
  if (demoMode.value) return
  if (refreshTimer.value) window.clearTimeout(refreshTimer.value)
  refreshTimer.value = window.setTimeout(() => { void refreshDesktopData().catch((error) => showToast(`读取本地数据失败：${String(error)}`, 'error')) }, 300)
}

function handleCrawlerEvent(event: CrawlerEvent) {
  if (event.type === 'browser_status') {
    connectionState.value = event.status === 'connected' ? 'connected' : 'failed'
    showToast(event.message || (connectionState.value === 'connected' ? '浏览器连接成功' : '浏览器连接失败'), connectionState.value === 'connected' ? 'success' : 'warning')
    return
  }
  if (event.type === 'log' && event.message) {
    logs.value.unshift({ id: nextLogId++, time: event.time || new Date().toLocaleTimeString('zh-CN', { hour12: false }), level: (event.level || '信息') as LogLevel, message: event.message })
  }
  if (event.type === 'progress' && event.taskId) {
    const values = event.data ?? {}
    const task = tasks.value.find((item) => item.id === event.taskId)
    if (task) {
      task.progress = asNumber(values.progress, task.progress)
      task.posts = asNumber(values.posts, task.posts)
      task.comments = asNumber(values.comments, task.comments)
      activeTask.value = task
    }
  }
  if (event.type === 'finished' || event.type === 'error') {
    const task = tasks.value.find((item) => item.id === event.taskId)
    if (task) {
      const rawStatus = asText(event.data?.status, 'completed')
      task.status = event.type === 'error' ? '失败' : rawStatus === 'stopped' ? '已停止' : rawStatus === 'failed' ? '失败' : '已完成'
      if (task.status === '已完成') task.progress = 100
      activeTask.value = null
    }
    if (event.message) showToast(event.message, event.type === 'error' ? 'error' : 'success')
  }
  if (['note', 'comment', 'attachment', 'progress', 'finished', 'error'].includes(event.type)) scheduleDesktopRefresh()
}

function statusClass(status?: TaskStatus) {
  if (status === '运行中') return 'status-running'
  if (status === '已完成') return 'status-complete'
  if (status === '已停止') return 'status-stopped'
  if (status === '失败') return 'status-failed'
  return 'status-idle'
}

function showToast(message: string, kind: typeof toast.kind = 'info') {
  toast.message = message
  toast.kind = kind
  toast.visible = true
  if (toastTimer.value) window.clearTimeout(toastTimer.value)
  toastTimer.value = window.setTimeout(() => { toast.visible = false }, 3200)
}

function addLog(message: string, level: LogLevel = '信息') {
  const now = new Date()
  const time = `${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}:${String(now.getSeconds()).padStart(2, '0')}`
  logs.value.unshift({ id: nextLogId++, time, level, message })
}

function modeTargetPlaceholder() {
  if (mode.value === 'detail') return '粘贴帖子链接或帖子 ID，支持多行输入'
  if (mode.value === 'creator') return '粘贴吧主主页链接或用户 ID，支持多行输入'
  return ''
}

function makeTaskName() {
  if (mode.value === 'keyword') return `${keyword.value.trim() || forumName.value.trim() || '关键词'} · 关键词搜索`
  if (mode.value === 'detail') return `${targetText.value.split(/[\n,，]+/).filter(Boolean).length || 1} 个帖子 · 详情采集`
  return `${targetText.value.split(/[\n,，]+/).filter(Boolean).length || 1} 个主页 · 创作者采集`
}

function startDemoTask() {
  if (isRunning.value) return
  if (mode.value === 'keyword' && !keyword.value.trim()) {
    showToast('请先填写搜索关键词', 'warning')
    return
  }
  if (mode.value !== 'keyword' && !targetText.value.trim()) {
    showToast(mode.value === 'detail' ? '请填写帖子链接或 ID' : '请填写主页链接或用户 ID', 'warning')
    return
  }
  if (!Number.isInteger(maxPosts.value) || maxPosts.value < 1 || maxPosts.value > 100) {
    showToast('帖子数量需设置为 1 到 100 之间', 'warning')
    return
  }
  if (!Number.isInteger(maxComments.value) || maxComments.value < 1 || maxComments.value > 100) {
    showToast('每帖评论数量需设置为 1 到 100 之间', 'warning')
    return
  }
  if (intervalSeconds.value < 1.5) {
    showToast('请求间隔不能低于 1.5 秒', 'warning')
    return
  }

  if (!demoMode.value) {
    void desktopApi.startTask({
      mode: mode.value,
      keyword: keyword.value.trim(),
      forumName: forumName.value.trim(),
      targets: targetText.value.trim(),
      maxPosts: maxPosts.value,
      maxComments: maxComments.value,
      intervalSeconds: intervalSeconds.value,
      includeComments: includeComments.value,
      includeNested: includeNested.value,
      downloadMedia: downloadMedia.value,
      browserMode: browserMode.value,
    }).then((row) => {
      const task = toTask(row)
      activeTask.value = task
      tasks.value.unshift(task)
      addLog(`已启动本地采集任务：${task.name}`)
      showToast('采集任务已启动', 'success')
      scheduleDesktopRefresh()
    }).catch((error) => showToast(String(error), 'error'))
    return
  }

  const id = `TB-${new Date().toISOString().slice(0, 10).replaceAll('-', '')}-${String(Date.now()).slice(-3)}`
  activeTask.value = {
    id,
    name: makeTaskName(),
    mode: mode.value,
    status: '运行中',
    time: '刚刚',
    posts: 0,
    comments: 0,
    progress: 0,
  }
  tasks.value.unshift(activeTask.value)
  addLog(`开始演示任务：${activeTask.value.name}`)
  addLog(`任务参数：最多 ${maxPosts.value} 篇帖子、每帖 ${maxComments.value} 条评论，间隔 ${intervalSeconds.value} 秒`)
  showToast('模拟任务已启动', 'success')

  if (taskTimer.value) window.clearInterval(taskTimer.value)
  taskTimer.value = window.setInterval(() => {
    if (!activeTask.value || activeTask.value.status !== '运行中') return
    activeTask.value.progress = Math.min(100, activeTask.value.progress + 10)
    activeTask.value.posts = Math.min(maxPosts.value, Math.floor(activeTask.value.progress / 100 * Math.min(maxPosts.value, 18)))
    activeTask.value.comments = Math.floor(activeTask.value.posts * Math.min(maxComments.value, 13))
    if (activeTask.value.progress === 30) addLog('已发现帖子，正在整理楼层与评论')
    if (activeTask.value.progress === 60 && downloadMedia.value) addLog('正在检查帖子图片和视频附件')
    if (activeTask.value.progress >= 100) {
      window.clearInterval(taskTimer.value)
      taskTimer.value = undefined
      if (simulateErrorNext.value) {
        activeTask.value.status = '失败'
        addLog('演示错误：连接已中断，已保留已采集的数据', '错误')
        showToast('模拟任务失败；已保留部分结果', 'error')
        simulateErrorNext.value = false
      } else {
        activeTask.value.status = '已完成'
        activeTask.value.posts = Math.min(maxPosts.value, Math.max(activeTask.value.posts, 8))
        activeTask.value.comments = Math.max(activeTask.value.comments, Math.min(maxComments.value, 13) * activeTask.value.posts)
        addLog(`演示任务完成：${activeTask.value.posts} 篇帖子，${activeTask.value.comments} 条评论`, '成功')
        showToast('演示任务已完成', 'success')
      }
    }
  }, 420)
}

function stopDemoTask() {
  if (!activeTask.value || !isRunning.value) return
  if (!demoMode.value) {
    void desktopApi.stopTask(activeTask.value.id)
      .then(() => showToast('已发送停止请求，当前请求结束后会停止', 'warning'))
      .catch((error) => showToast(String(error), 'error'))
    return
  }
  if (taskTimer.value) window.clearInterval(taskTimer.value)
  taskTimer.value = undefined
  activeTask.value.status = '已停止'
  addLog('任务已由用户停止，已采集内容保留', '警告')
  showToast('演示任务已停止', 'warning')
}

function openPost(post: MockPost) {
  selectedPost.value = post
  showDetail.value = true
}

function openAttachment(attachment: MockAttachment) {
  const post = posts.value.find((item) => item.id === attachment.postId)
  if (post) openPost(post)
  else void desktopApi.openSourceUrl(attachment.sourceUrl).catch((error) => showToast(String(error), 'error'))
}

function openAttachmentSource(attachment: MockAttachment) {
  if (!attachment.sourceUrl) return
  void desktopApi.openSourceUrl(attachment.sourceUrl).catch((error) => showToast(String(error), 'error'))
}

function closeDetail() {
  showDetail.value = false
  selectedPost.value = null
}

function exportData(format: 'CSV' | 'Excel' | 'JSON' | 'JSONL') {
  exportMenuOpen.value = false
  if (dataTab.value === '帖子' && filteredPosts.value.length === 0) {
    showToast('当前没有可导出的帖子', 'warning')
    return
  }
  if (dataTab.value === '评论' && filteredComments.value.length === 0) {
    showToast('当前没有可导出的评论', 'warning')
    return
  }
  if (!demoMode.value) {
    const scope = dataTab.value === '评论' ? 'comments' : dataTab.value === '媒体' ? 'media' : 'posts'
    void desktopApi.exportData(scope, format, dataQuery.value.trim())
      .then((path) => showToast(`文件已导出：${path}`, 'success'))
      .catch((error) => showToast(String(error), 'error'))
    return
  }
  const rows = dataTab.value === '评论' ? filteredComments.value : filteredPosts.value
  const body = format === 'JSON' ? JSON.stringify(rows, null, 2) : format === 'JSONL' ? rows.map((row) => JSON.stringify(row)).join('\n') : rows.map((row) => Object.values(row).join(',')).join('\n')
  if (format !== 'Excel') {
    const blob = new Blob([body], { type: format === 'JSON' || format === 'JSONL' ? 'application/json' : 'text/csv;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = `贴吧采集演示.${format === 'JSONL' ? 'jsonl' : format.toLowerCase()}`
    anchor.click()
    URL.revokeObjectURL(url)
  }
  showToast(`${format} 演示导出已准备（内容为模拟数据）`, 'success')
}

function checkBrowserConnection() {
  if (!demoMode.value) {
    connectionState.value = 'unknown'
    void desktopApi.browserAction('check', browserMode.value)
      .then(() => showToast('正在检查浏览器连接…', 'info'))
      .catch((error) => { connectionState.value = 'failed'; showToast(String(error), 'error') })
    return
  }
  connectionState.value = browserMode.value === 'managed' ? 'connected' : 'failed'
  if (browserMode.value === 'managed') {
    showToast('演示连接成功：应用专用浏览器配置已就绪', 'success')
  } else {
    showToast('演示未连接：请先为 Chrome 开启远程调试', 'warning')
  }
  addLog(browserMode.value === 'managed' ? '演示浏览器配置检查成功' : 'Chrome 演示连接失败：未检测到远程调试服务', browserMode.value === 'managed' ? '成功' : '警告')
}

function openTiebaLogin() {
  if (demoMode.value) {
    showToast('浏览器登录将在 Tauri 桌面版中打开', 'info')
    return
  }
  void desktopApi.browserAction('open_login', browserMode.value)
    .then(() => showToast('正在打开贴吧，请在 Chrome 中手动登录', 'info'))
    .catch((error) => showToast(String(error), 'error'))
}

function savePreferences() {
  if (demoMode.value) {
    showToast('原型设置已保存到当前页面状态', 'success')
    return
  }
  void desktopApi.saveSettings({
    browserMode: browserMode.value,
    maxPostsDefault: settingsMaxPosts.value,
    maxCommentsDefault: settingsMaxComments.value,
    intervalSecondsDefault: settingsInterval.value,
  }).then(() => {
    maxPosts.value = settingsMaxPosts.value
    maxComments.value = settingsMaxComments.value
    intervalSeconds.value = settingsInterval.value
    showToast('设置已保存到本机', 'success')
    scheduleDesktopRefresh()
  }).catch((error) => showToast(String(error), 'error'))
}

function openArchive() {
  if (demoMode.value) {
    showToast('原型目录尚未连接真实文件系统', 'info')
    return
  }
  void desktopApi.openArchive().catch((error) => showToast(String(error), 'error'))
}

async function copyArchiveRoot() {
  try {
    await navigator.clipboard.writeText(archiveRoot.value)
    showToast('归档路径已复制', 'success')
  } catch {
    showToast('无法访问剪贴板，请手动选择路径', 'warning')
  }
}

function refreshLogs() {
  if (demoMode.value) {
    showToast('演示日志已刷新', 'success')
    return
  }
  void desktopApi.getLogs().then((rows) => {
    logs.value = rows.map((row) => ({ id: asNumber(row.id), time: asText(row.time), level: asText(row.level, '信息') as LogLevel, message: asText(row.message) }))
    showToast('日志已刷新', 'success')
  }).catch((error) => showToast(String(error), 'error'))
}

function downloadLogs() {
  if (demoMode.value) {
    showToast('演示日志未连接真实文件', 'info')
    return
  }
  void desktopApi.exportData('logs', 'JSONL', logQuery.value.trim()).then((path) => showToast(`日志已导出：${path}`, 'success')).catch((error) => showToast(String(error), 'error'))
}

function openPostSource(post: MockPost) {
  if (!post.url) {
    showToast('当前记录没有来源链接', 'warning')
    return
  }
  if (demoMode.value) {
    showToast('模拟记录没有可打开的真实来源', 'info')
    return
  }
  void desktopApi.openSourceUrl(post.url).catch((error) => showToast(String(error), 'error'))
}

function setPage(nextPage: Page) {
  page.value = nextPage
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && showDetail.value) closeDetail()
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault()
    page.value = 'data'
    window.setTimeout(() => document.querySelector<HTMLInputElement>('.data-search-input')?.focus(), 0)
  }
}

onMounted(async () => {
  window.addEventListener('keydown', handleKeydown)
  if (!demoMode.value) {
    try {
      unlistenCrawler = await desktopApi.listen(handleCrawlerEvent)
      await refreshDesktopData()
    } catch (error) {
      showToast(`本地工作区初始化失败：${String(error)}`, 'error')
    }
  }
})
onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown)
  unlistenCrawler?.()
  if (taskTimer.value) window.clearInterval(taskTimer.value)
  if (toastTimer.value) window.clearTimeout(toastTimer.value)
  if (refreshTimer.value) window.clearTimeout(refreshTimer.value)
})
</script>

<template>
  <div class="app-shell" :class="{ 'sidebar-collapsed': sidebarCollapsed }">
    <aside class="sidebar">
      <div class="brand-row">
        <div class="brand-mark" role="img" aria-label="贴吧"><span>吧</span></div>
        <div class="brand-copy">
          <div class="brand-title">贴吧采集助手</div>
          <div class="brand-subtitle">TIEBA WORKSPACE</div>
        </div>
      </div>

      <div class="workspace-select" title="本地工作区 · 仅此设备可见">
        <span class="workspace-dot"></span>
        <div class="workspace-name"><strong>本地工作区</strong><small>仅此设备可见</small></div>
        <ChevronDown class="workspace-chevron" :size="15" />
      </div>

      <div class="nav-label">工作台</div>
      <nav class="side-nav" aria-label="主导航">
        <button
          v-for="item in navItems"
          :key="item.id"
          class="nav-item"
          :class="{ active: page === item.id }"
          :aria-current="page === item.id ? 'page' : undefined"
          :title="sidebarCollapsed ? item.label : undefined"
          @click="setPage(item.id)"
        >
          <component :is="item.icon" :size="18" :stroke-width="1.75" />
          <span>{{ item.label }}</span>
          <span v-if="item.id === 'tasks' && isRunning" class="nav-pulse"></span>
        </button>
      </nav>

      <div class="sidebar-spacer"></div>
      <div class="sidebar-bottom">
        <button
          class="icon-button sidebar-collapse sidebar-bottom-collapse"
          :title="sidebarCollapsed ? '展开侧栏' : '折叠侧栏'"
          :aria-label="sidebarCollapsed ? '展开侧栏' : '折叠侧栏'"
          :aria-expanded="!sidebarCollapsed"
          @click="sidebarCollapsed = !sidebarCollapsed"
        ><ChevronRight :size="16" /></button>
      </div>
    </aside>

    <main class="main-shell">
      <header class="topbar">
        <div class="breadcrumbs"><span>贴吧工作台</span><ChevronRight :size="14" /><strong>{{ pageTitle }}</strong></div>
        <div class="topbar-actions">
          <div class="demo-pill" :class="{ 'live-pill': !demoMode }"><span class="demo-pill-dot"></span>{{ demoMode ? '交互原型' : '本地采集' }}</div>
          <button class="icon-button topbar-help" :title="demoMode ? '原型说明' : '采集说明'" :aria-label="demoMode ? '原型说明' : '采集说明'" @click="showToast(demoMode ? '当前为交互原型，所有任务与数据均为模拟内容' : '仅整理公开内容；登录在 Chrome 中手动完成，任务和数据保存在本机', 'info')"><CircleAlert :size="17" /></button>
        </div>
      </header>

      <div class="content-scroll">
        <div class="content-wrap">
          <div class="prototype-notice">
            <div class="notice-left"><span class="notice-icon"><Info :size="15" /></span><span><strong>{{ demoMode ? '原型演示' : '本地采集' }}</strong><span class="notice-separator">·</span>{{ demoMode ? '不会启动真实采集，页面数据为虚构样例' : '任务和媒体存储在本机；仅采集贴吧公开内容' }}</span></div>
            <button class="notice-link" @click="setPage('settings')">查看使用说明 <ArrowRight :size="14" /></button>
          </div>

          <template v-if="page === 'tasks'">
            <div class="page-heading task-heading">
              <div>
                <div class="eyebrow"><span class="eyebrow-line"></span>数据采集</div>
                <h1>采集任务</h1>
                <p>设置采集范围，整理贴吧公开内容与讨论。</p>
              </div>
              <button class="button button-secondary" @click="setPage('data')"><Database :size="16" />查看数据中心</button>
            </div>

            <section class="task-overview-strip surface" aria-label="任务状态统计">
              <div v-for="item in taskSummary" :key="item.label" class="task-overview-item">
                <span>{{ item.label }}</span><strong>{{ item.value }}</strong><small>项</small>
              </div>
            </section>

            <div class="task-layout">
              <section class="surface task-form-card">
                <div class="card-heading">
                  <div><h2>新建采集</h2><p>选择一种方式开始整理公开内容</p></div>
                  <span class="step-count"><span>01</span> / 03</span>
                </div>
                <form @submit.prevent="startDemoTask">
                  <div class="section-caption"><span>采集方式</span><span class="caption-note">选择一种采集方式</span></div>
                  <div class="mode-grid" role="tablist" aria-label="采集方式">
                    <button v-for="item in modeOptions" :key="item.id" type="button" class="mode-option" :class="{ selected: mode === item.id }" role="tab" :aria-selected="mode === item.id" @click="mode = item.id">
                      <span class="mode-icon"><component :is="item.icon" :size="17" /></span>
                      <span class="mode-copy"><strong>{{ item.title }}</strong><small>{{ item.detail }}</small></span>
                      <span class="mode-check"><Check :size="12" /></span>
                    </button>
                  </div>

                  <div class="form-fields">
                    <template v-if="mode === 'keyword'">
                      <label class="field-block">
                        <span class="field-label">搜索关键词 <span class="required-mark">*</span><span class="field-label-side">支持逗号或换行分隔</span></span>
                        <div class="input-wrap"><Search :size="17" /><input v-model.trim="keyword" type="text" placeholder="例如：城市徒步、周末露营" autocomplete="off" /></div>
                      </label>
                      <label class="field-block">
                        <span class="field-label">限定贴吧 <span class="optional-mark">选填</span></span>
                        <div class="input-wrap"><MessageCircle :size="17" /><input v-model.trim="forumName" type="text" placeholder="例如：杭州吧" autocomplete="off" /><span class="input-trailing">吧</span></div>
                      </label>
                    </template>
                    <template v-else>
                      <label class="field-block">
                        <span class="field-label">{{ mode === 'detail' ? '帖子链接或 ID' : '主页链接或用户 ID' }} <span class="required-mark">*</span><span class="field-label-side">支持多条，按行输入</span></span>
                        <textarea v-model.trim="targetText" class="textarea-control" :placeholder="modeTargetPlaceholder()" rows="3"></textarea>
                        <span class="field-footnote"><Link2 :size="13" /> {{ mode === 'detail' ? '支持 tieba.baidu.com/p/ 链接' : '支持贴吧个人主页链接' }}</span>
                      </label>
                    </template>
                  </div>

                  <div class="section-caption limits-caption"><span>采集范围</span><span class="caption-note">轻量默认值 · 上限 100</span></div>
                  <div class="limit-grid">
                    <label class="number-field"><span>最多帖子</span><div class="number-input"><input v-model.number="maxPosts" type="number" min="1" max="100" /><span>篇</span></div><small>1–100 篇</small></label>
                    <label class="number-field"><span>每帖最多评论</span><div class="number-input"><input v-model.number="maxComments" type="number" min="1" max="100" /><span>条</span></div><small>1–100 条</small></label>
                    <label class="number-field"><span>请求间隔</span><div class="number-input"><input v-model.number="intervalSeconds" type="number" min="1.5" step="0.5" /><span>秒</span></div><small>不得低于 1.5 秒</small></label>
                  </div>

                  <div class="section-caption collection-caption"><span>采集内容</span></div>
                  <div class="toggle-list">
                    <label class="toggle-row"><span class="toggle-icon"><MessageSquare :size="16" /></span><span class="toggle-copy"><strong>采集评论</strong><small>收集帖子楼层与讨论内容</small></span><input v-model="includeComments" type="checkbox" class="toggle-input" /><span class="toggle-switch"></span></label>
                    <label class="toggle-row" :class="{ disabled: !includeComments }"><span class="toggle-icon"><MessageCircle :size="16" /></span><span class="toggle-copy"><strong>采集二级回复</strong><small>包含楼中楼对话</small></span><input v-model="includeNested" type="checkbox" class="toggle-input" :disabled="!includeComments" /><span class="toggle-switch"></span></label>
                    <label class="toggle-row"><span class="toggle-icon"><Image :size="16" /></span><span class="toggle-copy"><strong>下载图片与视频</strong><small>主帖、楼层及子回复附件</small></span><input v-model="downloadMedia" type="checkbox" class="toggle-input" /><span class="toggle-switch"></span></label>
                  </div>

                  <div class="form-browser-note">
                    <span class="browser-note-icon"><Monitor :size="15" /></span>
                    <span>登录方式：<strong>{{ browserMode === 'existing' ? '连接 Chrome（CDP）' : '应用专用浏览器' }}</strong></span>
                    <button type="button" @click="setPage('settings')">更改设置 <ArrowRight :size="13" /></button>
                  </div>

                  <div class="form-actions">
                    <button v-if="!isRunning" class="button button-primary start-button" type="submit"><Play :size="16" fill="currentColor" />{{ demoMode ? '开始模拟任务' : '开始采集' }}</button>
                    <button v-else class="button button-stop start-button" type="button" @click="stopDemoTask"><Square :size="15" fill="currentColor" />停止任务</button>
                    <button v-if="demoMode" class="button button-quiet" type="button" :aria-pressed="simulateErrorNext" @click="simulateErrorNext = !simulateErrorNext"><CircleAlert :size="15" />{{ simulateErrorNext ? '下次将模拟异常' : '体验异常状态' }}</button>
                  </div>
                </form>
              </section>

              <div class="task-side-column">
                <section class="surface live-card">
                  <div class="card-heading live-card-heading">
                    <div><h2>任务状态</h2><p>当前工作区运行概况</p></div>
                    <span class="status-badge" :class="statusClass(activeTask?.status)"><span class="status-dot"></span>{{ activeTask?.status ?? '待命' }}</span>
                  </div>
                  <div v-if="activeTask" class="active-task-panel">
                    <div class="active-task-title"><span class="task-type-icon"><component :is="modeOptions.find((item) => item.id === activeTask?.mode)?.icon" :size="16" /></span><div><strong>{{ activeTask.name }}</strong><small>{{ activeTask.id }}</small></div><button class="icon-button more-button" aria-label="更多任务操作" @click="showToast('原型中暂不提供更多任务操作', 'info')"><MoreHorizontal :size="18" /></button></div>
                    <div class="progress-meta"><span>{{ activeTask.status === '运行中' ? '正在采集' : activeTask.status === '失败' ? '任务中断' : activeTask.status === '已停止' ? '已手动停止' : '采集完成' }}</span><strong>{{ activeTask.progress }}%</strong></div>
                    <div class="progress-track"><span class="progress-fill" :class="{ failed: activeTask.status === '失败' }" :style="{ width: `${activeTask.progress}%` }"></span></div>
                    <div class="task-stats-grid">
                      <div><span>帖子</span><strong>{{ activeTask.posts }}</strong><small>篇</small></div>
                      <div><span>评论</span><strong>{{ activeTask.comments }}</strong><small>条</small></div>
                      <div><span>附件</span><strong>{{ demoMode ? (activeTask.posts > 0 ? Math.min(activeTask.posts, 7) : 0) : attachments.length }}</strong><small>个</small></div>
                    </div>
                    <div class="live-log-head"><span><span class="live-indicator"></span>最近动态</span><button @click="setPage('logs')">全部日志 <ArrowRight :size="13" /></button></div>
                    <div class="mini-log-list">
                      <div v-for="entry in logs.slice(0, 3)" :key="entry.id" class="mini-log-row"><span class="mini-log-time">{{ entry.time }}</span><span class="mini-log-mark" :class="`level-${entry.level}`"></span><span class="mini-log-message">{{ entry.message }}</span></div>
                    </div>
                  </div>
                  <div v-else class="idle-panel">
                    <div class="idle-illustration"><div class="idle-icon"><ListChecks :size="23" /></div></div>
                    <strong>准备好开始采集</strong>
                    <p>填写左侧采集条件后，<br />这里会显示实时任务进度。</p>
                    <div class="idle-divider"></div>
                    <div class="idle-steps"><span><CheckCircle2 :size="14" />选择采集方式</span><ChevronRight :size="13" /><span><CheckCircle2 :size="14" />设置采集范围</span><ChevronRight :size="13" /><span><Play :size="12" />启动任务</span></div>
                  </div>
                  <div v-if="!activeTask && (latestCompleted || demoMode)" class="recent-completed"><div><span class="tiny-success"><Check :size="11" /></span><span>最近完成</span></div><strong>{{ latestCompleted?.name ?? '城市徒步 · 关键词搜索' }}</strong><small>{{ latestCompleted?.posts ?? 18 }} 篇帖子 <i>·</i> {{ latestCompleted?.comments ?? 236 }} 条评论 <i>·</i> {{ latestCompleted?.time ?? '今天 14:32' }}</small></div>
                </section>

                <section class="tip-card">
                  <div class="tip-icon"><ShieldCheck :size="17" /></div>
                  <div><strong>温和采集提醒</strong><p>请仅采集公开内容，并遵守贴吧服务条款。当前任务上限为 100 篇帖子。</p></div>
                  <button class="tip-more" title="查看说明" aria-label="查看采集说明" @click="setPage('settings')"><ChevronRight :size="16" /></button>
                </section>
              </div>
            </div>

            <section class="surface recent-tasks-card">
              <div class="section-card-heading"><div><h2>最近任务</h2><p>{{ demoMode ? '本机工作区中的模拟任务记录' : '本机 SQLite 中的任务历史' }}</p></div><button class="text-button" @click="setPage('logs')">查看运行日志 <ArrowRight :size="14" /></button></div>
              <div class="recent-task-table-wrap">
                <table class="recent-task-table">
                  <thead><tr><th>任务名称</th><th>采集方式</th><th>状态</th><th>帖子</th><th>评论</th><th>时间</th><th><span class="sr-only">操作</span></th></tr></thead>
                  <tbody>
                    <tr v-for="task in tasks.slice(0, 4)" :key="task.id">
                      <td><div class="task-name-cell"><span class="tiny-task-icon"><MessageCircle :size="14" /></span><span><strong>{{ task.name }}</strong><small>{{ task.id }}</small></span></div></td>
                      <td><span class="mode-tag">{{ modeOptions.find((item) => item.id === task.mode)?.title }}</span></td>
                      <td><span class="table-status" :class="statusClass(task.status)"><span class="status-dot"></span>{{ task.status }}</span></td>
                      <td class="numeric-cell">{{ task.posts }}</td>
                      <td class="numeric-cell">{{ task.comments }}</td>
                      <td class="muted-cell">{{ task.time }}</td>
                      <td><button class="row-more" title="查看详情" aria-label="查看任务详情" @click="setPage('logs')"><MoreHorizontal :size="17" /></button></td>
                    </tr>
                  </tbody>
                </table>
              </div>
              <div class="table-card-footer"><span>显示 <strong>{{ Math.min(tasks.length, 4) }}</strong> 条任务记录</span><button class="text-button" @click="setPage('data')">打开数据中心 <ArrowRight :size="13" /></button></div>
            </section>
          </template>

          <template v-else-if="page === 'data'">
            <div class="page-heading">
              <div><div class="eyebrow"><span class="eyebrow-line"></span>本地归档</div><h1>数据中心</h1><p>浏览、筛选和导出采集到的帖子与讨论。</p></div>
              <div class="heading-actions"><button class="button button-secondary" @click="openArchive"><FolderOpen :size="16" />打开归档目录</button><div class="export-menu-wrap"><button class="button button-primary" @click="exportMenuOpen = !exportMenuOpen"><ArrowDownToLine :size="16" />导出数据<ChevronDown :size="14" /></button><div v-if="exportMenuOpen" class="export-menu"><button @click="exportData('CSV')"><FileText :size="15" />CSV 文件</button><button @click="exportData('Excel')"><FileSpreadsheet :size="15" />Excel 工作簿</button><button @click="exportData('JSON')"><FileJson :size="15" />JSON 文件</button><button @click="exportData('JSONL')"><FileJson :size="15" />JSONL 文件</button></div></div></div>
            </div>

            <div class="data-summary-grid">
              <div v-for="item in pageTypes" :key="item.label" class="surface summary-card"><span class="summary-icon" :class="`summary-${item.label}`"><component :is="item.icon" :size="17" /></span><div><span>{{ item.label }}记录</span><strong>{{ item.value.toLocaleString('zh-CN') }}</strong></div><small class="summary-change"><span>{{ demoMode ? '演示' : '本机' }}</span>{{ demoMode ? '样例' : 'SQLite' }}</small></div>
            </div>

            <section class="surface data-table-card">
              <div class="data-toolbar">
                <div class="segmented-tabs" role="tablist" aria-label="数据类型"><button v-for="tab in ['帖子', '评论', '媒体'] as DataTab[]" :key="tab" role="tab" :aria-selected="dataTab === tab" :class="{ active: dataTab === tab }" @click="dataTab = tab">{{ tab }}<span>{{ tab === '帖子' ? posts.length : tab === '评论' ? comments.length : pageTypes[2].value }}</span></button></div>
                <div class="data-filters"><label class="search-field"><Search :size="16" /><input v-model="dataQuery" class="data-search-input" type="search" :placeholder="dataTab === '评论' ? '搜索评论内容…' : dataTab === '媒体' ? '搜索附件、帖子或来源…' : '搜索标题、贴吧或关键词…'" /><kbd>Ctrl K</kbd></label><button class="filter-button" @click="showToast(demoMode ? '所有记录均来自演示任务' : '当前按关键词即时筛选本地归档', 'info')"><Filter :size="15" />筛选</button><button class="icon-button data-more" aria-label="更多数据操作" @click="showToast('可使用搜索框筛选当前记录', 'info')"><SlidersHorizontal :size="17" /></button></div>
              </div>

              <template v-if="dataTab === '帖子'">
                <div v-if="filteredPosts.length" class="data-table-wrap"><table class="data-table posts-table"><thead><tr><th>帖子内容</th><th>贴吧</th><th>作者</th><th>发布时间</th><th class="centered">回复</th><th>附件</th><th></th></tr></thead><tbody>
                  <tr v-for="post in filteredPosts" :key="post.id" class="clickable-row" tabindex="0" @click="openPost(post)" @keydown.enter="openPost(post)">
                    <td><div class="post-title-cell"><div class="post-thumb" :class="`thumb-${post.color}`"><span class="thumb-hill hill-a"></span><span class="thumb-hill hill-b"></span><span class="thumb-sun"></span></div><div><strong>{{ post.title }}</strong><small>{{ post.body }}</small></div></div></td>
                    <td><span class="forum-chip"><MessageCircle :size="12" />{{ post.forum }}</span></td>
                    <td><span class="author-cell"><span class="author-avatar">{{ post.author.slice(0, 1) }}</span>{{ post.author }}</span></td>
                    <td class="muted-cell nowrap">{{ post.time }}</td>
                    <td class="centered numeric-cell">{{ post.replies }}</td>
                    <td><span v-if="post.media" class="attachment-chip"><Image :size="13" />{{ post.media }} 个</span><span v-else class="muted-dash">—</span></td>
                    <td><button class="row-open" title="查看帖子详情" aria-label="查看帖子详情"><ChevronRight :size="16" /></button></td>
                  </tr>
                </tbody></table></div>
                <div v-else class="empty-state"><div class="empty-icon"><Search :size="22" /></div><strong>没有找到匹配的帖子</strong><p>换个标题、贴吧名称或关键词试试看。</p><button class="button button-secondary" @click="dataQuery = ''">清除搜索条件</button></div>
              </template>

              <template v-else-if="dataTab === '评论'">
                <div v-if="filteredComments.length" class="data-table-wrap"><table class="data-table comments-table"><thead><tr><th>楼层</th><th>评论内容</th><th>作者</th><th>所属帖子</th><th>时间</th><th>子回复</th></tr></thead><tbody>
                  <tr v-for="comment in filteredComments" :key="comment.id"><td><span class="floor-badge">{{ comment.floor }}</span></td><td class="comment-body-cell">{{ comment.body }}</td><td><span class="author-cell"><span class="author-avatar small">{{ comment.author.slice(0, 1) }}</span>{{ comment.author }}</span></td><td><button class="post-link" @click="openPost(posts.find((item) => item.id === comment.postId) ?? posts[0])">{{ posts.find((item) => item.id === comment.postId)?.title ?? '演示帖子' }}<ExternalLink :size="12" /></button></td><td class="muted-cell nowrap">{{ comment.time }}</td><td><span class="reply-count"><MessageCircle :size="13" />{{ comment.nested }}</span></td></tr>
                </tbody></table></div>
                <div v-else class="empty-state"><div class="empty-icon"><MessageSquare :size="22" /></div><strong>没有找到匹配的评论</strong><p>清除筛选条件查看全部演示评论。</p><button class="button button-secondary" @click="dataQuery = ''">清除搜索条件</button></div>
              </template>

              <template v-else>
                <div class="media-grid">
                  <template v-if="demoMode">
                    <button v-for="(post, index) in filteredPosts.filter((item) => item.media > 0)" :key="post.id" class="media-item" @click="openPost(post)">
                      <div class="media-preview" :class="`thumb-${post.color}`"><span class="thumb-hill hill-a"></span><span class="thumb-hill hill-b"></span><span class="thumb-sun"></span><span class="media-count"><Image :size="12" />{{ post.media }} 个附件</span></div>
                      <div class="media-item-info"><span class="media-kind"><Video v-if="index === 1" :size="13" /> <Image v-else :size="13" />{{ index === 1 ? '视频' : '图片' }}</span><strong>{{ post.title }}</strong><small>{{ post.forum }} <i>·</i> {{ post.time }}</small></div>
                    </button>
                    <div v-if="filteredPosts.filter((item) => item.media > 0).length === 0" class="empty-state media-empty"><div class="empty-icon"><Image :size="22" /></div><strong>没有找到媒体附件</strong><p>调整搜索条件，或查看所有帖子。</p><button class="button button-secondary" @click="dataQuery = ''">清除搜索条件</button></div>
                  </template>
                  <template v-else>
                    <button v-for="attachment in filteredMedia" :key="attachment.id" class="media-item" @click="openAttachment(attachment)">
                      <div class="media-preview attachment-preview">
                        <img v-if="attachment.previewUrl && attachment.kind === '图片'" :src="attachment.previewUrl" :alt="attachment.fileName || '贴吧图片附件'" loading="lazy" />
                        <video v-else-if="attachment.previewUrl && attachment.kind === '视频'" :src="attachment.previewUrl" preload="metadata" muted />
                        <span v-else class="attachment-placeholder"><Video v-if="attachment.kind === '视频'" :size="24" /><Image v-else :size="24" /><small>{{ attachment.status }}</small></span>
                        <span class="media-count"><CheckCircle2 v-if="attachment.status === '已下载'" :size="12" /><CircleAlert v-else :size="12" />{{ attachment.status }}</span>
                      </div>
                      <div class="media-item-info"><span class="media-kind"><Video v-if="attachment.kind === '视频'" :size="13" /><Image v-else :size="13" />{{ attachment.kind }}</span><strong>{{ posts.find((item) => item.id === attachment.postId)?.title ?? attachment.fileName ?? '贴吧附件' }}</strong><small>{{ posts.find((item) => item.id === attachment.postId)?.forum ?? '来源待关联' }} <i>·</i> {{ attachment.fileName || attachment.sourceUrl }}</small></div>
                    </button>
                    <div v-if="filteredMedia.length === 0" class="empty-state media-empty"><div class="empty-icon"><Image :size="22" /></div><strong>还没有媒体附件</strong><p>完成采集并开启“下载图片与视频”后，附件会显示在这里。</p><button class="button button-secondary" @click="dataQuery = ''">清除搜索条件</button></div>
                  </template>
                </div>
              </template>

              <div class="data-footer"><span>{{ demoMode ? '数据均为虚构样例，未连接真实采集结果' : '数据保存在本机 SQLite 数据库' }}</span><span>每任务归档 <i>·</i> 媒体路径使用相对路径</span></div>
            </section>
          </template>

          <template v-else-if="page === 'logs'">
            <div class="page-heading">
              <div><div class="eyebrow"><span class="eyebrow-line"></span>任务观测</div><h1>运行日志</h1><p>按时间查看任务状态和采集过程中的提示。</p></div>
              <button class="button button-secondary" @click="refreshLogs"><LoaderCircle :size="16" />刷新日志</button>
            </div>
            <div class="log-overview-grid">
              <div class="surface log-overview-card"><span class="log-overview-icon info-icon"><FileText :size="17" /></span><div><span>日志条数</span><strong>{{ logs.length }}</strong></div></div>
              <div class="surface log-overview-card"><span class="log-overview-icon success-icon"><CheckCircle2 :size="17" /></span><div><span>采集成功</span><strong>{{ logs.filter((item) => item.level === '成功').length }}</strong></div></div>
              <div class="surface log-overview-card"><span class="log-overview-icon warning-icon"><CircleAlert :size="17" /></span><div><span>待关注</span><strong>{{ logs.filter((item) => item.level === '警告' || item.level === '错误').length }}</strong></div></div>
              <div class="surface log-overview-card"><span class="log-overview-icon time-icon"><Clock3 :size="17" /></span><div><span>最近更新</span><strong class="small-overview-value">{{ logs[0]?.time ?? '—' }}</strong></div></div>
            </div>
            <section class="surface log-panel">
              <div class="log-toolbar"><div><h2>全部日志</h2><p>按时间倒序排列</p></div><div class="log-actions"><label class="search-field log-search"><Search :size="15" /><input v-model="logQuery" type="search" placeholder="搜索日志内容" /></label><label class="select-wrap"><ListFilter :size="15" /><select v-model="logFilter" aria-label="筛选日志级别"><option>全部</option><option>信息</option><option>成功</option><option>警告</option><option>错误</option></select><ChevronDown :size="13" /></label><button class="icon-button" title="下载日志" aria-label="下载日志" @click="downloadLogs"><Download :size="16" /></button></div></div>
              <div v-if="filteredLogs.length" class="log-table"><div v-for="entry in filteredLogs" :key="entry.id" class="log-row"><div class="log-time">{{ entry.time }}</div><div class="log-level" :class="`log-${entry.level}`"><span></span>{{ entry.level }}</div><div class="log-content">{{ entry.message }}</div><button class="log-detail-button" title="查看日志" aria-label="查看日志" @click="showToast(entry.message, entry.level === '错误' ? 'error' : 'info')"><MoreHorizontal :size="16" /></button></div></div>
              <div v-else class="empty-state compact-empty"><div class="empty-icon"><Search :size="20" /></div><strong>没有匹配的日志</strong><p>尝试调整关键字或日志级别。</p></div>
              <div class="log-panel-footer"><span><span class="live-indicator"></span>{{ demoMode ? '原型日志流已暂停' : '实时日志本机保存' }}</span><span>当前显示 {{ filteredLogs.length }} 条</span></div>
            </section>
          </template>

          <template v-else>
            <div class="page-heading">
              <div><div class="eyebrow"><span class="eyebrow-line"></span>工作区偏好</div><h1>设置</h1><p>管理浏览器登录方式、任务归档和采集边界。</p></div>
              <button class="button button-secondary" @click="savePreferences"><Check :size="16" />保存设置</button>
            </div>
            <div class="settings-layout">
              <div class="settings-main">
                <section class="surface settings-card">
                  <div class="settings-card-heading"><div class="settings-icon blue"><Monitor :size="18" /></div><div><h2>浏览器与登录</h2><p>选择贴吧登录状态的使用方式</p></div><span class="settings-step">01</span></div>
                  <div class="browser-choice-grid">
                    <button class="browser-choice" :class="{ selected: browserMode === 'existing' }" @click="browserMode = 'existing'; connectionState = 'unknown'">
                      <span class="choice-radio"><span></span></span><span class="browser-choice-icon"><Monitor :size="19" /></span><span class="browser-choice-copy"><strong>连接 Chrome（CDP）</strong><small>连接已开启远程调试的 Chrome</small></span><span class="recommended-tag">当前默认</span>
                    </button>
                    <button class="browser-choice" :class="{ selected: browserMode === 'managed' }" @click="browserMode = 'managed'; connectionState = 'unknown'">
                      <span class="choice-radio"><span></span></span><span class="browser-choice-icon"><FolderOpen :size="19" /></span><span class="browser-choice-copy"><strong>应用专用浏览器</strong><small>独立配置目录，首次使用需登录</small></span>
                    </button>
                  </div>
                  <div class="browser-info-box" :class="{ connected: connectionState === 'connected', failed: connectionState === 'failed' }">
                    <span class="browser-info-icon"><CircleAlert v-if="connectionState === 'failed'" :size="17" /><CheckCircle2 v-else-if="connectionState === 'connected'" :size="17" /><ShieldCheck v-else :size="17" /></span>
                    <div><strong>{{ connectionState === 'connected' ? '浏览器已连接' : connectionState === 'failed' ? '浏览器暂不可用' : browserMode === 'existing' ? '连接 Chrome（CDP）' : '应用专用浏览器' }}</strong><p>{{ connectionState === 'failed' ? (browserMode === 'existing' ? '请使用非默认 --user-data-dir 并以 9222 端口启动 Chrome，再重新检查。' : '请确认已安装 Chrome。应用专用浏览器首次打开后需要手动登录。') : browserMode === 'existing' ? '连接 127.0.0.1:9222。Chrome 136+ 必须使用非默认 --user-data-dir；请在该 Chrome 页面手动登录。' : '使用本机独立配置目录；首次使用请在打开的浏览器页面手动登录。' }}</p></div>
                    <div class="browser-action-buttons"><button class="button button-secondary check-button" @click="checkBrowserConnection">{{ connectionState === 'unknown' ? (demoMode ? '模拟检查' : '检查连接') : '重新检查' }}</button><button class="button button-small" @click="openTiebaLogin">打开贴吧</button></div>
                  </div>
                  <div class="settings-footnote"><ShieldCheck :size="14" /><span>{{ demoMode ? '本原型不会读取、显示或保存你的账号密码与 Cookie。' : '请在连接的 Chrome 页面自行登录；Chrome 136+ 需使用非默认配置目录。应用不会复制或保存密码与 Cookie。' }}</span></div>
                </section>

                <section class="surface settings-card">
                  <div class="settings-card-heading"><div class="settings-icon amber"><FolderOpen :size="18" /></div><div><h2>数据归档</h2><p>每个任务独立保存导出文件和媒体</p></div><span class="settings-step">02</span></div>
                  <label class="field-label settings-field-label">默认任务目录</label>
                  <div class="path-field"><span class="path-icon"><FolderOpen :size="16" /></span><code>{{ archiveRoot }}\{task_id}</code><button class="button button-small" @click="copyArchiveRoot">复制路径</button></div>
                  <div class="archive-preview"><div class="archive-folder"><FolderOpen :size="17" /><strong>{{ tasks[0]?.id ?? '暂无任务归档' }}</strong></div><div class="archive-children"><span><FileText :size="14" /> exports</span><span><Image :size="14" /> media</span><span><Database :size="14" /> task data</span></div></div>
                  <div class="settings-footnote"><CheckCircle2 :size="14" /><span>媒体导出包含相对路径，可从导出文件定位到对应任务的归档文件。</span></div>
                </section>

                <section class="surface settings-card">
                  <div class="settings-card-heading"><div class="settings-icon green"><SlidersHorizontal :size="18" /></div><div><h2>采集边界</h2><p>适用于新建任务的轻量默认值</p></div><span class="settings-step">03</span></div>
                  <div class="settings-limits-grid"><label><span class="setting-metric-label">默认帖子数</span><strong><input v-model.number="settingsMaxPosts" class="settings-number" type="number" min="1" max="100" /> <small>篇</small></strong><span class="setting-metric-cap">上限 100 篇 / 任务</span></label><label><span class="setting-metric-label">默认评论数</span><strong><input v-model.number="settingsMaxComments" class="settings-number" type="number" min="1" max="100" /> <small>条</small></strong><span class="setting-metric-cap">上限 100 条 / 帖</span></label><label><span class="setting-metric-label">最短请求间隔</span><strong><input v-model.number="settingsInterval" class="settings-number" type="number" min="1.5" step="0.5" /> <small>秒</small></strong><span class="setting-metric-cap">可调高，不可调低</span></label></div>
                  <div class="ethics-note"><span><ShieldCheck :size="16" /></span><p>仅用于个人学习与研究。请遵守贴吧服务条款，不采集非公开内容，不进行大规模采集。</p><button class="text-button" @click="showToast('已打开项目使用说明', 'info')">使用说明 <ArrowRight :size="13" /></button></div>
                </section>
              </div>
              <aside class="settings-aside">
                <section class="about-card"><div class="about-brand"><div class="brand-mark small-mark" role="img" aria-label="贴吧"><span>吧</span></div><span>贴吧采集助手</span></div><span class="version-chip">{{ demoMode ? '原型版 0.1.0' : '桌面版 0.2.0' }}</span><p>桌面端公开内容整理工具，数据存储在本机工作区。</p><div class="about-rule"></div><div class="about-meta"><span>运行平台</span><strong>Windows</strong></div><div class="about-meta"><span>界面语言</span><strong>简体中文</strong></div><div class="about-meta"><span>数据模式</span><strong>{{ demoMode ? '模拟数据' : 'SQLite + 本地文件' }}</strong></div><button class="about-link" @click="showToast(demoMode ? '当前是交互原型，所有样例均为虚构数据' : '采集任务、日志和附件均保存在本机工作区', 'info')">查看项目说明 <ArrowRight :size="14" /></button></section>
                <section class="privacy-card"><div class="privacy-icon"><ShieldCheck :size="19" /></div><strong>隐私优先</strong><p>{{ demoMode ? '本原型使用虚构样例，不会访问网络，也不会向任何服务上传内容。' : '登录由你在 Chrome 页面手动完成；采集内容和附件仅保存在本机。' }}</p><div class="privacy-check"><CheckCircle2 :size="14" />数据留在本机</div><div class="privacy-check"><CheckCircle2 :size="14" />创作者信息脱敏</div></section>
              </aside>
            </div>
          </template>

          <footer class="page-footer"><span>贴吧采集助手 <i>·</i> {{ demoMode ? '交互原型' : '本地桌面端' }}</span><span>部分贴吧实现 © relakkes@gmail.com · 非商业学习许可 1.1 · 禁止大规模采集</span></footer>
        </div>
      </div>
    </main>

    <Transition name="toast"><div v-if="toast.visible" class="toast-message" :class="`toast-${toast.kind}`" role="status"><CheckCircle2 v-if="toast.kind === 'success'" :size="17" /><CircleAlert v-else :size="17" /><span>{{ toast.message }}</span><button aria-label="关闭提示" @click="toast.visible = false"><X :size="15" /></button></div></Transition>

    <Transition name="modal">
      <div v-if="showDetail && selectedPost" class="modal-backdrop" @click.self="closeDetail">
        <section class="detail-modal" role="dialog" aria-modal="true" aria-labelledby="detail-title">
          <div class="detail-modal-header"><div><div class="eyebrow"><span class="eyebrow-line"></span>{{ selectedPost.forum }}</div><h2 id="detail-title">帖子详情</h2></div><button class="icon-button" aria-label="关闭详情" @click="closeDetail"><X :size="18" /></button></div>
          <div class="detail-modal-content">
            <div class="detail-post-meta"><span class="author-avatar">{{ selectedPost.author.slice(0, 1) }}</span><strong>{{ selectedPost.author }}</strong><span>{{ selectedPost.time }}</span><span class="forum-chip"><MessageCircle :size="12" />{{ selectedPost.forum }}</span></div>
            <h3>{{ selectedPost.title }}</h3><p class="detail-post-body">{{ selectedPost.body }}</p>
            <div v-if="demoMode && selectedPost.media" class="detail-media-grid"><div v-for="index in Math.min(selectedPost.media, 3)" :key="index" class="detail-media-thumb" :class="`thumb-${selectedPost.color}`"><span class="thumb-hill hill-a"></span><span class="thumb-hill hill-b"></span><span class="thumb-sun"></span><span class="detail-media-label"><Video v-if="index === 3" :size="13" /> <Image v-else :size="13" />{{ index === 3 ? '视频' : '图片' }}样例 {{ index }}</span></div></div>
            <div v-else-if="!demoMode && selectedAttachments.length" class="detail-media-grid actual-detail-media">
              <div v-for="attachment in selectedAttachments" :key="attachment.id" class="detail-media-thumb actual-detail-thumb">
                <img v-if="attachment.previewUrl && attachment.kind === '图片'" :src="attachment.previewUrl" :alt="attachment.fileName || '贴吧附件'" />
                <video v-else-if="attachment.previewUrl && attachment.kind === '视频'" :src="attachment.previewUrl" controls preload="metadata" />
                <div v-else class="attachment-placeholder"><Video v-if="attachment.kind === '视频'" :size="22" /><Image v-else :size="22" /><span>{{ attachment.status }}</span></div>
                <button class="detail-media-label" @click.stop="openAttachmentSource(attachment)"><ExternalLink :size="13" />{{ attachment.kind }} · {{ attachment.status }}</button>
              </div>
            </div>
            <div class="detail-stats"><span><MessageSquareText :size="14" />{{ selectedPost.replies }} 条回复</span><span><Image :size="14" />{{ selectedPost.media }} 个附件</span><span><ShieldCheck :size="14" />作者信息已脱敏</span></div>
            <div class="detail-comments-heading"><h4>楼层预览</h4><span>{{ selectedComments.length }} 条{{ demoMode ? '模拟' : '本机' }}评论</span></div>
            <div v-if="selectedComments.length" class="detail-comment-list"><div v-for="comment in selectedComments" :key="comment.id" class="detail-comment"><span class="author-avatar small">{{ comment.author.slice(0, 1) }}</span><div><div class="detail-comment-head"><strong>{{ comment.author }}</strong><span>{{ comment.floor }}</span><time>{{ comment.time }}</time></div><p>{{ comment.body }}</p><small v-if="comment.parentId" class="nested-reply"><MessageCircle :size="12" />楼中楼回复</small><small v-else-if="comment.nested" class="nested-reply"><MessageCircle :size="12" />{{ comment.nested }} 条子回复</small></div></div></div>
            <div v-else class="detail-empty-comments">当前帖子暂无{{ demoMode ? '模拟' : '已采集' }}评论。</div>
          </div>
          <div class="detail-modal-footer"><span>{{ demoMode ? '模拟帖子 ID：' : '帖子 ID：' }}{{ selectedPost.id }}</span><div class="detail-footer-actions"><button v-if="!demoMode" class="button button-secondary" @click="openPostSource(selectedPost)">打开原帖 <ExternalLink :size="14" /></button><button class="button button-primary" @click="closeDetail">完成 <Check :size="15" /></button></div></div>
        </section>
      </div>
    </Transition>
  </div>
</template>
