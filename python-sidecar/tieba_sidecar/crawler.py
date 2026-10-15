# -*- coding: utf-8 -*-
# Adapted from MediaCrawler's Baidu Tieba client and extractor.
# Copyright (c) 2025 relakkes@gmail.com
# Source: https://github.com/NanmiCoder/MediaCrawler
# Licensed under NON-COMMERCIAL LEARNING LICENSE 1.1; see the project LICENSE.

from __future__ import annotations

import asyncio
import hashlib
import json
import mimetypes
import re
import time
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlencode, urlparse

from playwright.async_api import APIResponse, Browser, BrowserContext, Page, Playwright, async_playwright

from .extractor import (
    TIEBA,
    extract_media_urls,
    forum_name,
    parse_comments,
    parse_creator_portrait,
    parse_creator_thread_ids,
    parse_note_detail,
    parse_post_id,
    parse_search_results,
    parse_sub_comments,
)

PC_SIGN_SECRET = "36770b1f34c9bbf2e7d1a99d2b82fa9e"
ALLOWED_MEDIA_HOSTS = ("baidu.com", "bdstatic.com", "bcebos.com")
MAX_MEDIA_BYTES = 50 * 1024 * 1024


class TaskStopped(Exception):
    pass


class TiebaService:
    def __init__(self, emit):
        self.emit = emit
        self.playwright: Playwright | None = None
        self.browser: Browser | None = None
        self.context: BrowserContext | None = None
        self.page: Page | None = None
        self.browser_mode = "existing"
        self.active_task_id: str | None = None
        self.stop_event: asyncio.Event | None = None
        self.active_task: asyncio.Task | None = None
        self._pc_tbs = ""
        self._last_network_request = 0.0
        self._request_interval = 1.5

    async def _throttle_request(self) -> None:
        loop = asyncio.get_running_loop()
        remaining = self._request_interval - (loop.time() - self._last_network_request)
        if remaining > 0:
            await asyncio.sleep(remaining)
        self._last_network_request = loop.time()

    def log(self, task_id: str, message: str, level: str = "信息") -> None:
        self.emit({"type": "log", "taskId": task_id, "time": time.strftime("%H:%M:%S"), "level": level, "message": message})

    def check_stop(self) -> None:
        if self.stop_event and self.stop_event.is_set():
            raise TaskStopped()

    async def ensure_browser(self, mode: str, profile_dir: str = "") -> Page:
        if self.page and self.browser_mode == mode:
            try:
                if self.page.is_closed():
                    self.page = None
                else:
                    return self.page
            except Exception:
                self.page = None

        if self.playwright is None:
            self.playwright = await async_playwright().start()

        if self.browser_mode != mode:
            if self.context and self.browser_mode == "managed":
                try:
                    await self.context.close()
                except Exception:
                    pass
            self.page = None
            self.context = None
            self.browser = None

        if mode == "existing":
            self.browser = await self.playwright.chromium.connect_over_cdp("http://127.0.0.1:9222", timeout=5000)
            if not self.browser.contexts:
                raise RuntimeError("Chrome 已连接，但没有可用的浏览器会话。请使用 Chrome 稳定版并确认已登录贴吧。")
            self.context = self.browser.contexts[0]
            self.page = await self.context.new_page()
        else:
            if not profile_dir:
                raise RuntimeError("专用浏览器配置目录未设置")
            Path(profile_dir).mkdir(parents=True, exist_ok=True)
            self.context = await self.playwright.chromium.launch_persistent_context(
                user_data_dir=profile_dir,
                channel="chrome",
                headless=False,
                accept_downloads=True,
                viewport={"width": 1440, "height": 900},
            )
            self.page = self.context.pages[0] if self.context.pages else await self.context.new_page()
        self.browser_mode = mode
        await self.page.goto(TIEBA, wait_until="domcontentloaded", timeout=30000)
        return self.page

    async def check_browser(self, mode: str, profile_dir: str = "") -> None:
        try:
            await self.ensure_browser(mode, profile_dir)
            self.emit({"type": "browser_status", "status": "connected", "browserMode": mode, "message": "浏览器已连接。登录状态由你在 Chrome 中确认。"})
        except Exception as exc:
            hint = "请使用 Chrome 启动远程调试端口 9222 后重试；也可以切换到应用专用浏览器。"
            if mode == "managed":
                hint = "请确认已安装 Google Chrome，并在应用专用浏览器中完成贴吧登录。"
            self.emit({"type": "browser_status", "status": "failed", "browserMode": mode, "message": f"{exc}。{hint}"})

    async def open_login(self, mode: str, profile_dir: str = "") -> None:
        try:
            page = await self.ensure_browser(mode, profile_dir)
            await page.goto("https://tieba.baidu.com/", wait_until="domcontentloaded", timeout=30000)
            self.emit({"type": "browser_status", "status": "connected", "browserMode": mode, "message": "已打开贴吧。请在 Chrome 页面手动登录；应用不会读取或保存密码、Cookie。"})
        except Exception as exc:
            hint = "请先启动带远程调试端口 9222 的 Chrome。" if mode == "existing" else "请确认已安装 Chrome。"
            self.emit({"type": "browser_status", "status": "failed", "browserMode": mode, "message": f"无法打开贴吧登录页：{exc}。{hint}"})

    async def _evaluate_fetch(self, url: str, method: str = "GET", body: str = "") -> dict[str, Any]:
        if not self.page:
            raise RuntimeError("浏览器尚未连接")
        await self._throttle_request()
        response = await self.page.evaluate(
            """async ({ url, method, body }) => {
              const headers = { Accept: 'application/json, text/plain, */*' };
              const options = { method, credentials: 'include', headers };
              if (method === 'POST') {
                headers['Content-Type'] = 'application/x-www-form-urlencoded;charset=UTF-8';
                options.body = body;
              }
              const response = await fetch(url, options);
              return { status: response.status, text: await response.text() };
            }""",
            {"url": url, "method": method, "body": body},
        )
        if response.get("status") != 200:
            raise RuntimeError(f"贴吧接口返回 HTTP {response.get('status')}")
        try:
            result = json.loads(response.get("text") or "{}")
        except json.JSONDecodeError as exc:
            body_preview = re.sub(r"\s+", " ", response.get("text", ""))[:160]
            if any(word in body_preview.lower() for word in ("captcha", "验证码", "安全验证")):
                raise RuntimeError("贴吧要求进行验证。请在 Chrome 中按页面提示处理后再重试；应用不会尝试绕过验证。") from exc
            raise RuntimeError("贴吧接口返回了非 JSON 内容，请检查浏览器登录状态或稍后重试") from exc
        code = result.get("error_code", result.get("no", 0))
        if str(code) not in {"0", "None"}:
            message = str(result.get("error_msg") or result.get("error") or "接口拒绝了当前请求")
            if any(word in message.lower() for word in ("captcha", "verify", "验证")):
                message += "；请在 Chrome 中按页面提示处理，应用不会尝试绕过验证。"
            raise RuntimeError(f"贴吧接口提示：{message}")
        return result

    @staticmethod
    def _sign(values: dict[str, Any]) -> str:
        source = "".join(f"{key}={values[key]}" for key in sorted(values) if key not in {"sign", "sig"} and values[key] is not None)
        return hashlib.md5((source + PC_SIGN_SECRET).encode("utf-8")).hexdigest()

    async def _api(self, uri: str, params: dict[str, Any] | None = None, data: dict[str, Any] | None = None, method: str = "GET", sign: bool = False) -> dict[str, Any]:
        method = method.upper()
        query = {key: value for key, value in (params or {}).items() if value is not None}
        form = {key: value for key, value in (data or {}).items() if value is not None}
        if sign:
            source = form if method == "POST" else query
            source.setdefault("subapp_type", "pc")
            source.setdefault("_client_type", "20")
            source["sign"] = self._sign(source)
        url = TIEBA + uri
        if query:
            url += "?" + urlencode(query)
        return await self._evaluate_fetch(url, method, urlencode(form) if form else "")

    async def _get_tbs(self) -> str:
        if self._pc_tbs:
            return self._pc_tbs
        response = await self._api("/c/s/pc/sync", params={"subapp_type": "pc", "_client_type": "20"}, sign=True)
        value = (((response.get("data") or {}).get("anti") or {}).get("tbs"))
        if not value:
            raise RuntimeError("未能读取贴吧请求令牌，请确认 Chrome 中已登录贴吧")
        self._pc_tbs = str(value)
        return self._pc_tbs

    async def _get_post_page(self, post_id: str, page_num: int = 1) -> dict[str, Any]:
        tbs = await self._get_tbs()
        return await self._api(
            "/c/f/pb/page_pc",
            method="POST",
            data={
                "pn": page_num, "lz": 0, "r": 2, "mark_type": 0, "back": 0,
                "fr": "", "kz": post_id, "session_request_times": 1, "tbs": tbs,
                "subapp_type": "pc", "_client_type": "20",
            },
            sign=True,
        )

    async def _search(self, keyword: str, page_num: int) -> list[dict[str, Any]]:
        result = await self._api(
            "/mo/q/search/multsearch",
            params={
                "rn": 20, "st": 1, "word": keyword, "needbrand": 1, "sug_type": 2,
                "pn": page_num, "come_from": "search", "subapp_type": "pc", "_client_type": "20",
            },
            sign=True,
        )
        return parse_search_results(result, keyword)

    async def _creator_feed(self, portrait: str, page_num: int) -> dict[str, Any]:
        return await self._api(
            "/c/u/feed/myThread",
            params={"pn": page_num, "rn": 20, "portrait": portrait, "type": 1, "un": "", "subapp_type": "pc", "_client_type": "20"},
            sign=True,
        )

    def _emit(self, event_type: str, task_id: str, **values: Any) -> None:
        self.emit({"type": event_type, "taskId": task_id, **values})

    async def _collect_candidate_ids(self, task_id: str, request: dict[str, Any]) -> list[dict[str, Any]]:
        mode = request.get("mode", "keyword")
        limit = min(100, max(1, int(request.get("maxPosts", 20))))
        if mode == "keyword":
            keyword = str(request.get("keyword", "")).strip()
            forum_filter = str(request.get("forumName", "")).strip().removesuffix("吧")
            result: list[dict[str, Any]] = []
            seen: set[str] = set()
            for page_num in range(1, 11):
                self.check_stop()
                batch = await self._search(keyword, page_num)
                if not batch:
                    break
                for post in batch:
                    if forum_filter and post["forum"].removesuffix("吧") != forum_filter:
                        continue
                    if post["id"] not in seen:
                        seen.add(post["id"])
                        result.append(post)
                        if len(result) >= limit:
                            return result
                if page_num < 10:
                    await asyncio.sleep(max(1.5, float(request.get("intervalSeconds", 1.5))))
            return result
        if mode == "detail":
            targets = re.split(r"[\s,，;；]+", str(request.get("targets", "")).strip())
            ids = list(dict.fromkeys(filter(None, (parse_post_id(target) for target in targets))))
            if not ids:
                raise RuntimeError("没有从输入内容中识别到有效的贴吧帖子 ID")
            return [{"id": post_id, "keyword": ""} for post_id in ids[:limit]]
        if mode == "creator":
            targets = [item for item in re.split(r"[\s,，;；]+", str(request.get("targets", "")).strip()) if item]
            if not targets:
                raise RuntimeError("请填写吧主主页链接或用户标识")
            portrait = parse_creator_portrait(targets[0])
            if not portrait:
                raise RuntimeError("无法识别主页中的用户标识，请使用贴吧个人主页链接")
            result = []
            seen: set[str] = set()
            for page_num in range(1, 11):
                self.check_stop()
                feed = await self._creator_feed(portrait, page_num)
                ids = parse_creator_thread_ids(feed)
                if not ids:
                    break
                for post_id in ids:
                    if post_id not in seen:
                        seen.add(post_id)
                        result.append({"id": post_id, "keyword": ""})
                        if len(result) >= limit:
                            return result
                if not int((feed.get("data") or {}).get("has_more") or 0):
                    break
                await asyncio.sleep(max(1.5, float(request.get("intervalSeconds", 1.5))))
            return result
        raise RuntimeError("未知的采集模式")

    async def _media_response(self, url: str, referer: str) -> tuple[bytes | None, str]:
        parsed = urlparse(url)
        host = (parsed.hostname or "").lower()
        if parsed.scheme != "https" or not any(host == domain or host.endswith("." + domain) for domain in ALLOWED_MEDIA_HOSTS):
            return None, "链接不在百度媒体域名范围内"
        if not self.context:
            return None, "浏览器会话不可用"
        response: APIResponse | None = None
        try:
            await self._throttle_request()
            response = await self.context.request.get(url, timeout=30000, max_redirects=3, headers={"Referer": referer})
            final_host = (urlparse(response.url).hostname or "").lower()
            if not any(final_host == domain or final_host.endswith("." + domain) for domain in ALLOWED_MEDIA_HOSTS):
                return None, "媒体重定向目标不是百度域名"
            if not response.ok:
                return None, f"HTTP {response.status}"
            length = response.headers.get("content-length")
            if length and int(length) > MAX_MEDIA_BYTES:
                return None, "附件超过 50 MiB，已跳过"
            body = await response.body()
            if len(body) > MAX_MEDIA_BYTES:
                return None, "附件超过 50 MiB，已跳过"
            content_type = response.headers.get("content-type", "")
            if not content_type.startswith(("image/", "video/")):
                return None, "来源未返回图片或视频内容"
            return body, content_type
        except Exception as exc:
            return None, str(exc)[:180]
        finally:
            if response:
                try:
                    await response.dispose()
                except Exception:
                    pass

    async def _save_media(self, task_id: str, archive_dir: Path, post: dict[str, Any], comment: dict[str, Any] | None, content: Any, download: bool) -> int:
        urls = extract_media_urls(content)
        count = 0
        for item in urls:
            url = item["url"]
            if not url:
                continue
            comment_id = str((comment or {}).get("id") or "")
            attachment_id = hashlib.sha256(f"{post['id']}|{comment_id}|{url}".encode("utf-8")).hexdigest()[:20]
            relative_path = ""
            status = "未下载"
            kind = item["kind"]
            error = ""
            if download:
                body, content_type = await self._media_response(url, post.get("url", TIEBA))
                if body is not None:
                    ext = Path(urlparse(url).path).suffix.lower()
                    if ext not in IMAGE_EXTENSIONS | VIDEO_EXTENSIONS:
                        ext = mimetypes.guess_extension(content_type.split(";", 1)[0]) or (".mp4" if kind == "视频" else ".jpg")
                    folder = archive_dir / "media"
                    folder.mkdir(parents=True, exist_ok=True)
                    file_name = hashlib.sha256(url.encode("utf-8")).hexdigest()[:24] + ext
                    target = folder / file_name
                    if not target.exists():
                        target.write_bytes(body)
                    relative_path = f"media/{file_name}"
                    status = "已下载"
                else:
                    status = "下载失败"
                    error = content_type
            attachment = {
                "id": attachment_id,
                "postId": str(post["id"]),
                "commentId": comment_id,
                "kind": kind,
                "sourceUrl": url,
                "relativePath": relative_path,
                "status": status,
                "error": error,
                "fileName": Path(relative_path).name if relative_path else "",
                "time": time.strftime("%H:%M:%S"),
            }
            self._emit("attachment", task_id, data=attachment)
            count += 1
            if error:
                self.log(task_id, f"附件下载失败，来源链接已保留：{error}", "警告")
        return count

    async def _collect_post(self, task_id: str, candidate: dict[str, Any], request: dict[str, Any], archive_dir: Path) -> tuple[int, int, int]:
        post_id = str(candidate.get("id") or "")
        keyword = str(candidate.get("keyword") or request.get("keyword") or "")
        api_data = await self._get_post_page(post_id, 1)
        post = parse_note_detail(api_data, keyword)
        if not post.get("id"):
            raise RuntimeError(f"帖子 {post_id} 返回的数据不完整")
        first_content = post.pop("firstFloorContent", [])
        first_media = await self._save_media(task_id, archive_dir, post, None, first_content, bool(request.get("downloadMedia", True)))
        self._emit("note", task_id, data=post)
        self.log(task_id, f"已保存帖子：{post['title'][:80]}")

        comment_total = 0
        media_total = first_media
        if request.get("includeComments", True):
            pages = max(1, int(post.get("totalPages") or 1))
            max_comments = min(100, max(1, int(request.get("maxComments", 50))))
            interval = max(1.5, float(request.get("intervalSeconds", 1.5)))
            floor_index = 2
            for page_num in range(1, pages + 1):
                self.check_stop()
                if comment_total >= max_comments:
                    break
                page_data = api_data if page_num == 1 else await self._get_post_page(post_id, page_num)
                parent_comments = parse_comments(page_data, post, floor_index)
                floor_index += len(parent_comments)
                if not parent_comments:
                    break
                parent_comments = parent_comments[:max_comments - comment_total]
                for parent in parent_comments:
                    self.check_stop()
                    content = parent.pop("content", [])
                    media_total += await self._save_media(task_id, archive_dir, post, parent, content, bool(request.get("downloadMedia", True)))
                    self._emit("comment", task_id, data=parent)
                    comment_total += 1
                    remaining = max_comments - comment_total
                    if request.get("includeNested", True) and int(parent.get("nested") or 0) > 0 and remaining > 0:
                        sub_pages = min(10, (int(parent["nested"]) + 9) // 10)
                        for sub_page in range(1, sub_pages + 1):
                            self.check_stop()
                            if comment_total >= max_comments:
                                break
                            if self.page is None:
                                break
                            query = urlencode({"tid": post_id, "pid": parent["id"], "fid": parent.get("forumId", ""), "pn": sub_page})
                            await self._throttle_request()
                            await self.page.goto(f"{TIEBA}/p/comment?{query}", wait_until="domcontentloaded", timeout=30000)
                            sub_html = await self.page.content()
                            sub_comments = parse_sub_comments(sub_html, post, parent)
                            if not sub_comments:
                                break
                            for child in sub_comments[:max_comments - comment_total]:
                                child_content = child.pop("content", [])
                                media_total += await self._save_media(task_id, archive_dir, post, child, child_content, bool(request.get("downloadMedia", True)))
                                self._emit("comment", task_id, data=child)
                                comment_total += 1
                            if sub_page < sub_pages:
                                await asyncio.sleep(interval)
                    if comment_total >= max_comments:
                        break
                if page_num < pages and comment_total < max_comments:
                    await asyncio.sleep(interval)

        post["media"] = media_total
        self._emit("note", task_id, data=post)
        return 1, comment_total, media_total

    async def run_task(self, task_id: str, request: dict[str, Any], profile_dir: str, archive_dir: str) -> None:
        self.active_task_id = task_id
        self.stop_event = asyncio.Event()
        totals = {"posts": 0, "comments": 0}
        try:
            await self.ensure_browser(str(request.get("browserMode") or "existing"), profile_dir)
            self.log(task_id, "浏览器已连接。请确保贴吧账号已由你手动登录。")
            interval = max(1.5, float(request.get("intervalSeconds", 1.5)))
            request["intervalSeconds"] = interval
            self._request_interval = interval
            candidates = await self._collect_candidate_ids(task_id, request)
            if not candidates:
                raise RuntimeError("没有找到符合条件的帖子。请确认关键词、主页链接或登录状态。")
            archive_path = Path(archive_dir)
            archive_path.mkdir(parents=True, exist_ok=True)
            self.log(task_id, f"发现 {len(candidates)} 篇帖子，开始逐篇整理。")
            self._emit("progress", task_id, data={"progress": 0, "posts": 0, "comments": 0, "total": len(candidates)})
            total_media = 0
            for index, candidate in enumerate(candidates):
                self.check_stop()
                try:
                    posts, comments, media = await self._collect_post(task_id, candidate, request, archive_path)
                    totals["posts"] += posts
                    totals["comments"] += comments
                    total_media += media
                except TaskStopped:
                    raise
                except Exception as exc:
                    self.log(task_id, f"帖子 {candidate.get('id', '')} 采集失败，已保留已保存数据：{str(exc)[:180]}", "警告")
                progress = min(99, int((index + 1) * 100 / len(candidates)))
                self._emit("progress", task_id, data={"progress": progress, "posts": totals["posts"], "comments": totals["comments"], "total": len(candidates)})
                if index + 1 < len(candidates):
                    await asyncio.sleep(interval)
            if totals["posts"] == 0:
                raise RuntimeError("所有帖子都未能读取，没有保存到新数据。请检查贴吧登录状态后重试。")
            status = "completed"
            if self.stop_event.is_set():
                status = "stopped"
            self._emit("finished", task_id, data={"status": status, "posts": totals["posts"], "comments": totals["comments"], "media": total_media}, message="任务已完成" if status == "completed" else "任务已停止，已采集内容保留")
        except TaskStopped:
            self._emit("finished", task_id, data={"status": "stopped", **totals}, message="任务已停止，已采集内容保留")
        except Exception as exc:
            self._emit("error", task_id, message=str(exc)[:500])
        finally:
            self.active_task_id = None
            self.stop_event = None
            self.active_task = None

    async def handle(self, message: dict[str, Any]) -> None:
        action = str(message.get("action") or "")
        if action == "start_task":
            if self.active_task and not self.active_task.done():
                self.emit({"type": "error", "taskId": str(message.get("taskId") or ""), "message": "已有采集任务正在运行"})
                return
            request = message.get("request") or {}
            self.active_task = asyncio.create_task(self.run_task(
                str(message.get("taskId") or ""), request,
                str(message.get("profileDir") or ""), str(message.get("archiveDir") or ""),
            ))
            return
        if action == "stop_task":
            if self.active_task_id == str(message.get("taskId") or "") and self.stop_event:
                self.stop_event.set()
                self.log(self.active_task_id, "收到停止请求，等待当前请求结束。", "警告")
            return
        if self.active_task and not self.active_task.done() and action in {"check", "open_login"}:
            self.emit({"type": "browser_status", "status": "failed", "browserMode": str(message.get("browserMode") or self.browser_mode), "message": "任务运行期间浏览器正在使用中，请等待任务结束或先停止任务。"})
            return
        if action == "configure":
            return
        mode = str(message.get("browserMode") or "existing")
        profile_dir = str(message.get("profileDir") or "")
        if action == "check":
            await self.check_browser(mode, profile_dir)
        elif action == "open_login":
            await self.open_login(mode, profile_dir)
        elif action == "shutdown":
            if self.stop_event:
                self.stop_event.set()

