# 测试

每个阶段合并前都要求三套自动化测试全部通过（CI：`.github/workflows/ci.yml`，Rust 在 Ubuntu 和 Windows 上各跑一遍），发布前再按第 4 节做一次手工验收。

## 1. 运行自动化测试

| 套件 | 命令 | 说明 |
| --- | --- | --- |
| 前端 | `npx vue-tsc --noEmit`<br>`npm test` | 类型检查；vitest 单元测试（Markdown 渲染与转义、引用解析、检索高亮、时间戳与学习步骤跳转、测验工具函数） |
| Rust 核心 | `cd src-tauri`<br>`cargo clippy --all-targets -- -D warnings`<br>`cargo test` | 全部使用临时数据库和临时目录，不会触碰真实数据；AI 接口由测试内启动的本地 HTTP 服务模拟 |
| 文档处理（Python） | `cd python-sidecar`<br>`python -m unittest -v` | 测试时自动生成样例文件；OCR 用例需要中文字体（Windows 自带微软雅黑，Linux 安装 `fonts-noto-cjk`） |

Rust 测试的前置条件：

- 先运行一次 `npm run build`（Tauri 编译时会嵌入 `dist/`）；
- `src-tauri/binaries/` 下需要有文档处理程序。完整构建用 `npm run sidecar:build`；只跑 Rust 测试时可以像 CI 一样放一个空的占位文件 `examops-parser-<目标三元组>[.exe]`（Rust 测试不会调用它）。

## 2. 路线图要求的专项测试

| 类别 | 覆盖的情况 | 位置 |
| --- | --- | --- |
| 异常文件 | 加密 PDF、只限制编辑的 PDF、下载中断的 PDF、改了扩展名的 Word、不是课件的 PPTX、旧版 .doc/.ppt、只有空行的 Markdown、超长单行文本、GBK / Big5 编码的 TXT、二进制乱码、空文件、不支持的格式 | `python-sidecar/test_parser.py`（`AbnormalFileTests`、`EncodingTests`、`ParserTests`）、`test_structure.py`、`src-tauri/src/documents/mod.rs` |
| | 文档处理程序报错、异常退出；重新解析失败时保留原有结果 | `src-tauri/src/documents/parse.rs`、`documents/mod.rs` |
| API 错误 | 401 / 403 / 404 / 429 / 5xx 的中文提示；网络错误、429、5xx 自动重试一次（流式回答只在还没收到内容时重试）；新参数规则（`max_completion_tokens` 等）自动兼容 | `src-tauri/src/ai/client.rs`（`transient_server_errors_are_retried_once`、`streams_retry_once_before_any_text_but_never_after`）、`ai/error.rs` |
| | 模型输出无法解析（出题、批改、知识点、学习路径、复习建议）时不写入任何数据；漏评的题可重新批改或自评 | `assessment/tests.rs`（`unusable_ai_replies_store_nothing`、`failed_ai_grading_falls_back_to_self_grading`）、`knowledge`、`paths`、`review` |
| 断网 | 无法连接、请求超时、流式回答中途断开或长时间无数据 | `ai/client.rs`（`unreachable_servers_and_silent_streams_fail_with_clear_messages`） |
| | 回答中断时保存已生成的部分并标明原因；学习进度只在完整回答后前进 | `tutor/chat.rs`（`api_errors_and_interruptions_are_saved_with_partial_text`）、`tutor/teach.rs` |
| 数据持久化 | 学习对话、分段进度、小测作答、掌握度、复习计划在“重启”（关闭全部连接后重新打开）后保持 | `assessment/tests.rs`（`a_study_session_survives_a_restart`）、`tutor/chat.rs` |
| | 旧版数据库自动升级且数据不丢；拒绝打开更新版本的数据库；被强制退出打断的首次导入在下次启动时标为失败，被打断的重新解析保留原有结果 | `database/migrations.rs`、`database/mod.rs`（`work_cut_off_by_a_crash_is_recovered_at_start_up`） |
| | 多个写入同时发生时等待而不是报“database is locked” | `database/mod.rs`（`concurrent_writers_wait_instead_of_failing`） |
| | 数据目录恢复到另一台电脑或另一个 Windows 用户下，资料副本和图片仍能找到 | `documents/mod.rs`（`a_data_folder_restored_elsewhere_still_finds_copies_and_figures`） |
| 安全 | API Key 不写入模型配置、界面只显示掩码；资料中的“指令”不会被执行；学生答案作为数据隔离 | `ai/profiles.rs`、`security/mod.rs`、`context/mod.rs`、`assessment/grade.rs` |

## 3. 本地模拟 AI 服务

`scripts/mock_ai.py` 是一个兼容 OpenAI 接口的本地模拟服务（只用于开发，不随安装包分发），可以在没有 API Key、不产生费用的情况下走通全部 AI 功能，并模拟各种故障：

```bash
python scripts/mock_ai.py                 # http://127.0.0.1:18080/v1，API Key：sk-test
python scripts/mock_ai.py --fail drop     # 从一开始就模拟某种故障
curl http://127.0.0.1:18080/__fail/500    # 运行中切换：none | 401 | 429 | 500 | drop | stall
```

在 ExamOps 中添加模型：接口地址 `http://127.0.0.1:18080/v1`，模型名称 `mock`，API Key `sk-test`。

| 模式 | 模拟的情况 | ExamOps 应有的表现 |
| --- | --- | --- |
| `401` | API Key 错误 | 提示“API Key 无效或无权访问该模型”，并提供“检查模型设置” |
| `429` | 请求过多 / 额度不足 | 自动重试一次，仍失败则提示“请求过于频繁或账户额度不足” |
| `500` | 服务端错误 | 自动重试一次，仍失败则提示“AI 服务暂时不可用” |
| `drop` | 回答到一半网络断开 | 已生成的部分保留，提示“连接中断，已保留已生成的内容，可以点击“重试”重新生成”；学习分段进度不前进 |
| `stall` | 连接建立后不再返回数据 | 超过模型设置中的超时时间后提示“AI 服务响应超时”，已生成的部分保留 |
| 停止服务 | 断网 / 服务未启动 | 自动重试一次后提示“无法连接到 AI 服务”；导入、阅读、检索、笔记照常可用 |

出题、批改、知识点提取等非流式请求遇到同样的故障时只提示原因，不会生成残缺的小测或知识点。

## 4. 发布前手工验收

在一台 Windows 10/11 上安装正式安装包后逐项检查：

1. **安装**：当前用户安装无需管理员权限；没有 WebView2 的系统会自动安装；开始菜单和桌面快捷方式可以启动。
2. **首次使用**：无课程时首页有引导；添加模型（真实服务和模拟服务各一次），测试连接成功。
3. **资料**：一次导入 PDF（文字版 + 扫描版）、DOCX、PPTX、TXT（GBK 编码）、Markdown，再导入一个损坏文件；
   检查章节目录、原页面预览、图片、检索结果定位；损坏文件显示原因且不影响其他文件。
4. **学习闭环**：生成知识点 → 按章节生成学习路径 → AI 从零学习（继续、举例、跳过、点击分段、标记已学完）→ 小测（五种题型、跳过、交卷、AI 批改）→ 复习页（到期、薄弱、错题重练、AI 建议）。
5. **故障**：用模拟服务依次切换第 3 节的各种模式，确认提示清楚、已有数据不丢失，切回 `none` 后可以继续。
6. **持久化**：在讲解、作答过程中直接关闭窗口，重新打开后学习位置、分段进度、已作答的题目都在；
   导入资料、重新解析或 AI 回答过程中在任务管理器里结束进程：重新打开后首次导入的资料显示“上次正文提取被中断，请重新解析”，重新解析的资料保留原有内容，回答标记为中断并保留已生成的部分。
7. **备份迁移**：关闭 ExamOps，复制数据目录到另一台电脑（或另一个 Windows 用户）的 `%APPDATA%\com.examops.app`，打开后资料、图片、学习记录完整。
8. **卸载**：卸载后数据目录默认保留；勾选删除应用数据时目录被删除。重新安装后保留的数据可以直接使用。
9. **关于页**：许可证、Universal ExamPrep Skill 的 MIT 声明、开源许可列表可以打开；安装目录中有 `THIRD_PARTY_NOTICES.md`。
