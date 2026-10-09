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
- Python >= 3.8
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

### 3. 构建安装包

```bash
npm run tauri:build
```

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
- [ ] 文档导入功能
- [ ] Python Sidecar 集成

### 第二阶段

- [ ] 文档解析与索引
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

## 许可证

MIT

## 贡献

欢迎提交 Issue 和 Pull Request！

## 相关链接

- 参考项目: [Universal ExamPrep Skill](https://github.com/ZeKaiNie/universal-examprep-skill)
- 仓库地址: [ExamOps](https://github.com/zengxiaofeng233/ExamOps)
