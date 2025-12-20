# ProxyCast 版本检查功能 - 修改文件

本文件夹包含实现版本检查功能时修改的所有文件。

## 功能概述

- 添加 GitHub API 版本检查功能
- 实现版本比较逻辑
- 更新前端显示新版本和下载链接
- 修复字段名匹配问题 (has_update -> hasUpdate)
- 更新仓库链接到新地址
- 更新文档链接到 https://aiclientproxy.github.io/proxycast/
- 统一版本号到 0.13.0

## 修改文件列表

### 后端文件 (Rust)
1. **src-tauri/src/commands/config_cmd.rs** - 核心版本检查逻辑
2. **src-tauri/src/lib.rs** - 命令注册
3. **src-tauri/Cargo.toml** - 版本号和仓库信息

### 前端文件 (TypeScript/React)
4. **src/components/settings/AboutSection.tsx** - 版本检查 UI

### 配置文件
5. **package.json** - 版本号和仓库信息
6. **package-lock.json** - 版本号同步

## 主要变更

### 后端变更
- 新增 `VersionCheckResult` 结构体
- 新增 `check_for_updates()` 异步函数
- 实现语义化版本比较逻辑
- 集成 GitHub Releases API
- 添加 serde 字段重命名支持

### 前端变更
- 更新版本检查接口调用
- 改进 UI 显示逻辑
- 添加新版本提示和下载链接
- 更新文档链接

### 配置变更
- 统一所有文件版本号到 0.13.0
- 更新仓库地址到 aiclientproxy 组织
- 更新主页链接

## 文件结构
```
modified_files/
├── README.md                                    # 本说明文件
├── package.json                                 # 前端配置
├── package-lock.json                            # 依赖锁定
├── src/
│   └── components/
│       └── settings/
│           └── AboutSection.tsx                 # 关于页面组件
└── src-tauri/
    ├── Cargo.toml                              # Rust 项目配置
    └── src/
        ├── lib.rs                              # 主库文件
        └── commands/
            └── config_cmd.rs                   # 配置命令
```

## 使用说明

这些文件可以直接替换原项目中的对应文件，实现完整的版本检查功能。

生成时间: 2025-12-20