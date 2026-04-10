# `ding` 当前问题清单（按新方案）

更新时间：2026-04-10

本文记录当前代码库相对于“Claude 交互式 TUI + 用户级 hooks”新方案的主要缺口，避免继续沿着已废弃方向推进。

## 高优先级

### 1. Claude 审批闭环仍未完成

- 现状：
  当前 Claude hooks 已能回传基础事件并驱动基础状态更新，但 `PermissionRequest -> UI -> 决策 -> Claude` 还未打通。
- 风险：
  能看到权限请求趋势，但还不能通过 `ding` 完成真正的交互式审批闭环。

### 2. Claude 旧的 `stream-json` 适配器代码仍在仓库中

- 现状：
  仓库中还保留旧的 Claude `stream-json` 适配器实现。
- 风险：
  容易误导后续开发继续维护已废弃路径。
- 新方向：
  Claude 改为交互式 TUI + 官方 hooks。

### 3. 当前 hook relay 入口仍偏向过渡实现

- 现状：
  当前用户级 hooks 调用的是 `ding` 主程序的隐藏 `hook-relay` 子命令。
- 风险：
  这已经可用，但后续若需要更轻量或更独立的部署形态，可能还需要评估是否恢复成专用 `ding-hook` 可执行文件。

## 中优先级

### 4. Codex 可执行文件定位仍不稳定

- 现状：
  `ding codex` 仍依赖 `Command::new("codex")`。
- 风险：
  在 PATH 解析不稳定或 Windows shim 场景下容易出现 `program not found`。

### 5. Codex 审批回传闭环仍未补完

- 现状：
  Codex 已能解析审批事件，但 UI 决策回传和底层会话恢复仍不可靠。
- 风险：
  审批卡片看起来可点，但 agent 不一定真正收到决策。

### 6. `ding kill` 仍未真正结束底层进程

- 现状：
  当前 kill 逻辑仍以实例移除为主。
- 风险：
  UI 里实例消失，但 Claude / Codex 进程可能继续运行。

### 7. Tauri 侧 create command 仍是 demo 风格

- 现状：
  `create_claude_instance` / `create_codex_instance` 仍带 demo 痕迹。
- 风险：
  容易误导后续实现者把它们当成正式入口。

## 低优先级

### 8. 仍需补项目级测试

- 现状：
  已补一个 IPC 启动回归测试，但整体测试覆盖仍然很薄。
- 建议：
  优先补 daemon / Claude hook / Codex 审批 的最小回归测试。

### 9. 编译告警与乱码仍未清理

- 现状：
  还有未使用导入、未使用字段和部分文案乱码。
- 风险：
  会持续降低调试效率。

## 建议修复顺序

1. Claude 审批闭环
2. 清理/隔离旧的 Claude `stream-json` 路径
3. Codex 可执行文件定位
4. Codex 审批闭环
5. kill 真正结束进程
6. 测试、告警、细节清理
