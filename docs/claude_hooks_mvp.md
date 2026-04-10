# Claude 交互式 TUI + Hooks 监控方案（MVP）

更新时间：2026-04-10

## 目标

为 `ding claude` 提供以下行为：

- 当前终端中的 Claude Code 行为与直接在当前目录执行 `claude` 保持一致
- `ding` 不再依赖 `claude -p --output-format stream-json`
- `ding` 通过 Claude Code 官方 hooks 接收结构化事件
- 悬浮窗继续提供实例状态、审批、关键日志和错误提示
- hooks 采用用户级安装方式，直接运行 `claude` 也默认带上 `ding` hooks

## 官方 hooks 中第一期接入范围

以下事件符合 `ding` 的监控要求，作为当前实现与 MVP 首批接入：

- `SessionStart`
- `PreToolUse`
- `PostToolUse`
- `Notification`
- `Stop`
- `SubagentStop`
- `SessionEnd`

### 子代理扩展事件

如果后续需要在 UI 中展示子代理/任务活动，可追加：

- `SubagentStart`
- `SubagentStop`
- `TaskCreated`
- `TaskCompleted`

## 不纳入 MVP 的事件

以下事件先不接入，避免第一期范围过大：

- `UserPromptSubmit`
- `InstructionsLoaded`
- `ConfigChange`
- `CwdChanged`
- `PreCompact`
- `PostCompact`
- `Elicitation`
- `ElicitationResult`
- `FileChanged`
- `WorktreeCreate`
- `WorktreeRemove`
- `TeammateIdle`

这些事件不是没价值，而是对第一期“交互式 TUI + 监控 + 审批”不是必需。

## 事件到 ding 状态的映射

### 1. `SessionStart`

用途：

- 创建或激活一个 Claude 实例
- 记录 `session_id`
- 绑定当前工作目录

建议映射：

- 创建实例
- 初始状态设为 `idle`
- 写入系统日志：`Claude session started`

### 2. `PreToolUse`

用途：

- 记录工具即将执行
- 为审批类工具准备 `PendingAction`

建议映射：

- 如果工具不需要人工审批：状态设为 `running`
- 如果工具需要审批：状态设为 `action_required`
- 日志中记录工具名和输入摘要

### 3. `PreToolUse` 审批职责

当前实现中，Claude 审批闭环以 `PreToolUse` 为主：

- 对需要拦截的工具，直接在 `PreToolUse` 阶段生成 `PendingAction`
- `hook-relay` 阻塞等待 `ding` 决策
- 决策结果以 hook JSON 返回给 Claude

### 4. `PostToolUse`

用途：

- 工具执行成功

建议映射：

- 清理对应 `PendingAction`
- 记录结构化日志
- 状态设为 `running`

### 5. `Notification`

用途：

- 捕获需要用户注意的通知
- 例如权限提示、空闲提示、需要输入等

建议映射：

- 追加系统日志
- 必要时提升胶囊可见性
- 某些通知可映射为轻量 `action_required`

### 6. `Stop`

用途：

- Claude 当前轮次结束

建议映射：

- 如果会话仍存活但当前回合结束：状态设为 `idle`
- 如果本次实例按产品定义视为“完成”：状态设为 `finished`

MVP 建议：

- CLI 退出时由 `SessionEnd` 负责最终收口
- `Stop` 先映射为 `idle`

### 7. `SubagentStop`

用途：

- 记录子代理结束

建议映射：

- 先作为普通系统日志

### 8. `SessionEnd`

用途：

- 会话最终结束

建议映射：

- 状态设为 `finished`
- 记录结束时间
- 清理会话级临时状态

## 建议的实例识别规则

由于用户之后可能直接运行 `claude`，而不是通过 `ding claude` 启动，因此实例识别必须依赖 hook 事件本身，而不是一次性临时环境变量。

MVP 建议：

- 以 `session_id` 作为 Claude 会话主键
- 如果 daemon 中不存在该 `session_id` 对应实例，则自动创建
- 实例显示名可优先取：
  1. 首次事件的当前目录名
  2. 用户配置中的默认别名
  3. `Claude Code`

## hook 回传的最小数据要求

`ding-hook` 从 Claude hook 的 `stdin` 读取 JSON 事件，归一化后发给 daemon。

最小保留字段：

- `hook_event_name`
- `session_id`
- `cwd`
- `tool_name`
- `tool_input`
- `message`
- `timestamp`

如果官方事件中包含更丰富字段，也应原样保留在原始 payload 中，便于后续扩展。

## hook relay 入口的职责

当前实现中，Claude hooks 调用的是 `ding` 主程序的隐藏 `hook-relay` 子命令。

这个 hook relay 入口在 MVP 中只负责三件事：

1. 读取 Claude hook 事件 JSON
2. 转发给本地 `ding` daemon
3. 对需要决策的事件，阻塞等待 daemon/UI 返回审批结果

这个 hook relay 入口不负责：

- 解析终端屏幕
- 推断 assistant 文本
- 保存完整会话上下文

## 用户级安装策略

`ding` 管理 `~/.claude/settings.json` 中的 hooks 条目。

要求：

- 只写入 `ding` 自己维护的 hooks 项
- 不覆盖用户已有的非 `ding` hooks
- 支持幂等更新
- 支持未来卸载/禁用

## 第一阶段验收标准

满足以下条件即视为 Claude Hooks MVP 可用：

1. 执行 `ding claude` 后，当前终端显示原生 Claude TUI
2. 不修改当前目录与原生 Claude 的交互行为
3. 用户级 hooks 能自动写入 `~/.claude/settings.json`
4. `SessionStart` 能在 `ding` 中创建实例
5. `PreToolUse` 能生成待审批动作并阻塞等待 `ding` 决策
6. `PostToolUse` 能更新日志和状态
7. `SessionEnd` 能正确结束实例

以下能力仍属于下一阶段：

- 真实 Claude 交互式会话下的完整 hooks 行为验证
- 更细粒度的失败状态映射

## 后续扩展方向

- 子代理事件可视化
- 用户输入轮次记录
- 上下文压缩事件可视化
- 工作目录切换监控
- 更细粒度的状态推断（如 `thinking` 与 `running` 拆分）

## 参考

参考 Claude Code 官方 hooks 文档的事件生命周期与事件类型说明：

- `https://code.claude.com/docs/en/hooks`
