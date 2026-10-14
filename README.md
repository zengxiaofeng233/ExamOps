# ExamOps

基于个人学习资料的 AI 智能复习桌面软件（Windows）。

导入自己的教材、PPT、讲义和网课笔记，用自己配置的 AI 模型按章节讲解、回答问题、出题测验，并根据掌握情况安排复习。单用户、本地运行：没有账号，没有云端存储，所有数据只保存在本机。

## 功能

- **资料**：导入 PDF（含扫描版，本地 OCR）、Word、PPT、TXT、Markdown；自动识别章节、提取图片，保留页码 / 幻灯片 / 段落 / 行号位置；全文检索，阅读器支持原页面预览。
- **AI 问答**：回答只依据你的资料，区分【教材】【补充】【推导】并标注来源编号，点击可跳到原文；流式输出，可随时停止、重新生成。
- **AI 学习**：从零学习（分段讲解、每段停下来等你）、正常复习（先回忆再纠正）、考前冲刺（必考点与典型题）；也可以根据你提供的字幕和笔记讲解网课。
- **知识点与学习路径**：从章节标题或用 AI 生成知识点；按章节生成、AI 推荐或手动安排学习路径，可拖动排序、插入、跳过。
- **测验**：单选、判断、简答、自由复述、情景应用五种题型；客观题自动判分，主观题 AI 批改并给出反馈，失败时可重新批改或自评。
- **复习**：掌握度与间隔复习（临近考试自动缩短间隔）、薄弱知识点、错题本与错题重练、下一步建议和 AI 复习建议。
- **网课与笔记**：保存网课链接、字幕、时间戳；笔记支持 Markdown 和公式，并关联资料位置、章节和知识点。
- **BYOK**：任何兼容 OpenAI Chat Completions 的接口（DeepSeek、通义千问、智谱、Kimi、OpenAI、Ollama 本机模型等）；API Key 保存在 Windows 凭据管理器中。

## 界面

| 资料阅读 | AI 学习 |
| --- | --- |
| ![资料阅读：章节目录与正文](docs/images/reader.png) | ![AI 学习：分段讲解与来源标注](docs/images/learn.png) |
| **测验结果** | **复习** |
| ![测验结果：得分与知识点掌握情况](docs/images/quiz.png) | ![复习：复习概况、下一步建议与错题本](docs/images/review.png) |

截图中的 AI 内容来自本地模拟服务（`scripts/mock_ai.py`），实际使用时由你配置的模型生成。

## 文档

| 文档 | 内容 |
| --- | --- |
| [安装与首次使用](docs/INSTALL.md) | 系统要求、安装、配置模型、数据位置与备份、卸载、常见问题 |
| [使用指南](docs/USER_GUIDE.md) | 每个页面和功能的用法 |
| [架构](docs/ARCHITECTURE.md) | 七个引擎、数据模型、主要流程、安全设计 |
| [命令参考](docs/API.md) | 前端调用的全部 Tauri 命令 |
| [测试](docs/TESTING.md) | 自动化测试、异常 / 断网 / 持久化测试、模拟 AI 服务、发布前验收 |
| [更新日志](CHANGELOG.md) | 按路线图阶段记录的变更 |
| [开发路线图 V1.4](dev_roadmap/v1.4.md) | 需求与阶段划分 |

## 技术栈

Tauri 2 · Vue 3 + TypeScript · Tailwind CSS 4 · Pinia · Rust · SQLite + FTS5 · Python Sidecar（PyInstaller 打包，RapidOCR + pypdfium2）· NSIS 安装包

## 开发

### 环境

- Node.js 20+、Rust（stable）、Python 3.12 x64（只在开发和构建时需要，安装包已内置运行环境）
- Windows 10/11；Rust 核心和前端也可以在 Linux / macOS 上开发和测试（Linux 需要 `libwebkit2gtk-4.1-dev` 等 Tauri 依赖）

### 常用命令

```bash
npm install
npm run tauri:dev        # 打包文档处理程序（首次会创建 python-sidecar/venv），然后启动开发模式
npm run tauri -- dev     # 只改前端或 Rust 时使用，复用已打包的文档处理程序
npm test                 # 前端单元测试；类型检查：npx vue-tsc --noEmit
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
cd python-sidecar && python -m unittest -v
```

没有 API Key 时可以用 `python scripts/mock_ai.py` 启动本地模拟 AI 服务，详见 [测试](docs/TESTING.md#3-本地模拟-ai-服务)。

### 构建安装包

安装包必须在 Windows x64 上构建：

```bash
npm run tauri:build      # 产物：src-tauri/target/release/bundle/nsis/ExamOps_<版本>_x64-setup.exe
```

也可以在 GitHub Actions 中手动运行 **Windows installer** 工作流，或推送 `v*` 标签自动构建并创建草稿 Release。安装包内置文档处理程序、OCR 模型和 WebView2 离线安装程序，按当前用户安装，不需要管理员权限。

### 目录

```
ExamOps/
├── src/                     # Vue 前端：views（页面）、components、stores（Pinia）、services（命令调用）、types、utils
├── src-tauri/
│   ├── src/
│   │   ├── commands/        # Tauri 命令（前端唯一入口）
│   │   ├── documents/       # Document Engine：导入、结构化、阅读、删除
│   │   ├── search/          # FTS5 检索与中文分词
│   │   ├── ai/              # 模型网关：OpenAI 兼容客户端、流式解析、错误映射、模型配置
│   │   ├── context/         # Context Engine：全文 / 检索 / 自动
│   │   ├── tutor/           # Tutor Engine：问答与分章教学
│   │   ├── knowledge/ paths/ resources/ notes/ study/   # 知识点、学习路径、网课、笔记、学习状态
│   │   ├── assessment/ mastery/ review/                 # 测验、掌握度与间隔复习、复习建议
│   │   ├── database/        # 连接与版本化迁移
│   │   └── security/        # API Key 存储
│   └── migrations/          # 0001–0006 数据库迁移（只新增，不修改已发布的迁移）
├── python-sidecar/          # 文档解析、章节识别、图片提取、OCR、页面渲染
├── scripts/                 # 打包文档处理程序、生成命令参考与开源许可、模拟 AI 服务
└── docs/
```

### 开发规范

- 每个功能在独立分支开发（`feat/*`），通过 Pull Request 合并；CI 在 Ubuntu 和 Windows 上运行全部测试。
- 数据库结构只通过新增迁移文件修改（`src-tauri/migrations/` 新增文件并在 `database/migrations.rs` 中登记），旧数据自动升级。
- 前后端数据结构统一定义：Rust 结构体与 `src/types` 中的 TypeScript 类型一一对应；新增命令时同步更新 `src/services` 并运行 `python scripts/command_reference.py` 更新命令参考。
- 新增或升级依赖后运行 `python scripts/third_party_notices.py` 更新开源许可清单。

## 许可与致谢

ExamOps 以 [MIT 许可证](LICENSE) 发布。

学习方法设计（回答区分教材 / 补充 / 推导、按章节讲解、错题再练等）参考了 [Universal ExamPrep Skill](https://github.com/ZeKaiNie/universal-examprep-skill)（MIT 许可，Copyright (c) 2026 ZeKaiNie）。ExamOps 没有复制其源代码，其版权与许可声明保留在 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 中，该文件同时列出了所有随软件分发的开源组件及其许可证，并随安装包一起安装，也可以在应用的“设置 → 关于 → 查看开源许可”中查看。
