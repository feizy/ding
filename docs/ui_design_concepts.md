# `ding` 界面视觉概念设计

基于我们的技术方案，我为你生成了 `ding` 悬浮窗的几个核心视觉组件概念图。

由于该工具旨在提升 Human-in-the-loop 效率，视觉设计（UI/UX）的核心理念是：**不打扰（Idle时极简）与 强提醒（Action Required时高亮）的结合**。

采用深色磨砂玻璃风格（Dark Glassmorphism），带来极致的高级感。

````carousel
![闲置胶囊态模型 (Capsule - Idle/Normal)](/C:/Users/HP/.gemini/antigravity/brain/264e109e-470f-4fd2-b7f7-5f81775fceb9/ding_capsule_idle_1775716263472.png)
### 1. 胶囊态 (Idle / Thinking / Running)
这是 `ding` 默认展现的形态。它悬浮在屏幕右上角或顶部，就像一个灵动岛（Dynamic Island）。它不占用太多空间，只有一个状态灯和当前最高优先级实例的简介。

<!-- slide -->
![强唤醒胶囊态 (Capsule - Action Required)](/C:/Users/HP/.gemini/antigravity/brain/264e109e-470f-4fd2-b7f7-5f81775fceb9/ding_capsule_flashing_1775716281142.png)
### 2. 强唤醒态 (Action Required)
当 Claude Code 的 hooks 触发 `PermissionRequest` 等需要人工决策的事件，或者 Codex 抛出拦截审批时，胶囊开始改变色调（红色/橙色）并高亮闪烁，配合系统提示音，迅速捕捉用户的注意力。

<!-- slide -->
![详情面板态 (Detail Panel - Multi-instance)](/C:/Users/HP/.gemini/antigravity/brain/264e109e-470f-4fd2-b7f7-5f81775fceb9/ding_detail_panel_1775716305257.png)
### 3. 详情与审批面板 (Detail Panel)
点击胶囊后展开。这里按优先级（Action Required > Error > Thinking...）列出所有并发的 Agent 实例。
注意顶部的红色高亮卡片：它直接展示了需要审批的工具和输入命令，并给出了明确的 **Approve / Deny** 操作按钮。下方则是其他处于分析和等待状态的 Agent。
````

### 动画与交互细节设计：
- **状态灯呼吸**：`Running` 状态使用较明显的蓝色闪动；`Thinking` 若后续能稳定从事件中推断，再作为增强态加入。
- **平滑展开**：点击胶囊时，高度不是硬切出来的，而是带有阻尼感的平滑向下弹出。
- **快捷审批**：用户可以在面板中直接点击 Approve，数据立刻通过本地 daemon 控制面回传，前端卡片进入 Running 状态，无缝衔接。

> [!NOTE]
> 这些是概念图（Concept Mockups）用于确认视觉方向。视觉方向保持有效，但 Claude 的后续实现以“交互式 TUI + 官方 hooks”方案为准。

你觉得这个视觉方向符合你的期望吗？如果OK的话，我们就可以开始 Phase 1 的项目初始化跟编码了！
