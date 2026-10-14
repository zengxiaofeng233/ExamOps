#!/usr/bin/env python3
"""
ExamOps Python Sidecar
处理文档解析、章节识别、图片提取和页面预览。

One-shot mode (used by the desktop app; prints one JSON object on stdout):
  examops-parser --input FILE [--figures-dir DIR] [--progress]
  examops-parser --render FILE --pages 1,2,3 --out-dir DIR [--width 1200]
JSON-lines mode (no arguments): one request per line on stdin, e.g.
  {"command": "parse_structured", "data": {"path": "...", "figures_dir": "..."}}
"""

import sys
import json
import argparse
from typing import Dict, Any

PROTOCOL_VERSION = 2


def error_response(error: Exception) -> Dict[str, Any]:
    from parser import DocumentError
    if isinstance(error, DocumentError):
        return {"status": "error", "message": str(error)}
    return {"status": "error", "message": f"{type(error).__name__}: {error}"}


def handle_command(command: str, data: Dict[str, Any], progress=None) -> Dict[str, Any]:
    """处理来自 Rust 的命令"""

    if command == "parse_document":
        from parser import parse_document
        return {"status": "success", "content": parse_document(data["path"])}

    if command == "parse_structured":
        from parser import parse_structured
        result = parse_structured(data["path"], data.get("figures_dir"), progress)
        return {"status": "success", "version": PROTOCOL_VERSION, **result}

    if command == "render_pages":
        from render import render_pages
        pages = render_pages(data["path"], data.get("pages", []), data["out_dir"], data.get("width", 1200))
        return {"status": "success", "pages": pages}

    return {"status": "error", "message": f"Unknown command: {command}"}


def main():
    """一次性模式或 JSON 行模式。"""

    sys.stdin.reconfigure(encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    args = argparse.ArgumentParser()
    args.add_argument("--input")
    args.add_argument("--figures-dir")
    args.add_argument("--progress", action="store_true")
    args.add_argument("--render")
    args.add_argument("--pages", default="")
    args.add_argument("--out-dir")
    args.add_argument("--width", type=int, default=1200)
    options = args.parse_args()

    if options.input or options.render:
        from parser import Progress
        try:
            if options.input:
                result = handle_command("parse_structured", {"path": options.input, "figures_dir": options.figures_dir},
                                        Progress(options.progress))
            else:
                pages = [int(p) for p in options.pages.split(",") if p.strip()]
                result = handle_command("render_pages", {"path": options.render, "pages": pages,
                                                         "out_dir": options.out_dir, "width": options.width})
        except Exception as error:
            result = error_response(error)
        print(json.dumps(result, ensure_ascii=False), flush=True)
        return

    # 通知 Rust 端已启动
    print(json.dumps({"status": "ready", "version": PROTOCOL_VERSION}), flush=True)

    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            request = json.loads(line.strip())
            result = handle_command(request.get("command"), request.get("data", {}))
        except json.JSONDecodeError as e:
            result = {"status": "error", "message": f"Invalid JSON: {str(e)}"}
        except Exception as e:
            result = error_response(e)
        print(json.dumps(result, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
