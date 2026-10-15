#!/usr/bin/env python3
"""Regenerates THIRD_PARTY_NOTICES.md for the Windows installer.

Lists what ExamOps ships: Rust crates linked into the Windows executable,
npm packages bundled into the user interface, the Python packages inside the
document sidecar, bundled runtimes and models, and the design reference
(Universal ExamPrep Skill, MIT). Standard license texts are appended.

    python scripts/third_party_notices.py [--python <sidecar venv python>]

Run it from the repository root after `npm ci` (and with the sidecar venv
when releasing) and commit the result.
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "THIRD_PARTY_NOTICES.md"
TARGET = "x86_64-pc-windows-msvc"
SIDECAR_ROOTS = [
    line.split("==")[0].strip()
    for line in (ROOT / "python-sidecar" / "requirements.txt").read_text(encoding="utf-8").splitlines()
    if line.strip() and not line.startswith("#") and "==" in line
]

EXAMPREP_NOTICE = """MIT License

Copyright (c) 2026 ZeKaiNie

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""

MIT_TEXT = """Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""

BSD3_TEXT = """Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE."""

ISC_TEXT = """Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE."""

ZLIB_TEXT = """This software is provided 'as-is', without any express or implied warranty. In
no event will the authors be held liable for any damages arising from the use
of this software.

Permission is granted to anyone to use this software for any purpose, including
commercial applications, and to alter it and redistribute it freely, subject to
the following restrictions:

1. The origin of this software must not be misrepresented; you must not claim
   that you wrote the original software. If you use this software in a product,
   an acknowledgment in the product documentation would be appreciated but is
   not required.
2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution."""

RUNTIMES = [
    ("Microsoft Edge WebView2 Runtime", "随安装程序分发的离线安装包", "Microsoft Software License Terms", "https://developer.microsoft.com/microsoft-edge/webview2/"),
    ("Python", "3.12（PyInstaller 打包进文档处理组件）", "PSF-2.0", "https://docs.python.org/3/license.html"),
    ("PyInstaller bootloader", "6.x", "GPL-2.0 with the PyInstaller bootloader exception", "https://pyinstaller.org/en/stable/license.html"),
    ("PDFium", "随 pypdfium2 分发", "BSD-3-Clause / Apache-2.0", "https://pdfium.googlesource.com/pdfium/"),
    ("ONNX Runtime", "随 onnxruntime 分发", "MIT", "https://github.com/microsoft/onnxruntime"),
    ("RapidOCR / PP-OCR 模型", "随 rapidocr 分发", "Apache-2.0", "https://github.com/RapidAI/RapidOCR"),
    ("KaTeX（含字体）", "随界面分发", "MIT", "https://github.com/KaTeX/KaTeX"),
]


def run(cmd: list[str], cwd: Path) -> str:
    # Windows exposes npm as npm.cmd; CreateProcess does not search PATHEXT.
    cmd = [shutil.which(cmd[0]) or cmd[0], *cmd[1:]]
    return subprocess.run(cmd, cwd=cwd, check=True, capture_output=True, text=True, encoding="utf-8").stdout


def rust_crates() -> list[tuple[str, str, str]]:
    raw = run(["cargo", "tree", "--target", TARGET, "-e", "normal", "--prefix", "none", "--format", "{p}|{l}"], ROOT / "src-tauri")
    crates = {}
    for line in raw.splitlines():
        line = line.replace(" (*)", "").strip()
        if "|" not in line:
            continue
        package, license_ = line.split("|", 1)
        parts = package.split()
        if len(parts) < 2 or "(" in package:  # skip the local exam-ops crate (has a path)
            continue
        crates[(parts[0], parts[1].lstrip("v"))] = license_.strip().replace("/", " OR ") or "见源码"
    return sorted((name, version, lic) for (name, version), lic in crates.items())


def npm_packages() -> list[tuple[str, str, str]]:
    raw = run(["npm", "ls", "--omit=dev", "--all", "--parseable"], ROOT)
    packages = {}
    for folder in raw.splitlines()[1:]:
        manifest = Path(folder) / "package.json"
        if not manifest.exists():
            continue
        data = json.loads(manifest.read_text(encoding="utf-8"))
        license_ = data.get("license") or (data.get("licenses") or [{}])[0].get("type", "")
        if isinstance(license_, dict):
            license_ = license_.get("type", "")
        packages[(data["name"], data["version"])] = license_ or "见源码"
    return sorted((name, version, lic) for (name, version), lic in packages.items())


PY_SCRIPT = r"""
import importlib.metadata as md, json, re, sys
roots = json.loads(sys.argv[1])
seen = {}
def norm(n): return re.sub(r'[-_.]+', '-', n).lower()
def walk(name):
    key = norm(name)
    if key in seen: return
    try: dist = md.distribution(name)
    except md.PackageNotFoundError:
        seen[key] = None; return
    meta = dist.metadata
    lic = meta.get('License-Expression') or ''
    if not lic or len(lic) > 80:
        classifiers = [c.split('::')[-1].strip() for c in (meta.get_all('Classifier') or []) if c.startswith('License ::')]
        lic = ', '.join(classifiers) or (meta.get('License') or '')[:80]
    seen[key] = [meta['Name'], dist.version, lic.replace('\n', ' ')]
    for req in dist.requires or []:
        if 'extra ==' in req: continue
        if 'sys_platform' in req and 'win32' not in req: continue
        m = re.match(r'[A-Za-z0-9_.\-]+', req)
        if m: walk(m.group(0))
for r in roots: walk(r)
print(json.dumps([v for v in seen.values() if v]))
"""


def python_packages(python: str | None) -> list[tuple[str, str, str]]:
    if not python:
        return []
    raw = subprocess.run([python, "-c", PY_SCRIPT, json.dumps(SIDECAR_ROOTS)], check=True, capture_output=True, text=True, encoding="utf-8").stdout
    return sorted((name, version, lic or "见源码") for name, version, lic in json.loads(raw))


def table(rows: list[tuple[str, str, str]]) -> str:
    out = ["| 组件 | 版本 | 许可证 |", "| --- | --- | --- |"]
    out += [f"| {name} | {version} | {lic} |" for name, version, lic in rows]
    return "\n".join(out)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python", help="python of the sidecar environment (lists the bundled Python packages)")
    args = parser.parse_args()
    apache = (ROOT / "node_modules/@tauri-apps/cli/LICENSE-APACHE-2.0").read_text(encoding="utf-8").strip()
    project_license = (ROOT / "LICENSE").read_text(encoding="utf-8").strip()
    rust, npm, python = rust_crates(), npm_packages(), python_packages(args.python)
    sections = [
        "# 第三方组件与许可声明（Third-Party Notices）",
        "ExamOps 本身以 MIT 许可证发布（见下文）。安装包中包含以下第三方组件，它们各自的许可证适用于相应组件。"
        "本文件由 `scripts/third_party_notices.py` 生成；依赖变更后请重新生成。",
        "## ExamOps\n\n```\n" + project_license + "\n```",
        "## 设计参考：Universal ExamPrep Skill\n\n"
        "ExamOps 的部分学习方法设计（例如回答中区分“教材 / 补充 / 推导”来源、按章节讲解、错题再练）参考了 "
        "[Universal ExamPrep Skill](https://github.com/ZeKaiNie/universal-examprep-skill)。ExamOps 没有复制其源代码，"
        "按照路线图要求保留其版权与许可声明如下：\n\n```\n" + EXAMPREP_NOTICE + "\n```",
        "## 随安装包分发的运行时与模型\n\n| 组件 | 说明 | 许可证 | 项目地址 |\n| --- | --- | --- | --- |\n"
        + "\n".join(f"| {a} | {b} | {c} | {d} |" for a, b, c, d in RUNTIMES),
        f"## Rust 依赖（Windows 可执行文件，{len(rust)} 个）\n\n"
        "“A OR B” 表示可任选其一的双许可证。标注 MPL-2.0 的组件未经修改，其源代码可从 crates.io 获取。\n\n" + table(rust),
        f"## 前端依赖（界面，{len(npm)} 个）\n\n" + table(npm),
    ]
    if python:
        sections.append(f"## 文档处理组件中的 Python 包（{len(python)} 个）\n\n"
                        "标注 MPL-2.0 的组件未经修改，其源代码可从 PyPI 获取。\n\n" + table(python))
    sections += [
        "## 许可证全文\n\n各组件的版权声明见其源代码包；以下为上表中常见许可证的标准文本。",
        "### MIT License\n\n```\n" + MIT_TEXT + "\n```",
        "### BSD 3-Clause License（BSD-2-Clause 为其中第 1、2 条）\n\n```\n" + BSD3_TEXT + "\n```",
        "### ISC License\n\n```\n" + ISC_TEXT + "\n```",
        "### zlib License\n\n```\n" + ZLIB_TEXT + "\n```",
        "### 其他许可证\n\n"
        "- Mozilla Public License 2.0：https://www.mozilla.org/MPL/2.0/\n"
        "- Unicode License v3：https://www.unicode.org/license.txt\n"
        "- Python Software Foundation License 2.0：https://docs.python.org/3/license.html\n"
        "- Historical Permission Notice and Disclaimer / MIT-CMU（Pillow）：https://github.com/python-pillow/Pillow/blob/main/LICENSE\n"
        "- The Unlicense：https://unlicense.org/\n"
        "- Boost Software License 1.0：https://www.boost.org/LICENSE_1_0.txt\n"
        "- CC0 1.0：https://creativecommons.org/publicdomain/zero/1.0/",
        "### Apache License 2.0\n\n```\n" + apache + "\n```",
    ]
    OUT.write_text("\n\n".join(sections) + "\n", encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(rust)} crates, {len(npm)} npm packages, {len(python)} Python packages")


if __name__ == "__main__":
    sys.exit(main())
