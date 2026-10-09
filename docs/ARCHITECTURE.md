# ExamOps 架构说明

## 项目概览

ExamOps 采用 Tauri 2 + Vue 3 + Rust 架构，实现单机桌面应用。

## 技术架构

### 前端层 (Vue 3 + TypeScript)

- **框架**: Vue 3 Composition API
- **状态管理**: Pinia
- **路由**: Vue Router
- **样式**: Tailwind CSS 4
- **构建**: Vite 6

**目录结构**:
```
src/
├── components/     # 可复用组件
├── views/          # 页面视图
├── router/         # 路由配置
├── stores/         # Pinia 状态管理
├── services/       # API 服务层
├── types/          # TypeScript 类型定义
└── assets/         # 静态资源
```

### 后端层 (Rust + Tauri)

- **框架**: Tauri 2
- **数据库**: SQLite (rusqlite)
- **异步运行时**: Tokio
- **HTTP 客户端**: Reqwest (用于 AI API 调用)

**目录结构**:
```
src-tauri/src/
├── commands/       # Tauri 命令接口
├── database/       # 数据库模块
├── documents/      # 文档处理模块
├── context/        # 上下文引擎
├── models/         # 数据模型
├── tutor/          # AI 教学引擎
├── study/          # 学习状态管理
└── security/       # 安全模块 (API Key 管理)
```

### 文档处理层 (Python Sidecar)

- **运行模式**: 进程间通信 (标准输入输出)
- **打包方式**: PyInstaller
- **通信协议**: JSON Line Protocol

**目录结构**:
```
python-sidecar/
├── parser/         # 文档解析器
├── chapter/        # 章节切分
├── figures/        # 图片提取
└── main.py         # 入口文件
```

## 数据流

### 1. 课程管理流程

```
用户操作 (Vue) 
  → Store (Pinia)
  → Service Layer (TypeScript)
  → Tauri Command (invoke)
  → Rust Handler
  → SQLite Database
  → 返回结果
```

### 2. 文档处理流程

```
用户导入文档
  → Rust 文件系统操作
  → 启动 Python Sidecar
  → 解析文档内容
  → 提取文本和结构
  → 建立索引
  → 存储到 SQLite
```

### 3. AI 问答流程

```
用户提问
  → Context Engine (构建上下文)
  → Model Gateway (调用 AI API)
  → 流式响应处理
  → 前端实时展示
  → 存储对话记录
```

## 数据库设计

### 核心表结构

1. **courses** - 课程基础信息
2. **documents** - 文档元数据
3. **document_chunks** - 文档切片 (用于 RAG)
4. **document_chunks_fts** - 全文检索索引 (SQLite FTS5)
5. **study_sessions** - 学习会话
6. **chat_messages** - 对话记录
7. **notes** - 笔记

详细 Schema 见 `src-tauri/src/database/schema.rs`

## 关键设计决策

### 1. 为什么使用 Tauri 而非 Electron?

- 更小的安装包体积
- 更低的内存占用
- 原生 Rust 性能
- 更好的安全性

### 2. 为什么选择 SQLite?

- 单用户场景下零配置
- 无需独立数据库服务
- 内置 FTS5 全文检索
- 事务支持完善

### 3. 为什么需要 Python Sidecar?

- 成熟的文档处理库生态 (PyPDF2, python-docx, python-pptx)
- 复杂文档解析逻辑
- 与 Rust 生态互补

### 4. BYOK (Bring Your Own Key) 设计

- 用户完全控制 AI 服务提供商
- 无云端依赖
- 灵活切换不同模型

## 模块解耦设计

所有核心模块均通过接口解耦:

- **Document Engine** - 独立的文档处理接口
- **Context Engine** - 上下文构建策略可替换
- **Model Gateway** - AI 服务抽象层
- **Storage Layer** - 数据持久化抽象

## 安全考虑

1. **API Key 存储**: 使用操作系统的安全存储 (Windows Credential Manager)
2. **SQL 注入防护**: 使用参数化查询
3. **文件路径验证**: 防止路径遍历攻击
4. **Tauri Capabilities**: 最小权限原则

## 未来扩展点

- 向量检索 (替换/增强 FTS5)
- 多语言模型支持
- 插件系统
- 数据导入导出
- 云同步 (可选)

## 性能考虑

1. **懒加载**: 文档内容按需加载
2. **流式响应**: AI 回答实时展示
3. **索引优化**: FTS5 + 自定义索引
4. **异步处理**: Rust async/await + Tokio

## 开发模式 vs 生产模式

**开发模式**:
- Vite 开发服务器 (端口 1420)
- 热更新
- Source maps
- 详细日志

**生产模式**:
- 静态资源打包
- 代码压缩
- NSIS 安装包
- Python Sidecar 捆绑
