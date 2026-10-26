# ExamOps 前后端接口（Tauri 命令）

界面通过 `@tauri-apps/api` 的 `invoke(命令, 参数)` 调用 Rust 核心。封装位于 `src/services/*.ts`，参数与返回值的 TypeScript 类型位于 `src/types/*.ts`，与 Rust 结构一一对应。

## 约定

- **参数名**：Rust 中的 `snake_case` 参数在 `invoke` 中写成 `camelCase`（例如 `course_id` → `courseId`）；结构体参数统一放在 `input` 中，结构体字段保持 `snake_case`。
- **错误**：所有命令返回 `Result<T, AppError>`。失败时 Promise 以一段可直接展示给学生的中文消息 reject（例如“章节不属于该课程”“AI 服务响应超时，请稍后重试…”）。前端统一用 `toast.error(e)` 展示。
- **流式事件**：`send_message` 通过 `Channel<ChatEvent>` 推送 `started`（问题与回答占位消息）→ 多个 `delta` / `reasoning` → `done`（最终消息，含来源与用量）。`import_document`、`reprocess_document` 保存任务后返回 pending 状态，通过 Channel 通知入队；后台进度由 `get_documents` / `get_document` 的 `completed_units`、`total_units`、`task_error` 获取，重启后也可查询。
- **耗时命令**（`async`）：导入与解析、AI 出题 / 批改 / 提取 / 推荐 / 建议、连接测试。它们在后台线程运行，不阻塞界面。
- **数据隔离**：所有涉及课程的写操作都会校验引用的资料、章节、知识点、网课、学习步骤属于同一课程。
- 本文件的命令表由 `python scripts/command_reference.py` 生成，新增命令后请重新生成。

## 命令清单

`send_message` 的 `input.images` 为可选数组（省略时按纯文字消息处理），每项包含 `name` 和 `data_url`（PNG / JPEG / WebP 的 base64 data URL）。每条消息最多 4 张，每张解码后最多 5 MB。图片保存在用户消息的 `meta.images`，随历史和重新生成传递；`content` 可在有图片时为空。数据库表结构保持不变。

### `commands/ai.rs`

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `list_model_profiles` | — | Vec<profiles::ModelProfile> | 所有模型配置（不含 API Key，只有首尾几位提示）。 |
| `save_model_profile` | `input`: profiles::ProfileInput | profiles::SaveResult | 新建或修改模型配置；API Key 存入系统凭据管理器（不可用时回退本地并返回提示）。 |
| `delete_model_profile` | `id`: i64 | () | 删除模型配置及其保存的 API Key。 |
| `set_default_model_profile` | `id`: i64 | () | 设为默认模型。 |
| `test_model_profile` | `input`: profiles::ProfileInput | TestReport | 测试连接（使用表单中尚未保存的值；未填写 API Key 时使用已保存的）。 |
| `fetch_available_models` | `input`: profiles::ModelListInput | Vec<String> | 获取接口的模型列表；无需先填写模型名称或保存配置，密钥留空时复用同一地址的已存密钥。 |
| `get_ai_usage` | — | ai::UsageSummary | Token 用量统计（今天 / 累计）。 |
| `list_conversations` | `courseId`: i64 | Vec<Conversation> | 课程的全部对话（AI 问答与 AI 学习）。 |
| `create_conversation` | `input`: tutor::CreateConversation | Conversation | 新建问答对话。 |
| `get_conversation` | `id`: i64 | ConversationDetail | 对话及其全部消息。 |
| `update_conversation` | `input`: tutor::UpdateConversation | Conversation | 修改标题、上下文模式、资料范围或使用的模型。 |
| `delete_conversation` | `id`: i64 | () | 删除对话及其消息。 |
| `send_message` | `input`: chat::SendInput, `onEvent`: Channel | Message | 发送消息并流式返回回答：问答对话直接回答；学习对话（从零学习 / 正常复习 / 考前冲刺）按动作（继续、举例、跳过等）组织讲解。 |
| `start_teaching` | `input`: teach::StartTeaching | Conversation | 打开（或新建）某一章节 / 网课在指定学习模式下的学习对话。 |
| `get_lesson_outline` | `conversationId`: i64 | Vec<teach::LessonPart> | “从零学习”的分段及每段状态（已讲 / 跳过 / 下一段 / 未讲）。 |
| `mark_lesson_finished` | `conversationId`: i64 | Conversation | 标记学完：相关知识点记为“学过”，关联的学习步骤完成。 |
| `cancel_generation` | `requestId`: String | bool | 停止正在生成的回答（保留已生成的内容）。 |

### `commands/app.rs`

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `get_storage_info` | — | StorageInfo | 获取待迁移位置和可手动清理的旧数据目录。 |
| `schedule_data_migration` | `directory`: Option<PathBuf> | () | 选择空文件夹，在退出后下次启动时迁移；传 null 取消待迁移计划。 |
| `cleanup_previous_data` | `directory`: PathBuf | () | 用户确认后清理已迁移的旧副本；如旧数据有变化则拒绝删除。 |
| `get_app_settings` | — | AppSettings | 应用设置。 |
| `update_app_settings` | `settings`: AppSettings | AppSettings | 保存应用设置（数值会被校正到允许范围）。 |
| `get_app_info` | — | AppInfo | 版本、数据目录、数据库与资料占用、数据库结构版本。 |
| `check_for_updates` | — | Option<AvailableUpdate> | 检查 GitHub 正式 Release，仅返回版本号更高且包含 Windows x64 安装包的版本；不下载或安装。 |
| `download_update` | `version`: String, `onProgress`: Channel<DownloadProgress> | () | 重新确认正式版本，下载对应签名包；推送 downloaded/total 字节进度，校验成功后暂存在进程内，不自动安装。 |
| `install_update` | — | () | 安装已校验的更新包；资料处理期间拒绝安装，成功启动安装程序后退出，由安装程序重新打开应用。 |
| `open_data_dir` | — | () | 在资源管理器中打开数据目录。 |
| `open_external_url` | `url`: String | () | 在系统浏览器中打开 http(s) 链接（AI 回答中的链接、网课）。 |
| `save_last_location` | `location`: LastLocation | () | 记住当前页面（下次启动时恢复）。 |
| `get_last_location` | — | Option<LastLocation> | 上次所在的页面。 |
| `get_third_party_notices` | — | String | 随软件分发的第三方组件许可证（THIRD_PARTY_NOTICES.md，随安装包安装）。 |

### `commands/assessment.rs`

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `create_quiz` | `input`: CreateQuiz | QuizDetail | AI 出题（学习步骤若有未完成的小测则直接返回它）。 |
| `list_quizzes` | `courseId`: i64 | Vec<Quiz> | 课程的小测记录。 |
| `get_quiz` | `id`: i64 | QuizDetail | 小测详情；交卷前不包含答案、解析和评分要点。 |
| `delete_quiz` | `id`: i64 | () | 删除小测（已计入的掌握度和复习计划不撤销）。 |
| `save_quiz_answer` | `input`: AnswerInput | Question | 保存一道题的答案，或标记跳过。 |
| `set_quiz_position` | `quizId`: i64, `index`: i64 | () | 记住当前所在的题目。 |
| `submit_quiz` | `id`: i64 | QuizDetail | 交卷：客观题判分、主观题 AI 批改；全部有分后更新掌握度、复习计划、错题本和学习步骤。 |
| `regrade_quiz` | `id`: i64 | QuizDetail | 重新让 AI 批改尚未评分的主观题。 |
| `self_grade_question` | `questionId`: i64, `score`: f64 | QuizDetail | 对照参考答案自评一道主观题（0–1 分）。 |
| `list_mistakes` | `courseId`: i64, `includeResolved`: bool | Vec<Mistake> | 错题本（可包含已解决的错题）。 |
| `resolve_mistake` | `id`: i64, `resolved`: bool | () | 标记错题已解决，或移回错题本。 |
| `get_review_queue` | `courseId`: i64 | ReviewQueue | 复习队列与概况（到期、薄弱、疑难、学过未测、错题）。 |
| `get_recommendations` | `courseId`: i64 | Vec<Action> | 下一步建议（由学习状态按规则生成，可直接执行）。 |
| `get_ai_advice` | `courseId`: i64 | Advice | AI 学习建议与优先复习的知识点（只保留本课程存在的知识点）。 |

### `commands/bilibili.rs`

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `get_bilibili_session` | — | Session | 检查本机保存的 B 站登录是否有效，不返回凭据。 |
| `start_bilibili_login` | — | LoginQr | 创建 B 站登录二维码（在本机渲染）。 |
| `poll_bilibili_login` | `key`: String | LoginPoll | 检查扫码状态，成功后将凭据保存到系统凭据管理器。 |
| `cancel_bilibili_login` | `key`: String | () | 取消当前二维码，防止已关闭的登录请求保存凭据。 |
| `forget_bilibili_login` | — | () | 清除本应用保存的 B 站登录凭据（不影响浏览器登录）。 |
| `fetch_bilibili_text` | `url`: String | VideoText | 读取链接所指分 P 的字幕，失败时尝试 AI 总结；不修改网课记录。 |

### `commands/course.rs`

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `create_course` | `input`: CreateCourseInput | Course | 新建课程。 |
| `get_courses` | — | Vec<Course> | 全部课程。 |
| `get_course` | `id`: i64 | Course | 单个课程。 |
| `update_course` | `input`: UpdateCourseInput | Course | 修改名称、简介、考试日期（考试日期传空字符串表示清除）。 |
| `delete_course` | `id`: i64 | () | 删除课程及其全部数据和资料副本。 |

### `commands/document.rs`

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `get_documents` | `courseId`: i64 | Vec<Document> | 课程的资料列表。 |
| `get_document` | `id`: i64 | Document | 单份资料。 |
| `get_document_content` | `id`: i64 | String | 资料全部页面拼接成的纯文本。 |
| `import_document` | `courseId`: i64, `path`: String, `onProgress`: Channel | Document | 复制资料并加入持久化后台队列；返回时不代表解析完成。 |
| `reprocess_document` | `id`: i64, `onProgress`: Channel | Document | 加入后台重新解析队列，失败时保留原结果和章节引用。 |
| `delete_document` | `id`: i64 | () | 删除资料及其章节、片段和图片（原文件不受影响）。 |
| `get_document_sections` | `documentId`: i64 | Vec<Section> | 资料的章节树。 |
| `edit_document_section` | `documentId`: i64, `sectionId`: i64, `edit`: documents::edit::SectionEdit | Vec<Section> | 手动调整章节；保留原文及已有引用，和重新解析互斥。 |
| `get_course_sections` | `courseId`: i64 | Vec<CourseSection> | 课程全部资料的章节（用于章节选择）。 |
| `get_document_pages` | `documentId`: i64, `start`: i64, `end`: i64 | Vec<PageView> | 指定范围的页面 / 单元内容。 |
| `get_page_preview` | `documentId`: i64, `page`: i64, `width`: Option<u32> | String | PDF 原页面预览图（data URL，按需渲染并缓存）。 |
| `get_document_image` | `id`: i64 | String | 提取出的图片（data URL）。 |
| `search_course` | `courseId`: i64, `query`: String, `documentId`: Option<i64>, `limit`: Option<usize> | SearchResult | 课程内关键词检索，返回带高亮的片段和原文位置。 |

### `commands/study.rs`

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `list_knowledge_points` | `courseId`: i64 | Vec<KnowledgePoint> | 课程的知识点（含掌握度与下次复习日期）。 |
| `save_knowledge_point` | `input`: KnowledgeInput | KnowledgePoint | 新建或修改知识点。 |
| `delete_knowledge_point` | `id`: i64 | () | 删除知识点。 |
| `set_knowledge_point_difficult` | `id`: i64, `difficult`: bool | KnowledgePoint | 标记或取消“疑难”。 |
| `set_knowledge_point_status` | `id`: i64, `status`: String | KnowledgePoint | 设为“学过”或“未学”；“已掌握”只能由测验结果授予。 |
| `generate_knowledge_points` | `courseId`: i64, `documentId`: Option<i64> | usize | 从章节标题生成知识点（不需要 AI），返回新增数量。 |
| `extract_knowledge_points` | `courseId`: i64, `sectionId`: i64 | ExtractResult | AI 从某一章提取知识点（结果校验并去重）。 |
| `list_learning_paths` | `courseId`: i64 | Vec<LearningPath> | 课程的学习路径。 |
| `get_learning_path` | `id`: i64 | PathDetail | 学习路径及其全部步骤。 |
| `save_learning_path` | `input`: PathInput | LearningPath | 新建或修改学习路径（名称、说明、模式、是否归档）。 |
| `delete_learning_path` | `id`: i64 | () | 删除学习路径。 |
| `activate_learning_path` | `courseId`: i64, `id`: i64 | StudyState | 设为当前学习路径。 |
| `create_path_from_template` | `input`: TemplateInput | PathDetail | 按章节模板生成学习路径。 |
| `recommend_learning_path` | `input`: RecommendInput | PathDetail | AI 推荐学习路径（每一步都校验能否对应到课程资料）。 |
| `save_path_step` | `input`: StepInput | PathStep | 新建步骤（可插入到指定步骤之后）或修改步骤。 |
| `delete_path_step` | `id`: i64 | () | 删除步骤。 |
| `reorder_path_steps` | `pathId`: i64, `stepIds`: Vec<i64> | Vec<PathStep> | 调整步骤顺序（必须包含路径的全部步骤）。 |
| `set_path_step_status` | `id`: i64, `status`: String, `note`: Option<String> | PathStep | 设置步骤状态：todo / in_progress / done / skipped。 |
| `list_resources` | `courseId`: i64 | Vec<Resource> | 课程的网课与链接（含时间戳）。 |
| `save_resource` | `input`: ResourceInput | Resource | 新建或修改网课（只允许 http/https 链接）。 |
| `delete_resource` | `id`: i64 | () | 删除网课及其时间戳。 |
| `save_resource_mark` | `input`: MarkInput | Resource | 新建或修改时间戳。 |
| `delete_resource_mark` | `resourceId`: i64, `id`: i64 | Resource | 删除时间戳，返回更新后的网课。 |
| `list_notes` | `courseId`: i64 | Vec<Note> | 课程的笔记。 |
| `save_note` | `input`: NoteInput | Note | 新建或修改笔记。 |
| `delete_note` | `id`: i64 | () | 删除笔记。 |
| `get_study_state` | `courseId`: i64 | StudyState | 课程学习状态：当前路径与步骤、上次阅读、上次 AI 学习、进度与近两周活动。 |
| `save_reading_position` | `courseId`: i64, `documentId`: i64, `page`: i64 | () | 记录阅读位置（同一资料 10 分钟内只计一次阅读活动）。 |
| `get_dashboard` | — | Vec<CourseDigest> | 首页概况：每门课程的进度、今日到期复习、未解决错题和考试倒计时。 |



### 文档后台处理与页面断点

- `document_jobs` 保存队列和进度，`document_page_drafts` 保存增量页面；启动时将中断任务重新排队。迁移只新增表，不改写已有资料。
- 初次导入时 `get_document_pages` 可读取已完成的草稿页；重新解析期间继续返回上一次完整结果。章节、搜索和 AI 仍以完整解析结果为准。
- Python `--input` 支持 `--cache-dir`、`--ocr-mode auto|cpu`。`--progress` 在 stderr 输出原有 progress 事件，以及 `{ "page_result": { "page": ..., "images": [...], "total": N } }`；stdout 保持最终协议 v2 JSON。
- PDF 按页缓存位于资料目录 `<course>/<document>_ocr/<fingerprint>/`，以源文件、模型内容、识别模式和缓存版本校验；原子写入完整页后才能复用。Word/PPT 的图片 OCR 使用内容缓存，恢复时无需重复识别已经完成的图片。
- 完整正文与索引最终在同一事务提交；图片目录保留备份直到事务完成，启动时恢复被中断的目录交换。锁定章节校验沿用原规则。
- Windows 安装包使用 `onnxruntime-directml`；不能同时安装 CPU 版 `onnxruntime`。DirectML 顺序推理、禁用内存模式，失败回退 CPU；CPU 使用受内存限制的 spawn 工作进程。关闭父进程后解析器与工作进程自动退出。
