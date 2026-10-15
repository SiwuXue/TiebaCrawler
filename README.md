# 贴吧采集助手

基于 Vue 3、TypeScript、Tauri 2 和 Python sidecar 的 Windows 桌面工作台，用于个人非商业学习与研究场景下整理百度贴吧公开帖子、楼层回复和媒体附件。作者昵称脱敏；密码和 Cookie 不会被应用复制或落库。

> MediaCrawler 贴吧实现采用 **NON-COMMERCIAL LEARNING LICENSE 1.1**。本项目仅限非商业学习和研究使用，必须保留版权及许可声明，禁止大规模采集或干扰平台运行。详见 [`LICENSE`](./LICENSE) 与 [`NOTICE.md`](./NOTICE.md)。

## 开发环境

- Windows 10/11
- Node.js / npm
- Rust / Cargo 与 Tauri 所需的 WebView2 工具链
- `uv`（首次运行时会安装 Python 3.12 与 sidecar 依赖）
- Google Chrome

```powershell
npm install
npm run tauri:dev
```

普通 Vite 浏览器预览仍是演示模式，不连接采集服务：

```powershell
npm run dev
```

## Chrome 登录

- **连接 Chrome（CDP）**：Chrome 136 起，远程调试开关不能用于默认用户配置目录，必须同时传入独立的 `--user-data-dir`。例如在 PowerShell 中运行以下命令，再在新开的 Chrome 配置中手动登录贴吧：

  ```powershell
  & "$env:ProgramFiles\Google\Chrome\Application\chrome.exe" --remote-debugging-port=9222 --user-data-dir="$env:LOCALAPPDATA\TiebaCrawler\ChromeCDP"
  ```

  然后在应用设置中检查连接。应用只连接本机 `127.0.0.1:9222`，不会复制或保存密码、Cookie。
- **应用专用浏览器**：使用独立的本机配置目录。第一次点击“打开贴吧”后，在 Chrome 页面自行登录。
- 不处理验证码、不绕过登录或平台限制。遇到验证提示时，请在 Chrome 中按页面要求手动处理。

## 采集与归档

- 支持关键词搜索、帖子链接/ID、个人主页三种入口。
- 默认每任务最多 20 篇帖子、每帖最多 50 条评论、请求间隔至少 1.5 秒；上限分别为 100 篇和 100 条评论。
- 评论数量上限同时计入楼层和楼中楼回复。
- 媒体只从帖子/回复正文识别，不下载头像和表情；媒体链接限制为百度相关域名。下载失败时保留来源 URL 并继续任务。
- 开发模式数据存于项目 `data/`；安装版存于当前 Windows 用户的 Tauri 本地数据目录。每个任务在 `tasks/<task_id>/` 保存 SQLite 记录关联的媒体、导出文件。媒体导出里的相对路径从导出文件位置指向实际媒体。
- 支持 CSV、Excel、JSON、JSONL 导出。

打包 sidecar 并构建 Windows 安装包：

```powershell
npm run tauri:build
```

## 上游来源

贴吧 API 请求和字段解析基于 [NanmiCoder/MediaCrawler](https://github.com/NanmiCoder/MediaCrawler) 的贴吧实现独立整理。运行时不依赖该仓库目录。上游版权与许可完整文本随项目保存在 [`LICENSE`](./LICENSE) 中。
