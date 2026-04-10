# `ding` 开发进度清单（当前基线）

更新时间：2026-04-10

> 本清单基于当前新方向维护。  
> Claude 后续以“交互式 TUI + 用户级 hooks”方案为准。

## Phase 1: 基础骨架与控制面

- `[x]` Tauri v2 项目初始化（React + TypeScript + Vite）
- `[x]` Rust 基础数据模型（Instance / DingStatus / PendingAction / Adapter trait）
- `[x]` Instance Manager 基础架构
- `[x]` 悬浮窗胶囊态 UI
- `[x]` 详情面板 UI（实例列表 / ActionPanel / LogViewer）
- `[x]` CSS 设计系统与状态动画
- `[x]` CLI 基础命令解析（`ding claude` / `ding codex` / `ding run` / `ding list` / `ding kill`）
- `[x]` daemon 控制面可用（当前为本地 loopback TCP）
- `[x]` Tauri Events 与前端 store 基础链路

## Phase 2: Claude 交互式 TUI + Hooks MVP

- `[x]` 方案定稿：Claude 不再以 `-p --output-format stream-json` 为主线
- `[x]` 明确用户级 hooks 安装策略（`~/.claude/settings.json`）
- `[x]` 明确 Claude MVP hooks 事件范围
- `[x]` `ding claude` 启动后保持与直接执行 `claude` 一致的原生入口行为
- `[x]` `ding` 自动安装/更新用户级 Claude hooks
- `[x]` 用户级 hook relay 入口打通（当前实现为 `ding hook-relay`）
- `[x]` Claude `session_id` 到 `ding` 实例的基础映射
- `[ ]` `PermissionRequest -> UI -> 决策 -> Claude` 审批闭环
- `[x]` `SessionStart / PostToolUse / Stop / SessionEnd` 的基础状态更新与日志映射

## Phase 3: Codex 结构化监控补完

- `[x]` Codex JSONL 事件解析骨架
- `[x]` daemon 可通过 `ding codex` 发起实例
- `[ ]` Codex 可执行文件定位与更明确的错误提示
- `[ ]` Codex 审批回传闭环
- `[ ]` `apply_patch` diff 预览
- `[ ]` Codex kill 真正结束底层进程

## Phase 4: Generic + SDK

- `[ ]` Generic Adapter 最小基建
- `[ ]` `ding run <program>` 最小可用闭环
- `[ ]` Python ding-sdk
- `[ ]` Node.js ding-sdk

## Phase 5: 体验打磨

- `[ ]` 系统提示音
- `[ ]` 系统托盘与菜单
- `[ ]` UI 过渡动效打磨
- `[ ]` 子代理/任务可视化
- `[ ]` 更细粒度的状态推断
- `[ ]` 日志虚拟化视图

## 当前说明

- Claude 当前代码中仍保留旧的非交互 `stream-json` 适配器代码，但它不再是后续目标方案
- 当前已完成“原生 Claude 入口 + 用户级 hooks 安装 + 基础 hooks 事件回传”
- 下一阶段重点是 Claude 审批闭环
