#!/usr/bin/env python3
"""Regenerates the command tables in docs/API.md (run after adding or changing commands).

    python scripts/command_reference.py           # rewrite the "## 命令清单" section of docs/API.md
    python scripts/command_reference.py --print   # print the tables instead

Each `#[tauri::command]` in src-tauri/src/commands is listed with the
arguments the frontend passes to `invoke` (camelCase, as Tauri expects), its
return type and the first paragraph of its `///` doc comment. Managed state
(`State<…>`, `AppHandle`) is omitted.
"""
from __future__ import annotations

import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMMANDS = ROOT / "src-tauri" / "src" / "commands"
API_DOC = ROOT / "docs" / "API.md"
SECTION = "## 命令清单"
SKIP_TYPES = ("AppHandle", "State<", "tauri::State<")
PATTERN = re.compile(
    r"((?:\s*///[^\n]*\n)*)\s*#\[tauri::command\]\s*pub\s+(async\s+)?fn\s+(\w+)\s*\((.*?)\)\s*->\s*([^{]+)\{",
    re.S,
)


def camel(name: str) -> str:
    head, *rest = name.split("_")
    return head + "".join(part.title() for part in rest)


def split_args(raw: str) -> list[tuple[str, str]]:
    args, depth, current = [], 0, ""
    for char in raw:
        if char == "<":
            depth += 1
        elif char == ">":
            depth -= 1
        if char == "," and depth == 0:
            args.append(current)
            current = ""
        else:
            current += char
    if current.strip():
        args.append(current)
    out = []
    for arg in args:
        name, _, kind = arg.strip().partition(":")
        out.append((name.strip(), " ".join(kind.split())))
    return out


def tables() -> str:
    out = []
    for file in sorted(COMMANDS.glob("*.rs")):
        if file.name == "mod.rs":
            continue
        text = file.read_text(encoding="utf-8")
        rows = []
        for docs, is_async, name, raw_args, returns in PATTERN.findall(text):
            params = []
            for arg, kind in split_args(raw_args):
                if any(kind.startswith(skip) for skip in SKIP_TYPES):
                    continue
                if kind.startswith("Channel<"):
                    params.append(f"`{camel(arg)}`: Channel")
                else:
                    params.append(f"`{camel(arg)}`: {kind}")
            doc = " ".join(line.strip().lstrip("/").strip() for line in docs.strip().splitlines())
            if not doc:
                raise SystemExit(f"{file.name}: `{name}` has no /// doc comment")
            result = " ".join(returns.split())
            if result.startswith("AppResult<") and result.endswith(">"):
                result = result[len("AppResult<"):-1]
            rows.append(f"| `{name}` | {', '.join(params) or '—'} | {result} | {doc} |")
        if rows:
            out.append(f"### `commands/{file.name}`\n")
            out.append("| 命令 | 参数 | 返回 | 说明 |\n| --- | --- | --- | --- |")
            out.append("\n".join(rows))
            out.append("")
    return "\n".join(out)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--print", action="store_true", help="print the tables instead of updating docs/API.md")
    options = parser.parse_args()
    generated = tables()
    if options.print:
        print(generated)
        return
    text = API_DOC.read_text(encoding="utf-8")
    start = text.index(SECTION) + len(SECTION)
    following = text.find("\n## ", start)
    rest = text[following:] if following != -1 else "\n"
    API_DOC.write_text(text[:start] + "\n\n" + generated.rstrip("\n") + "\n" + rest, encoding="utf-8")
    print(f"updated {API_DOC.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
