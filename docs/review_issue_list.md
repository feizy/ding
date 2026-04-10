# `ding` 当前问题清单（按新方案）

更新时间：2026-04-10

本文记录当前代码库相对于“Claude 交互式 TUI + 用户级 hooks”新方案的主要缺口，避免继续沿着已废弃方向推进。

## 高优先级

### 1. Claude 仍停留在旧的非交互 `stream-json` 适配器

- 现状：
  当前代码中的 Claude 适配器仍通过 `claude -p --output-format stream-json` 启动会话。
- 风险：
  与新要求冲突，无法满足“当前终端显示与直接运行 `claude` 完全一致的 TUI”。
- 新方向：
  Claude 改为交互式 TUI + 官方 hooks。

### 2. 用户级 Claude hooks 自动安装尚未实现

- 现状：
  已确定采用 `~/.claude/settings.json` 用户级安装策略，但代码还没有配置合并、幂等更新和持久管理逻辑。
- 风险：
  无法做到“之后直接运行 `claude` 也默认带上 `ding` hooks”。

### 3. `ding-hook` 仍偏向旧的单次会话拦截器设计

- 现状：
  当前 `ding-hook` 更接近一次性审批桥接器，不是长期有效的用户级 hook 入口。
- 风险：
  难以承接 `SessionStart / PermissionRequest / SessionEnd` 等持续事件流。

### 4. Claude 实例归属机制仍未切到 `session_id`

- 现状：
  新方案要求用 Claude hook 事件里的 `session_id` 识别实例，但当前实现尚未完成这层绑定。
- 风险：
  之后直接运行 `claude` 时，`ding` 无法稳定归并到同一个实例模型。

## 中优先级

### 5. Codex 可执行文件定位仍不稳定

- 现状：
  `ding codex` 仍依赖 `Command::new("codex")`。
- 风险：
  在 PATH 解析不稳定或 Windows shim 场景下容易出现 `program not found`。

### 6. Codex 审批回传闭环仍未补完

- 现状：
  Codex 已能解析审批事件，但 UI 决策回传和底层会话恢复仍不可靠。
- 风险：
  审批卡片看起来可点，但 agent 不一定真正收到决策。

### 7. `ding kill` 仍未真正结束底层进程

- 现状：
  当前 kill 逻辑仍以实例移除为主。
- 风险：
  UI 里实例消失，但 Claude / Codex 进程可能继续运行。

### 8. Tauri 侧 create command 仍是 demo 风格

- 现状：
  `create_claude_instance` / `create_codex_instance` 仍带 demo 痕迹。
- 风险：
  容易误导后续实现者把它们当成正式入口。

## 低优先级

### 9. 仍需补项目级测试

- 现状：
  已补一个 IPC 启动回归测试，但整体测试覆盖仍然很薄。
- 建议：
  优先补 daemon / Claude hook / Codex 审批 的最小回归测试。

### 10. 编译告警与乱码仍未清理

- 现状：
  还有未使用导入、未使用字段和部分文案乱码。
- 风险：
  会持续降低调试效率。

## 建议修复顺序

1. Claude 交互式 TUI 启动链路
2. 用户级 hooks 安装与幂等合并
3. `ding-hook` 长期化与 `session_id` 归属
4. Claude 审批闭环
5. Codex 可执行文件定位
6. Codex 审批闭环
7. kill 真正结束进程
8. 测试、告警、细节清理
