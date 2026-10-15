from __future__ import annotations

import asyncio
import json
import sys
from typing import Any

from tieba_sidecar.crawler import TiebaService


def emit(event: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(event, ensure_ascii=False, separators=(",", ":")) + "\n")
    sys.stdout.flush()


async def main() -> None:
    service = TiebaService(emit)
    while True:
        line = await asyncio.to_thread(sys.stdin.readline)
        if not line:
            break
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            emit({"type": "error", "taskId": "", "message": "收到无效的 JSONL 请求"})
            continue
        if message.get("action") == "shutdown":
            await service.handle(message)
            break
        try:
            await service.handle(message)
        except Exception as exc:
            emit({"type": "error", "taskId": str(message.get("taskId") or ""), "message": str(exc)[:500]})
    if service.active_task and not service.active_task.done():
        if service.stop_event:
            service.stop_event.set()
        try:
            await asyncio.wait_for(service.active_task, timeout=4)
        except Exception:
            service.active_task.cancel()
    if service.context and service.browser_mode == "managed":
        try:
            await service.context.close()
        except Exception:
            pass
    if service.playwright:
        try:
            await service.playwright.stop()
        except Exception:
            pass


if __name__ == "__main__":
    asyncio.run(main())
