# ExamOps 架构说明

本文说明 ExamOps 的模块划分、数据流、数据模型和安全设计，对应《ExamOps Development Roadmap V1.4》的七个引擎。

## 1. 总体结构

ExamOps 是单用户、本地运行的 Windows 桌面应用，固定技术栈为：

| 层 | 技术 | 作用 |
| --- | --- | --- |
| 桌面外壳 | Tauri 2（WebView2） | 窗口、系统对话框、进程管理、安装包 |
| 界面 | Vue 3 + TypeScript、Pinia、Vue Router、Tailwind CSS 4 | 全部页面与交互 |
| 核心 | Rust（Tokio、rusqlite、reqwest） | 业务逻辑、数据库、AI 网关、检索 |
| 数据 | SQLite（WAL）+ FTS5 | 课程、资料、学习记录、全文索引 |
| 文档处理 | Python Sidecar（PyInstaller 打包） | PDF / Word / PPT / TXT / Markdown 解析、章节识别、图片提取、本地 OCR、页面渲染 |
| AI | OpenAI 兼容接口（BYOK） | 讲解、问答、出题、批改、规划；用户自行配置 |

```
┌──────────────────────── WebView2（Vue 3 界面）────────────────────────┐
│ 课程 · 资料 · 知识点 · 学习路径 · AI 学习 · 测验 · 复习 · 问答 · 网课 · 笔记 │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ invoke / Channel（类型见 src/types）
┌───────────────────────────────▼──────────────────────────────────────┐
│ Rust 核心（src-tauri/src）                                            │
│  commands/  薄封装：参数 → 领域模块 → 结果（错误统一为中文 AppError）     │
│  documents/ search/    Document Engine（导入、结构、分块、FTS5）          │
│  context/              Context Engine（Full / RAG / Auto）               │
│  ai/ security/         Model Gateway（BYOK、SSE、错误映射、密钥保管）     │
│  tutor/                Tutor Engine（问答、从零学习 / 复习 / 冲刺）        │
│  knowledge/ paths/ resources/ notes/   学习路径与资源                     │
│  assessment/ mastery/ review/          Assessment Engine 与复习调度        │
│  study/                Study State Engine（学习状态、活动、概览）          │
│  database/             连接、迁移、启动恢复                               │
└───────────────┬───────────────────────────────┬──────────────────────┘
                │ 子进程（JSON 协议）            │ HTTPS（仅在用户使用 AI 时）
      ┌─────────▼─────────┐               ┌─────▼──────────────┐
      │ Python Sidecar     │               │ 用户配置的 AI 接口   │
      │ 解析 · OCR · 渲染   │               │ （OpenAI 兼容）      │
      └───────────────────┘               └────────────────────┘
```

前端与后端的接口类型一一对应：`src/types/*.ts` 镜像 Rust 的 `Serialize/Deserialize` 结构，命令清单见 [API.md](API.md)。

## 2. 七个引擎

### 2.1 Document Engine（`documents/`、`search/`、`python-sidecar/`）

- **导入**：资料复制到数据目录 `documents/<课程>/<资料>.<扩展名>`，原文件不被修改；每份资料独立保存、可重新解析或删除。
  副本和图片的文件名只由课程、资料编号决定，数据目录被整体恢复到别的电脑或用户下、数据库中的绝对路径失效时，会在当前数据目录中重新定位（`documents::locate_copy`、`image_path`）。
- **解析**（Sidecar 协议 v2）：输出“单元”（PDF 页、PPT 幻灯片、Word 段落、文本行块）、带标题级别的文本块、图片、警告和进度。
  章节识别顺序：PDF 书签 → Word 标题样式 → Markdown 标题 → PPT 节与标题 → 编号格式（第三章 / 3.2 / 一、）→ PDF 字号版式；都失败时按页分组。
  目录页、页眉页脚、页码会被去除；扫描页用随包分发的 RapidOCR 在本地识别。
- **结构化存储**：`document_pages`（单元与原文位置）、`document_sections`（章节树）、`document_chunks`（检索与引用的最小片段，带位置标签“第 3 页”“第 2–4 张幻灯片”）、`document_images`。
- **检索**：FTS5 + `unicode61`，中日韩文字逐字建索引；关键词检索用短语查询（“导数”只匹配连续的两个字），问题检索用二元组 OR + BM25 排序。
- **重新解析**：按“级别 + 标题”把旧章节映射到新章节，知识点、学习步骤、网课、笔记、对话及其资料范围中的章节引用自动迁移。
- **中断恢复**：启动时，被强制退出打断的首次导入标为失败（可重新解析），被打断的重新解析恢复为原有结果，残留的临时图片目录被清理。

### 2.2 Context Engine（`context/`）

- **Full**：所选范围（课程 / 资料 / 章节）的全部片段在预算内按阅读顺序给出；**RAG**：检索最相关的片段及其相邻片段；**Auto**：资料占用不超过预算的 `full_ratio` 时用 Full，否则用 RAG。
- 预算 = 模型上下文 − 最大输出 − 规则与历史；Token 估算：中日韩字符 1 个 token，其余约 3.5 字符 1 个 token。
- 每个片段用 `<<<资料 S1 | 文件 · 章节 · 位置>>> … <<<S1 结束>>>` 隔离并编号；片段内的 `<<<`、`>>>` 会被转义，资料无法“闭合”自己的标记块。
- 教学时的讲解片段作为“必含资料”放在最前（pinned），补充检索结果重新编号排在其后。

### 2.3 Tutor Engine（`tutor/`）

- **AI 问答**（`chat.rs`）：一次提问 = 保存问题 → 构建上下文 → 流式生成 → 保存回答与来源。回答要求用【教材】【补充】【推导】标注内容来源，并以 [S1] 引用资料；被引用的来源可点击跳转到原文。
- **AI 学习**（`teach.rs`）：
  - 从零学习：章节片段按约 1500 字分成“部分”（网课按字幕分段），每次讲一部分并停下等待；支持继续、举例、换种讲法、跳过下一部分、跳到任一部分、总结、标记学完；
  - 正常复习：先出回忆题，再逐题纠正；
  - 考前冲刺：结合考试倒计时和未掌握 / 疑难知识点，精炼要点与典型题；
  - 进度（`TeachProgress`）保存在对话上，只有完整回答后才前进；“重新生成”先恢复该轮开始前的进度。
- **网课**：AI 使用已保存的字幕、文字稿、标注来源的 B 站 AI 总结、笔记和时间戳；没有时明确告知无法得知视频内容。B 站读取在 Rust 后台执行：解析链接和分 P → WBI 签名字幕接口 → 字幕不可用时尝试 AI 总结。前端预览并填入空白文字稿，不覆盖已有内容，保存后复用既有学习流程；摘要在分段和引用中仍标为摘要。扫码登录凭据仅存系统凭据管理器，不进入前端、数据库或模型上下文；短链接与字幕下载不携带登录凭据，跳转和下载地址受域名限制。
- 流式生成可随时停止；生成中崩溃的回答在下次启动时标记为“中断”。

### 2.4 Learning Path Engine（`paths/`、`knowledge/`、`resources/`、`notes/`）

- 学习路径由有序步骤组成：阅读教材、阅读 PPT、观看网课、AI 讲解、知识点小测、主动回忆、复习旧知识、自定义任务。
- 创建方式：按章节模板生成、AI 推荐（每一步校验能否对应到真实章节 / 网课 / 知识点）、空白路径手动编排；步骤可拖动排序、插入、编辑、删除、完成 / 跳过。
- 知识点来自章节标题、AI 提取（校验与去重）或手动维护；网课保存链接、字幕、笔记和时间戳（不嵌入网站）；笔记可关联资料页、章节和知识点。

### 2.5 Assessment Engine（`assessment/`）

- 题型：单选、判断、简答、自由复述、情景应用题。出题只依据学生资料，每题关联知识点和来源；模型输出经过严格校验和归一化（`generate.rs`）。
- 判分：客观题本地判分；主观题由 AI 对照参考答案、评分要点和资料批改（`grade.rs`），学生答案作为数据隔离；AI 不可用时可重新批改或自评。
- 生命周期：`create` → `save_answer`（作答与位置随时保存）→ `submit` → `finalize`（得分、掌握度、复习计划、错题、学习步骤只更新一次）。
- 设置：每学完一个知识点后 / 每学完一章后 / 仅手动；题量；是否允许跳过；默认题型。

### 2.6 Study State Engine（`study/`、`mastery/`、`review/`）

- 学习状态：当前路径与步骤、当前章节、上次阅读位置、上次 AI 学习、学习活动记录（`study_events`），重启后恢复。
- **学过 ≠ 已掌握**：阅读、讲解、网课、回忆只会把知识点记为“学过”；“已掌握”只由测验得出（掌握度 ≥ 80% 且至少 2 题），之后答差会回到“学过”。
- 掌握度：测验结果的指数移动平均（权重 0.5）。
- 间隔复习：SM-2 风格（1 天 → 3 天 → 间隔 × 难度系数），答得差明天再复习；设置考试日期后压缩间隔，保证考前至少再复习一次。
- 复习队列：到期 → 薄弱 → 疑难 → 学过未测；错题本；规则化的“下一步建议”与 AI 学习建议（只保留本课程存在的知识点）。

### 2.7 Model Gateway（`ai/`、`security/`）

- 任意 OpenAI 兼容接口（含 Ollama、LM Studio 等本地服务）；多模型配置、默认模型、单个对话可切换模型。
- SSE 流式解析（含思考过程）、空闲超时、取消；网络错误、429、5xx 自动重试一次（流式回答只在还没收到内容时重试，已显示部分内容后不再重复请求）；接口不支持的参数（如 `max_tokens` → `max_completion_tokens`）自动兼容重试。
- 错误映射为可操作的中文提示（401、404、429、5xx、超时、断网、流中断）。
- API Key 保存在 Windows 凭据管理器（keyring），界面只显示首尾几位；凭据管理器不可用时回退到本地数据库并明确提示。Key 不写日志、不返回前端。

## 3. 数据模型

数据库文件：`%APPDATA%\com.examops.app\examops.db`（WAL 模式，`busy_timeout` 10 秒，外键开启）。结构通过版本化迁移维护（`src-tauri/migrations/`，`PRAGMA user_version`），每个迁移在独立事务中执行；比当前版本新的数据库会被拒绝打开，避免旧版程序破坏数据。

| 迁移 | 内容 |
| --- | --- |
| 0001 baseline | courses、documents、document_contents、notes 等第一阶段表 |
| 0002 app_settings | 应用设置（JSON 值） |
| 0003 document_structure | document_pages / sections / chunks / chunks_fts / images |
| 0004 ai_gateway | model_profiles、conversations、messages、ai_usage |
| 0005 learning | knowledge_points、resources、resource_marks、learning_paths、path_steps、study_state、study_events；对话与笔记关联字段；考试日期 |
| 0006 assessment | quizzes、quiz_questions、mastery、mistakes |

主要关系：

```
courses ─┬─ documents ─┬─ document_pages
         │             ├─ document_sections ─┬─ document_chunks ─ document_chunks_fts
         │             └─ document_images    └─（被知识点、步骤、网课、笔记、对话引用）
         ├─ knowledge_points ─ mastery
         ├─ resources ─ resource_marks
         ├─ learning_paths ─ path_steps
         ├─ conversations ─ messages
         ├─ quizzes ─ quiz_questions ─ mistakes
         ├─ notes
         ├─ study_state（每课程一行）
         └─ study_events（学习活动记录）
```

删除课程会级联删除以上全部数据和资料副本；删除资料会删除其章节、片段和图片，引用这些章节的知识点、步骤等保留但解除关联。

## 4. 关键流程

**导入资料**：选择文件 → 复制到数据目录 → 启动 Sidecar（`--input`，进度经 stderr 回传）→ 结构化结果在一个事务中写入（章节、单元、片段、图片、索引）→ 引用迁移 → 完成。失败时保留原有结果并给出原因。

**一次 AI 学习回合**：读取对话与进度 → 生成教学计划（当前部分作为必含资料 + 学生知识点状态 + 章节结构）→ 预算内补充检索 → 流式生成（每 0.8 秒保存一次已生成内容）→ 完整结束后执行“回合后动作”（推进进度、标记学过、记录活动、开始学习步骤）。

**一次小测**：确定知识点（章节没有知识点时先从标题生成）→ 均衡选取各章节资料 → AI 出题并校验 → 作答（随时保存）→ 交卷：客观题判分 + AI 批改 → 全部有分后更新掌握度、复习日期、错题本、学习步骤和活动记录。

## 5. 安全与隐私

- 无账号、无云端存储、无遥测；只有使用 AI 功能时，相关资料片段和问题才发送到用户自己配置的接口。
- 资料、学生答案、网课字幕都按“数据”隔离在标记块中，并在系统提示中声明其中的指令无效（提示注入防护）；模型无法访问数据库、文件或系统命令，所有模型输出（知识点、路径、题目、评分、建议）都经过校验后才写入数据库。
- 界面渲染 Markdown 时禁用原始 HTML；外部链接只允许 http/https 并在系统浏览器打开；生产环境启用内容安全策略（CSP）和原型冻结。
- 网络访问只发生在用户主动使用 AI 时；文档解析和 OCR 完全离线。

## 6. 目录结构

```
ExamOps/
├── src/                      Vue 前端
│   ├── views/                页面（course/ 下为课程内各标签页）
│   ├── components/           组件（chat / documents / quiz / study / settings / ui / layout）
│   ├── services/             invoke 封装（与 Rust 命令一一对应）
│   ├── stores/               Pinia（app / course / models / toast）
│   ├── types/                与 Rust 结构对应的类型
│   └── utils/                纯函数（含单元测试 __tests__）
├── src-tauri/                Rust 核心与 Tauri 配置
│   ├── migrations/           数据库迁移（只新增，不修改已发布的迁移）
│   └── src/                  见第 2 节
├── python-sidecar/           文档处理（parser / structure / chapters / figures / render）
├── scripts/                  Sidecar 打包、许可声明、命令清单生成
├── docs/                     文档
└── .github/workflows/        CI 与 Windows 安装包构建
```
