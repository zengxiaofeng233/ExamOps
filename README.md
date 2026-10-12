# ExamOps

AI 智能学习桌面应用

## 项目简介

ExamOps 是一款面向学生的 Windows 本地 AI 智能学习软件。允许用户导入本地教材、PPT、PDF、Word 等课程资料，通过自行配置的 AI 模型进行知识点讲解、教材问答、章节学习、笔记整理以及考前复习。

## 技术栈

- **前端**: Vue 3 + TypeScript + Vite + Tailwind CSS 4
- **桌面框架**: Tauri 2
- **后端**: Rust
- **数据库**: SQLite (带 FTS5 全文检索)
- **文档处理**: Python Sidecar
- **状态管理**: Pinia
- **路由**: Vue Router

## 项目特点

- ✅ 单用户本地应用，无需云端服务器
- ✅ 用户自带 AI API (BYOK)
- ✅ 所有数据本地存储
- ✅ 支持多种文档格式 (PDF, PPTX, DOCX, TXT, Markdown)
- ✅ 混合上下文策略 (全文 + RAG 检索)
- ✅ 独立安装包，无需额外环境配置

## 开发环境要求

- Node.js >= 18
- Rust >= 1.70
- Python 3.12 x64（仅开发/构建需要；安装包已内置）
- Windows 10/11

## 快速开始

### 1. 安装依赖

```bash
npm install
```

### 2. 开发模式

```bash
npm run tauri:dev
```

首次运行会在 `python-sidecar/venv` 创建隔离环境，安装依赖并打包本地文档处理程序。
只调整前端或 Rust 时可使用 `npm run tauri -- dev`，复用已构建的 Sidecar。

### 3. 构建安装包

```bash
npm run tauri:build
```

产物位于 `src-tauri/target/release/bundle/nsis/`。Windows x64 安装包包含 Python、OCR 模型和
WebView2 离线安装程序，使用端无需安装 Python，也无需下载 OCR 模型。构建端首次需要联网获取依赖。

## 项目结构

```
ExamOps/
├── src/                       # Vue 前端
│   ├── components/           # 组件
│   ├── views/                # 页面视图
│   ├── router/               # 路由配置
│   ├── stores/               # Pinia 状态管理
│   ├── services/             # 服务层
│   └── types/                # TypeScript 类型定义
│
├── src-tauri/                # Tauri / Rust 后端
│   ├── src/
│   │   ├── commands/         # Tauri 命令
│   │   ├── database/         # 数据库模块
│   │   ├── documents/        # 文档处理
│   │   ├── context/          # 上下文引擎
│   │   ├── models/           # 数据模型
│   │   └── tutor/            # AI 教学引擎
│   └── migrations/           # 数据库迁移
│
├── python-sidecar/           # Python 文档处理
│   ├── parser/               # 文档解析器
│   ├── chapter/              # 章节切分
│   └── figures/              # 图片提取
│
└── shared/                   # 共享类型定义
    └── contracts/
```

## 核心功能模块

### 第一阶段 (当前)

- [x] 基础项目架构
- [x] 课程管理 (CRUD)
- [x] SQLite 数据库集成
- [x] 文档导入功能（课程资料列表、独立副本、重启后保留）
- [x] Python Sidecar 集成
- [x] 正文提取与只读查看（本次确认纳入第一阶段）
- [x] 本地 OCR 兜底（简体中文、繁体中文、英文，覆盖 PDF 混合页面）

### 第二阶段

- [ ] 文档结构化解析与索引（基础正文提取已在第一阶段完成）
- [ ] AI 模型配置
- [ ] 基础问答功能
- [ ] 学习状态管理

### 第三阶段

- [ ] RAG 检索优化
- [ ] 章节式教学
- [ ] 笔记系统
- [ ] 数据导出

## 开发约束

- ❌ 不使用云端后端服务
- ❌ 不开发账号系统
- ❌ 不允许浏览器作为软件入口
- ✅ 必须为独立 Windows 桌面程序
- ✅ 用户自行配置 AI API
- ✅ 数据完全本地存储

## 第一阶段资料使用说明

1. 在课程卡片点击“资料”，选择“导入资料”，可多选 PDF、DOCX、PPTX、TXT 或 Markdown。
2. 每次导入独立保存，不按文件名覆盖或去重；不修改原文件。
3. 点击资料查看提取后的纯文本。提取失败时展示原因，无正文不会标记为成功。
4. PDF 逐页提取，有图片的页面和无文字页面执行 OCR；按位置排除已有文本层的重复文字。
   Word 提取正文、表格和内嵌图片文字；PPT 提取文本框、表格和内嵌图片文字。
5. OCR 模型在需要识别时加载，每个文件处理结束后退出进程。多文件依次处理，导入期间留在资料页。
6. 删除课程会删除关联记录、应用内资料副本及正文；用户选择的原文件保留。

正文保存为纯文本，不恢复原版式，也不承诺公式、手写或复杂表格的识别准确率。
不提取讲者备注、批注；不增加独立图片导入、单独删除资料、全文检索或 AI 功能。
应用中断时，未完成的资料在下次启动标记失败，可重新导入。

## 验证命令

```powershell
npm run build
npm run sidecar:build
cargo test --manifest-path src-tauri/Cargo.toml
python-sidecar/venv/Scripts/python.exe -m unittest discover -s python-sidecar -p "test_*.py" -v
```

Python 测试会生成临时样例，覆盖文本编码、五种格式、离线 OCR、混合页面去重、扫描多页、
错误文件和进程通信。Rust 测试使用临时数据库和文件，验证重复导入、正文持久化、失败记录和删除边界。

## 许可证

MIT

## 贡献

欢迎提交 Issue 和 Pull Request！

## 相关链接

- 参考项目: [Universal ExamPrep Skill](https://github.com/ZeKaiNie/universal-examprep-skill)
- 仓库地址: [ExamOps](https://github.com/zengxiaofeng233/ExamOps)
