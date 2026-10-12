#!/usr/bin/env python3
"""
ExamOps Python Sidecar
处理文档解析、章节切分等复杂任务
"""

import sys
import json
import argparse
from typing import Dict, Any


def handle_command(command: str, data: Dict[str, Any]) -> Dict[str, Any]:
    """处理来自 Rust 的命令"""

    if command == "parse_document":
        from parser import parse_document
        return {
            "status": "success",
            "content": parse_document(data["path"])
        }

    elif command == "extract_chapters":
        # TODO: 实现章节提取
        return {
            "status": "error",
            "message": "Chapter extraction not yet implemented"
        }

    elif command == "extract_images":
        # TODO: 实现图片提取
        return {
            "status": "error",
            "message": "Image extraction not yet implemented"
        }

    else:
        return {
            "status": "error",
            "message": f"Unknown command: {command}"
        }


def main():
    """主循环：从标准输入读取命令，输出结果到标准输出"""

    sys.stdin.reconfigure(encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    args = argparse.ArgumentParser()
    args.add_argument("--input")
    options = args.parse_args()
    if options.input:
        try:
            result = handle_command("parse_document", {"path": options.input})
        except Exception as error:
            result = {"status": "error", "message": str(error)}
        print(json.dumps(result, ensure_ascii=False), flush=True)
        return

    # 通知 Rust 端已启动
    print(json.dumps({"status": "ready"}), flush=True)

    # 主循环
    for line in sys.stdin:
        try:
            request = json.loads(line.strip())
            command = request.get("command")
            data = request.get("data", {})

            result = handle_command(command, data)
            print(json.dumps(result), flush=True)

        except json.JSONDecodeError as e:
            error_response = {
                "status": "error",
                "message": f"Invalid JSON: {str(e)}"
            }
            print(json.dumps(error_response), flush=True)

        except Exception as e:
            error_response = {
                "status": "error",
                "message": f"Error: {str(e)}"
            }
            print(json.dumps(error_response), flush=True)


if __name__ == "__main__":
    main()
