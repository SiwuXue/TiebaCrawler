# -*- coding: utf-8 -*-
# Adapted from MediaCrawler's Tieba API extraction routines.
# Copyright (c) 2025 relakkes@gmail.com
# Source: https://github.com/NanmiCoder/MediaCrawler
# Licensed under NON-COMMERCIAL LEARNING LICENSE 1.1; see the project LICENSE.

from __future__ import annotations

import hashlib
import json
import re
from datetime import datetime, timezone
from typing import Any
from urllib.parse import parse_qs, quote, unquote, urljoin, urlparse

from bs4 import BeautifulSoup

TIEBA = "https://tieba.baidu.com"
IMAGE_EXTENSIONS = {".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp", ".avif"}
VIDEO_EXTENSIONS = {".mp4", ".m3u8", ".webm", ".mov", ".m4v"}


def normalize_text(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, list):
        return "".join(normalize_text(item.get("text") or item.get("c") or "") for item in value if isinstance(item, dict)).strip()
    if isinstance(value, dict):
        return normalize_text(value.get("text") or value.get("c") or "")
    return re.sub(r"\s+", " ", str(value)).strip()


def mask_nickname(value: Any) -> str:
    name = normalize_text(value)
    if not name:
        return "匿名用户"
    if len(name) == 1:
        return "*"
    if len(name) == 2:
        return f"{name[0]}*"
    return f"{name[0]}***{name[-1]}"


def anonymize(value: Any) -> str:
    normalized = normalize_text(value)
    return hashlib.sha256(normalized.encode("utf-8")).hexdigest()[:16] if normalized else ""


def format_time(value: Any) -> str:
    try:
        stamp = int(value)
        if stamp <= 0:
            return ""
        return datetime.fromtimestamp(stamp, tz=timezone.utc).astimezone().strftime("%Y-%m-%d %H:%M")
    except (TypeError, ValueError, OSError):
        return normalize_text(value)


def forum_name(value: Any) -> str:
    name = normalize_text(value)
    return name if not name or name.endswith("吧") else f"{name}吧"


def forum_url(value: str) -> str:
    return f"{TIEBA}/f?kw={quote(value.removesuffix('吧'))}" if value else TIEBA


def parse_creator_portrait(value: str) -> str:
    raw = (value or "").strip()
    parsed = urlparse(raw if "://" in raw else f"https://tieba.baidu.com/home/main?id={quote(raw)}")
    query = parse_qs(parsed.query)
    portrait = query.get("id", [""])[0] or query.get("portrait", [""])[0] or query.get("un", [""])[0]
    if portrait:
        return unquote(portrait).split("?")[0]
    if raw.startswith("tb."):
        return raw.split("?")[0]
    return ""


def parse_post_id(value: str) -> str:
    match = re.search(r"(?:/p/|^)(\d{6,})(?:\D|$)", (value or "").strip())
    return match.group(1) if match else ""


def _user_map(api: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {str(user.get("id")): user for user in api.get("user_list", []) if user.get("id")}


def _user_public_fields(user: dict[str, Any]) -> tuple[str, str]:
    nickname = mask_nickname(user.get("name_show") or user.get("show_nickname") or user.get("user_name"))
    identity = user.get("portrait") or user.get("id") or nickname
    return nickname, anonymize(identity)


def _image_kind(url: str, hint: str = "") -> str | None:
    suffix = urlparse(url).path.lower().rsplit(".", 1)
    extension = f".{suffix[-1]}" if len(suffix) > 1 else ""
    low_hint = hint.lower()
    if extension in VIDEO_EXTENSIONS or "video" in low_hint or "play_url" in low_hint:
        return "视频"
    if extension in IMAGE_EXTENSIONS or any(token in low_hint for token in ("origin_src", "cdn_src", "bpic", "image_url", "pic_url")):
        return "图片"
    return None


def extract_media_urls(content: Any) -> list[dict[str, str]]:
    """Inspect only post/comment content objects; user/profile/avatar trees are never passed in."""
    found: dict[str, str] = {}
    url_pattern = re.compile(r"https?://[^\s\"'<>]+", re.IGNORECASE)
    media_keys = {"origin_src", "cdn_src", "big_cdn_src", "src", "url", "video_url", "play_url", "bpic", "pic_url", "image_url"}

    def visit(node: Any, hint: str = "") -> None:
        if isinstance(node, dict):
            for key, value in node.items():
                key_text = str(key).lower()
                if any(blocked in key_text for blocked in ("avatar", "portrait", "user_show", "head", "face", "emoji", "smiley")):
                    continue
                if isinstance(value, str):
                    candidate_values = [value]
                    if key_text in {"text", "c", "html"}:
                        candidate_values += url_pattern.findall(value)
                    for candidate in candidate_values:
                        url = candidate.strip().rstrip(",.;)]}")
                        if url.startswith("//"):
                            url = "https:" + url
                        elif url.startswith("/"):
                            url = urljoin(TIEBA, url)
                        kind = _image_kind(url, key_text) if url.startswith(("http://", "https://")) else None
                        if key_text in media_keys and url.startswith(("http://", "https://")):
                            kind = kind or ("视频" if "video" in key_text or "play" in key_text else "图片")
                        if kind:
                            found[url] = kind
                elif isinstance(value, (dict, list)):
                    visit(value, key_text)
        elif isinstance(node, list):
            for item in node:
                visit(item, hint)
        elif isinstance(node, str):
            for url in url_pattern.findall(node):
                kind = _image_kind(url, hint)
                if kind:
                    found[url] = kind

    visit(content)
    return [{"url": url, "kind": kind} for url, kind in found.items()]


def _post_from_search(item: dict[str, Any], keyword: str) -> dict[str, Any] | None:
    post_id = str(item.get("tid") or "")
    if not post_id:
        return None
    user = item.get("user") or {}
    forum = forum_name(item.get("forum_name") or "")
    nickname, _ = _user_public_fields({
        "id": user.get("id"), "portrait": user.get("portrait"),
        "show_nickname": user.get("show_nickname") or user.get("user_name"),
    })
    return {
        "id": post_id,
        "forum": forum,
        "title": normalize_text(item.get("title")),
        "body": normalize_text(item.get("content")),
        "author": nickname,
        "time": format_time(item.get("time") or item.get("create_time")),
        "replies": int(item.get("post_num") or 0),
        "media": 0,
        "keyword": keyword,
        "color": "stone",
        "url": f"{TIEBA}/p/{post_id}",
    }


def parse_search_results(api: dict[str, Any], keyword: str) -> list[dict[str, Any]]:
    result = []
    for card in (api.get("data") or {}).get("card_list", []):
        if card.get("cardInfo") != "thread" and card.get("cardStyle") != "thread":
            continue
        post = _post_from_search(card.get("data") or {}, keyword)
        if post:
            result.append(post)
    return result


def parse_note_detail(api: dict[str, Any], keyword: str = "") -> dict[str, Any]:
    thread = api.get("thread") or {}
    first = api.get("first_floor") or {}
    forum = api.get("forum") or api.get("display_forum") or {}
    users = _user_map(api)
    author = users.get(str(first.get("author_id")), {})
    post_id = str(thread.get("id") or thread.get("tid") or first.get("tid") or "")
    public_nickname, _ = _user_public_fields(author)
    return {
        "id": post_id,
        "forum": forum_name(forum.get("name") or ""),
        "title": normalize_text(thread.get("title") or first.get("title") or "未命名帖子"),
        "body": normalize_text(first.get("content") or (thread.get("origin_thread_info") or {}).get("abstract") or (thread.get("origin_thread_info") or {}).get("content")),
        "author": public_nickname,
        "time": format_time(first.get("time") or thread.get("create_time")),
        "replies": int(thread.get("reply_num") or 0),
        "media": 0,
        "keyword": keyword,
        "color": "stone",
        "url": f"{TIEBA}/p/{post_id}" if post_id else "",
        "forumId": str(forum.get("id") or ""),
        "totalPages": int((api.get("page") or {}).get("total_page") or 0),
        "firstFloorContent": first.get("content") or [],
    }


def parse_comments(api: dict[str, Any], post: dict[str, Any], floor_offset: int = 1) -> list[dict[str, Any]]:
    forum = api.get("forum") or api.get("display_forum") or {}
    users = _user_map(api)
    result = []
    for index, item in enumerate(api.get("post_list") or [], start=floor_offset):
        comment_id = str(item.get("id") or "")
        if not comment_id:
            continue
        nickname, _ = _user_public_fields(users.get(str(item.get("author_id")), {}))
        result.append({
            "id": comment_id,
            "postId": str(post.get("id") or ""),
            "parentId": "",
            "author": nickname,
            "body": normalize_text(item.get("content")),
            "time": format_time(item.get("time")),
            "floor": f"{index} 楼",
            "nested": int(item.get("sub_post_number") or 0),
            "forumId": str(forum.get("id") or post.get("forumId") or ""),
            "url": post.get("url", ""),
            "content": item.get("content") or [],
        })
    return result


def parse_creator_thread_ids(api: dict[str, Any]) -> list[str]:
    result = []
    for item in (api.get("data") or {}).get("list", []):
        thread = item.get("thread_info") or {}
        post_id = thread.get("tid") or thread.get("id")
        if post_id:
            result.append(str(post_id))
    return list(dict.fromkeys(result))


def parse_sub_comments(page_html: str, post: dict[str, Any], parent: dict[str, Any]) -> list[dict[str, Any]]:
    soup = BeautifulSoup(page_html, "html.parser")
    result = []
    nodes = soup.select("li.lzl_single_post.j_lzl_s_p, li.lzl_single_post")
    for node in nodes:
        data: dict[str, Any] = {}
        raw = node.get("data-field") or node.get("data-field-json")
        if raw:
            try:
                data = json.loads(raw)
            except (json.JSONDecodeError, TypeError):
                data = {}
        comment_id = str(data.get("spid") or node.get("data-pid") or "")
        body_node = node.select_one(".lzl_content_main")
        body = body_node.get_text(" ", strip=True) if body_node else node.get_text(" ", strip=True)
        if not comment_id or not body:
            continue
        nickname = data.get("showname") or "匿名用户"
        time_node = node.select_one(".lzl_time")
        media_content: list[dict[str, str]] = [{"text": body}]
        if body_node:
            for image in body_node.select("img"):
                classes = " ".join(image.get("class") or []).lower()
                if any(token in classes for token in ("smiley", "emoji", "face", "avatar", "portrait")):
                    continue
                source = image.get("origin_src") or image.get("data-src") or image.get("src") or ""
                if source:
                    media_content.append({"origin_src": str(source)})
            for video in body_node.select("video, source"):
                source = video.get("src") or video.get("data-src") or ""
                if source:
                    media_content.append({"video_url": str(source)})
        result.append({
            "id": comment_id,
            "postId": str(post.get("id") or ""),
            "parentId": str(parent.get("id") or ""),
            "author": mask_nickname(nickname),
            "body": body,
            "time": time_node.get_text(" ", strip=True) if time_node else "",
            "floor": "子回复",
            "nested": 0,
            "forumId": str(parent.get("forumId") or ""),
            "url": post.get("url", ""),
            "content": media_content,
        })
    return result

