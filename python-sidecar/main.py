#!/usr/bin/env python3
"""
ExamOps Python Sidecar
处理文档解析、章节切分等复杂任务
"""

import sys
import json
from typing import Dict, Any


def handle_command(command: str, data: Dict[str, Any]) -> Dict[str, Any]:
    """处理来自 Rust 的命令"""

    if command == "parse_document":
        # TODO: 实现文档解析
        return {
            "status": "success",
            "message": "Document parsing not yet implemented"
        }

    elif command == "extract_chapters":
        # TODO: 实现章节提取
        return {
            "status": "success",
            "message": "Chapter extraction not yet implemented"
        }

    elif command == "extract_images":
        # TODO: 实现图片提取
        return {
            "status": "success",
            "message": "Image extraction not yet implemented"
        }

    else:
        return {
            "status": "error",
            "message": f"Unknown command: {command}"
        }


def main():
    """主循环：从标准输入读取命令，输出结果到标准输出"""

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
