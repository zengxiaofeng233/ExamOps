# ExamOps 快速启动指南

## ✅ 项目已完成初始化

第一阶段基础架构已搭建完成并提交到 Git 仓库。

## 🚀 下一步操作

### 1. 运行开发模式（验证功能）

```bash
npm run tauri:dev
```

这将：
- 启动 Vite 开发服务器 (端口 1420)
- 启动 Tauri 应用窗口
- 启用热更新

**预期结果：**
- 应用窗口打开
- 显示"ExamOps"欢迎页面
- 点击"开始使用"进入课程管理页面
- 可以创建、编辑、删除课程
- 数据持久化到本地 SQLite 数据库

### 2. 构建 Windows 安装包

```bash
npm run tauri:build
```

**构建产物位置：**
```
src-tauri/target/release/bundle/nsis/
```

**安装包文件：**
- `ExamOps_0.1.0_x64-setup.exe` - NSIS 安装程序

### 3. 检查构建结果

```bash
# 查看构建产物
ls -lh src-tauri/target/release/bundle/nsis/

# 查看可执行文件
ls -lh src-tauri/target/release/exam-ops.exe
```

## 📁 项目结构概览

```
ExamOps/
├── src/                    # Vue 3 前端
│   ├── views/             # 页面：HomeView, CoursesView
│   ├── stores/            # Pinia 状态：course.ts
│   ├── services/          # API 调用：course.ts
│   └── router/            # 路由配置
│
├── src-tauri/             # Rust 后端
│   ├── src/
│   │   ├── commands/      # Tauri 命令：course.rs
│   │   ├── database/      # SQLite：schema.rs
│   │   └── models/        # 数据模型：course.rs
│   └── icons/             # 应用图标
│
├── python-sidecar/        # Python 文档处理（待实现）
└── docs/                  # 项目文档
```

## 🧪 功能测试清单

运行 `npm run tauri:dev` 后测试：

- [ ] 应用正常启动
- [ ] 主页显示正常
- [ ] 导航到课程页面
- [ ] 创建新课程
- [ ] 编辑课程信息
- [ ] 删除课程
- [ ] 关闭应用后重新打开，数据仍然存在

## 📊 当前完成度

### ✅ 已完成
- 项目初始化和配置
- 前端界面框架
- 后端 API 接口
- 数据库设计和集成
- 课程 CRUD 功能
- 构建系统配置

### ⏳ 待开发（第二阶段）
- 文档导入功能
- Python Sidecar 集成
- 文档解析和索引
- AI 模型配置
- 问答功能
- 学习状态管理

## 🐛 已知问题

1. **图标为临时占位符**
   - 当前使用基础蓝色"E"字母图标
   - 建议后续替换为正式设计图标

2. **Rust 编译警告**
   - 1 个未使用导入警告（不影响功能）
   - 可运行 `cargo fix` 自动修复

## 📝 Git 提交记录

```bash
# 查看提交记录
git log --oneline

# 推送到远程仓库
git push origin main
```

## 🔗 相关链接

- **仓库地址：** https://github.com/zengxiaofeng233/ExamOps
- **Tauri 文档：** https://tauri.app/
- **Vue 3 文档：** https://vuejs.org/
- **参考项目：** https://github.com/ZeKaiNie/universal-examprep-skill

## 💡 开发建议

### 继续开发前
1. 推送代码到 GitHub
2. 验证开发模式正常运行
3. 测试所有 CRUD 功能

### 第二阶段准备
1. 安装 Python 依赖
   ```bash
   pip install -r python-sidecar/requirements.txt
   ```

2. 设计文档导入界面
3. 实现 Python Sidecar 通信
4. 开发文档解析模块

## ❓ 常见问题

**Q: 开发模式启动慢？**
A: 首次启动需要编译 Rust 代码，约 30-60 秒。后续启动会更快。

**Q: 构建失败？**
A: 确保已安装 Rust 工具链，运行 `rustc --version` 检查。

**Q: 数据存储在哪里？**
A: SQLite 数据库位于系统的应用数据目录：
- Windows: `%APPDATA%\com.examops.app\examops.db`

## 📞 支持

遇到问题请查看：
1. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - 架构文档
2. [docs/PHASE1_SUMMARY.md](docs/PHASE1_SUMMARY.md) - 阶段总结
3. GitHub Issues

---

**祝开发顺利！🎉**
