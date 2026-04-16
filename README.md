# ding User Manual

ding is a floating monitor for Claude Code. It keeps Claude Code's native terminal UI intact while giving you a small always-on-top window that watches for tool activity, permission prompts, and user-input requests.

## Why ding Exists

Have you ever left Claude Code running in the background, only to discover much later that it was stuck on a permission-required prompt?

This is especially common with compound commands, file writes, package installs, git operations, or any workflow where Claude Code needs your approval before continuing. Without a visible reminder, Claude can sit idle for minutes or hours while you think it is still working.

With ding, you can keep vibecoding while watching YouTube, browsing, reading docs, or doing something else. The ding floating window stays visible and alerts you whenever Claude Code needs any action from you. When Claude needs permission, a selection, or another blocking response, ding brings that interaction to the surface so you can respond immediately without constantly checking the terminal.

## What ding Does

- Launches Claude Code normally with `ding claude`.
- Keeps the Claude Code TUI behavior the same as running `claude` directly.
- Shows a floating always-on-top status window.
- Displays which tool Claude Code is currently using.
- Alerts you when Claude Code needs permission or input.
- Mirrors Claude Code permission choices, including allow, deny, and always-allow options.
- Mirrors Claude Code user-question options when Claude asks you to choose from a list.
- Lets you close ding from the floating window's right-click menu.

## Requirements

- Windows 10 or Windows 11.
- Claude Code installed and available from your terminal as `claude`.
- A valid Claude Code authentication setup.

Check Claude Code first:

```powershell
claude --version
```

If this command does not work, install or fix Claude Code before installing ding.

## Installation

ding is distributed as a Windows installer.

Recommended installer:

```text
ding_0.1.0_x64-setup.exe
```

Alternative MSI installer:

```text
ding_0.1.0_x64_en-US.msi
```

Install steps:

1. Run `ding_0.1.0_x64-setup.exe`.
2. Follow the installer prompts.
3. Ensure the installed `ding.exe` is available in your `PATH`.
4. Open a new PowerShell terminal.
5. Run:

```powershell
ding claude
```

If Windows cannot find `ding`, add the ding installation directory to your `PATH`, then open a new terminal.

## First Run

Run:

```powershell
ding claude
```

On first run, ding will:

1. Start the ding floating window in the background if it is not already running.
2. Install or update ding's Claude Code hooks in your user-level Claude settings.
3. Start the native Claude Code TUI in the current terminal.

The Claude Code terminal experience should remain the same as running:

```powershell
claude
```

The difference is that ding is now watching the session and will surface important blocking events.

## Daily Usage

Start Claude Code through ding:

```powershell
ding claude
```

You can pass normal Claude Code arguments:

```powershell
ding claude --permission-mode default
```

The floating window will show:

- Current Claude Code session.
- Current status.
- Current tool name, such as `Bash`, `Edit`, or `AskUserQuestion`.
- Permission prompts.
- User-input choices.
- Recent activity.

## Handling Permission Prompts

When Claude Code needs permission, the Claude Code TUI may show a prompt such as:

```text
Do you want to proceed?
1. Yes
2. Yes, and always allow access to ...
3. No
```

ding mirrors that interaction in the floating window. You can respond directly in ding instead of switching back to the terminal.

Examples of actions ding can surface:

- Allow a command once.
- Always allow a suggested permission rule.
- Deny a tool call.
- Choose an option when Claude Code asks a question.
- Submit a user-input response requested by Claude Code.

## Closing ding

Right-click the ding floating window.

You will see a ding-specific menu with:

```text
关闭 ding
```

Select it to close the ding floating window and daemon.

## Important Behavior Notes

ding installs Claude Code hooks into your user-level Claude settings, but those hooks are gated. They only activate for Claude sessions launched through:

```powershell
ding claude
```

Running `claude` directly keeps normal Claude Code behavior and does not activate ding monitoring.

## Troubleshooting

### `ding` is not recognized

The installation directory is not in your `PATH`.

Fix:

1. Find the installed `ding.exe`.
2. Add its folder to your user `PATH`.
3. Open a new PowerShell terminal.
4. Run:

```powershell
ding claude
```

### The floating window does not appear

Try:

```powershell
ding list
```

If ding is installed correctly, this should contact the background daemon or start it if needed.

You can also close any stale ding process and retry:

```powershell
Get-Process ding -ErrorAction SilentlyContinue | Stop-Process -Force
ding claude
```

### Claude Code starts, but ding does not show activity

Run `ding claude` once from the installed ding path. This refreshes the Claude Code hook commands to point at the installed `ding.exe`.

If you moved `ding.exe` after installation, run:

```powershell
ding claude
```

again so the hook paths are updated.

### You see `localhost refused to connect`

That happens when using a development/debug build directly, such as:

```powershell
src-tauri\target\debug\ding.exe
```

The formal product build does not depend on `localhost` or a Vite dev server. Install and use the release package instead.

## Recommended Workflow

Use ding whenever you want Claude Code to work in the background:

```powershell
ding claude
```

Then keep doing your normal work. Watch YouTube, read docs, review code, or continue vibecoding. If Claude Code needs permission, ding will make it visible immediately.

