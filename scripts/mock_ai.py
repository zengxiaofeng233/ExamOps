#!/usr/bin/env python3
"""Local OpenAI-compatible test server for ExamOps (development only, never shipped).

Lets you try every AI feature without an API key or token costs, and simulate the
failures ExamOps must survive: wrong key, rate limits, server errors, a connection
that drops mid-answer and a stream that goes silent.

    python scripts/mock_ai.py                     # http://127.0.0.1:18080/v1, key sk-test
    python scripts/mock_ai.py --fail drop         # every answer breaks off halfway

In ExamOps add a model with base URL http://127.0.0.1:18080/v1, model "mock" and
API key "sk-test". Switch failures while the app is running:

    curl http://127.0.0.1:18080/__fail/500        # none | 401 | 429 | 500 | drop | stall

The replies recognise ExamOps' prompt markers (material fences, quiz and grading
requests, knowledge extraction, learning paths). If prompts change, unknown
requests still get a generic cited answer, so the mock degrades instead of failing.
"""

import argparse
import json
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

FAILURES = ("none", "401", "429", "500", "drop", "stall")
STATE = {"fail": "none", "key": "sk-test", "delay": 0.03, "log": None}
LOCK = threading.Lock()
KINDS = ["single_choice", "true_false", "short_answer", "recall", "scenario"]


def materials(text):
    """(label, header, body) for every <<<资料 S1 | …>>> fence in a prompt."""
    return re.findall(r"<<<资料 (S\d+) \| ([^>]*)>>>\n(.*?)\n<<<\1 结束>>>", text, re.S)


def first_sentence(text):
    text = text.replace("\n", "")
    match = re.search(r"^(.{8,80}?[。；.;])", text)
    return (match.group(1) if match else text[:60]).strip()


def chat_reply(system, question, found):
    if "【本次讲解片段】" in system:
        label, _, body = found[0] if found else ("S1", "", "资料中的内容")
        part = re.search(r"本次讲解(第 \d+/\d+ 部分)", system)
        return "\n\n".join([
            f"【教材】这一部分（{part.group(1) if part else '本部分'}）讲的是：{first_sentence(body)} [{label}]",
            "【补充】打个比方：函数像一台机器，输入一个数，输出一个确定的数。",
            "检查一下：你能用自己的话说说这一部分的核心概念吗？",
        ])
    if "全部讲解完毕" in system:
        return "【教材】本章要点：\n\n1. 回顾本章的核心概念 [S1]\n2. 注意概念之间的联系\n\n【补充】建议做一次本章小测，检验掌握情况。"
    parts = [f"【教材】{first_sentence(body)} [{label}]" for label, _, body in found[:2]]
    if not parts:
        parts.append("课程资料中没有找到相关内容。")
    parts.append("【补充】可以把导数理解为“瞬时变化率”，例如位移对时间的导数就是速度。")
    parts.append("【推导】由定义 $f'(x_0)=\\lim_{\\Delta x\\to 0}\\frac{f(x_0+\\Delta x)-f(x_0)}{\\Delta x}$，取 $f(x)=x^2$ 得 $f'(x)=2x$。")
    return "\n\n".join(parts)


def quiz(question, found):
    points = re.findall(r"^\[K(\d+)\] ([^：\n]+)", question, re.M) or [("1", "本章内容")]
    count = int((re.search(r"出 (\d+) 道小测题", question) or [0, 3])[1])
    allowed = re.search(r"【题型】只能使用：([^。\n]+)", question)
    kinds = [k for k in KINDS if allowed and f"（{k}）" in allowed.group(1)] or ["single_choice"]
    fact = first_sentence(found[0][2]) if found else "资料中的定义"
    questions = []
    for i in range(count):
        point, title = points[i % len(points)]
        kind = kinds[i % len(kinds)]
        item = {"type": kind, "point": f"K{point}", "sources": ["S1"], "explanation": f"依据教材：{fact} [S1]"}
        if kind == "single_choice":
            item.update(question=f"（第 {i + 1} 题）关于「{title}」，下列说法正确的是？",
                        options=[f"{title}：{fact[:18]}", "与本知识点无关的说法", "把定义说反了", "只在特殊情况下成立"], answer="A")
        elif kind == "true_false":
            item.update(question=f"（第 {i + 1} 题）「{title}」属于本课程的学习内容。", answer=True)
        else:
            item.update(question=f"（第 {i + 1} 题）用自己的话说明「{title}」。", answer=fact, rubric=["说出定义", "指出关键条件"])
        questions.append(item)
    return "```json\n" + json.dumps({"questions": questions}, ensure_ascii=False) + "\n```"


def grading(question):
    results = []
    for label, text in re.findall(r"<<<学生答案 (Q\d+)>>>\n(.*?)\n<<<学生答案结束>>>", question, re.S):
        if len(text.strip()) >= 6:
            results.append({"id": label, "score": 0.8, "feedback": "要点基本正确，表述清楚 [S1]。", "missing": []})
        else:
            results.append({"id": label, "score": 0.3, "feedback": "回答过于简略，没有说明关键条件。", "missing": ["关键条件"]})
    return json.dumps({"results": results}, ensure_ascii=False)


def json_reply(system, question, found):
    if "道小测题" in question:
        return quiz(question, found)
    if "批改" in system:
        return grading(question)
    if "学习规划顾问" in system:
        focus = [f"K{i}" for i in re.findall(r"\[K(\d+)\]", question)[:2]]
        return json.dumps({"advice": "**今天**：先做到期复习的小测，再重练错题。\n\n**接下来几天**：每天学习一节新内容，学完立即做小测。",
                           "focus": focus}, ensure_ascii=False)
    if "核心知识点" in question:
        points = [{"title": first_sentence(body)[:12] + "的要点", "summary": first_sentence(body), "importance": 2, "sources": [label]}
                  for label, _, body in found[:4]]
        return "```json\n" + json.dumps({"points": points}, ensure_ascii=False) + "\n```"
    if "【课程目录】" in question:
        steps = []
        for section, title in re.findall(r"^\s{2}\[(\d+)\] (.+?) · ", question, re.M)[:2]:
            steps.append({"kind": "read_text", "title": f"精读：{title}", "section_id": int(section), "minutes": 25})
            steps.append({"kind": "ai_explain", "section_id": int(section), "minutes": 15})
        return json.dumps({"name": "AI 推荐路径", "description": "先读后讲，循序渐进", "steps": steps}, ensure_ascii=False)
    return json.dumps({"items": []}, ensure_ascii=False)


def reply(body):
    messages = body.get("messages", [])
    system = messages[0]["content"] if messages and messages[0].get("role") == "system" else ""
    question = messages[-1]["content"] if messages else ""
    found = materials(system) or materials(question)
    if "JSON" in system or "JSON" in question:
        return json_reply(system, question, found)
    return chat_reply(system, question, found)


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def send_json(self, status, payload):
        raw = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        mode = self.path.rsplit("/", 1)[-1]
        if self.path.startswith("/__fail/") and mode in FAILURES:
            with LOCK:
                STATE["fail"] = mode
            print(f"failure mode: {mode}", flush=True)
            self.send_json(200, {"fail": mode})
        else:
            self.send_json(404, {"error": {"message": f"GET /__fail/<{'|'.join(FAILURES)}>"}})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("content-length", 0))) or b"{}")
        if STATE["log"]:
            with LOCK, open(STATE["log"], "a", encoding="utf-8") as log:
                log.write(json.dumps(body, ensure_ascii=False) + "\n")
        fail = STATE["fail"]
        if fail == "401" or self.headers.get("authorization") != f"Bearer {STATE['key']}":
            return self.send_json(401, {"error": {"message": "Incorrect API key provided", "type": "invalid_request_error"}})
        if fail in ("429", "500"):
            message = "Rate limit reached" if fail == "429" else "The server had an error"
            return self.send_json(int(fail), {"error": {"message": message}})
        text = reply(body)
        if not body.get("stream"):
            if fail in ("drop", "stall"):
                self.close_connection = True
                return
            return self.send_json(200, {"choices": [{"message": {"role": "assistant", "content": text}, "finish_reason": "stop"}],
                                        "usage": {"prompt_tokens": 100, "completion_tokens": len(text)}})
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("transfer-encoding", "chunked")
        self.end_headers()

        def chunk(data):
            raw = data.encode("utf-8")
            self.wfile.write(f"{len(raw):x}\r\n".encode() + raw + b"\r\n")
            self.wfile.flush()

        if fail == "stall":
            time.sleep(600)
            return
        cut = len(text) // 2 if fail == "drop" else len(text)
        try:
            for i in range(0, cut, 6):
                delta = {"choices": [{"delta": {"content": text[i:min(i + 6, cut)]}, "finish_reason": None}]}
                chunk("data: " + json.dumps(delta, ensure_ascii=False) + "\n\n")
                time.sleep(STATE["delay"])
            if fail == "drop":
                self.close_connection = True
                return
            done = {"choices": [{"delta": {}, "finish_reason": "stop"}], "usage": {"prompt_tokens": 120, "completion_tokens": len(text)}}
            chunk("data: " + json.dumps(done) + "\n\n")
            chunk("data: [DONE]\n\n")
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            # The app stopped generating ("停止") or went away.
            self.close_connection = True


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--port", type=int, default=18080)
    parser.add_argument("--key", default="sk-test", help="API key the app must send (default: sk-test)")
    parser.add_argument("--delay", type=float, default=0.03, help="seconds between streamed pieces")
    parser.add_argument("--fail", choices=FAILURES, default="none", help="failure to simulate from the start")
    parser.add_argument("--log", help="append every request body to this JSON-lines file")
    options = parser.parse_args()
    STATE.update(fail=options.fail, key=options.key, delay=options.delay, log=options.log)
    server = ThreadingHTTPServer(("127.0.0.1", options.port), Handler)
    server.daemon_threads = True
    print(f"mock AI on http://127.0.0.1:{options.port}/v1 (key {options.key}, failure mode {options.fail})", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        sys.exit(0)


if __name__ == "__main__":
    main()
