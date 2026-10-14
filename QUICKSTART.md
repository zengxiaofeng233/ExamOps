# 快速上手

只想使用 ExamOps：安装后按 [安装与首次使用](docs/INSTALL.md) 配置模型即可。下面是开发者从克隆到走通完整学习流程的最短路径。

## 1. 准备环境（Windows 10/11）

- Node.js 20+、Rust stable（含 MSVC 构建工具）、Python 3.12 x64
- 克隆仓库后安装前端依赖：

```bash
npm install
```

## 2. 启动开发模式

```bash
npm run tauri:dev
```

首次运行会在 `python-sidecar/venv` 创建虚拟环境、安装依赖并用 PyInstaller 打包文档处理程序（需要几分钟）；之后只改前端或 Rust 时用 `npm run tauri -- dev` 跳过这一步。应用数据保存在 `%APPDATA%\com.examops.app`，可在 **设置 → 数据与存储** 中打开。

## 3. 不花钱接入 AI

另开一个终端启动本地模拟 AI 服务：

```bash
python scripts/mock_ai.py
```

在 ExamOps 的 **设置 → AI 模型 → 添加模型** 中填写：接口地址 `http://127.0.0.1:18080/v1`，模型名称 `mock`，API Key `sk-test`，点击“测试连接”后保存。要用真实模型时，选择服务商预设并填入自己的 API Key。

## 4. 走一遍学习闭环

1. **我的课程 → 创建课程**，设置考试日期。
2. **资料 → 导入资料**，选择一份教材 PDF 或讲义，等待解析完成；在阅读器中查看章节目录、原页面和检索。
3. **知识点 → 从章节标题生成**（或 **AI 提取**）。
4. **学习路径 → 新建学习路径 → 按章节生成**，勾选“每章结束后加入一次小测”。
5. 在路径中点击第一步的“开始”，进入 **AI 学习**：继续讲解、举个例子、跳过下一部分，最后“标记已学完”。
6. 按提示开始小测，作答并交卷，查看批改结果和掌握度。
7. 打开 **复习**：查看复习队列、错题本和 AI 学习建议。
8. 用 `curl http://127.0.0.1:18080/__fail/drop` 让模拟服务在回答中途断开，观察中断提示；`/__fail/none` 恢复。

## 5. 提交前

```bash
npx vue-tsc --noEmit && npm test
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
cd ../python-sidecar && python -m unittest
```

改了数据库结构时新增迁移文件（`src-tauri/migrations/000N_*.sql`）并在 `src-tauri/src/database/migrations.rs` 中登记，不要修改已发布的迁移；改了命令时运行 `python scripts/command_reference.py` 更新 [命令参考](docs/API.md)。更多说明见 [README](README.md) 和 [测试](docs/TESTING.md)。
