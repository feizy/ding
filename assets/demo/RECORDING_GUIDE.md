# ding Demo GIF Recording Guide

This guide defines the GIFs used by the GitHub README.

Record real product behavior. Avoid mockups. ding's value is that it catches real Claude Code blocking states.

## Recommended Tool

Use ScreenToGif on Windows.

Recommended capture settings:

- FPS: 12-15
- Width: 760-900 px
- Duration: 6-12 seconds per GIF
- Crop tightly around the terminal and ding floating window
- Hide private paths, tokens, and unrelated windows
- Keep final GIFs under 10 MB when possible

If you record MP4 files instead, use `scripts/convert-demo-video.ps1` to convert them to GIFs with ffmpeg.

## Asset Names

Place final GIFs here:

```text
assets/demo/permission-required.gif
assets/demo/ask-user-question.gif
assets/demo/multi-instance-monitoring.gif
```

After adding the files, uncomment the GIF image block in `README.md`.

## Demo 1: Permission Required

Goal: show that ding catches a Claude Code permission prompt.

Setup:

```powershell
ding claude --permission-mode default
```

Prompt:

```text
Run a command that creates a small test file, then stop.
```

Expected Claude behavior:

- Claude Code asks for permission to run a `Bash` command.
- ding changes to `Action needed`.
- ding shows the command and permission choices.
- Click `Allow` in ding.
- Claude Code continues.

Capture:

- Start with the Claude terminal visible and ding floating above it.
- Trigger the prompt.
- Show ding entering `Action needed`.
- Click `Allow`.
- End after Claude continues.

## Demo 2: AskUserQuestion Mirrored

Goal: show that ding mirrors Claude Code's real user-question choices instead of generic Allow/Deny buttons.

Prompt:

```text
Ask me which hook scenario I want to test. Give me three choices: permission command interception, option selection capture, and tool call monitoring.
```

Expected behavior:

- Claude Code uses `AskUserQuestion`.
- ding shows the original choices.
- Select one option in ding.
- Submit it.
- Claude Code continues with that answer.

Capture:

- Zoom/crop so the ding floating window is readable.
- Make sure the GIF clearly shows the actual option labels.

## Demo 3: Multi-Instance Monitoring

Goal: show that ding can monitor multiple Claude Code sessions at the same time.

Setup:

- Open two or three terminals in the same repo.
- Start Claude Code through ding in each terminal.
- Keep the ding floating window visible and expanded.

Terminal 1 prompt:

```text
Use Bash to inspect the current git status, then stop.
```

Terminal 2 prompt:

```text
Ask me which hook scenario I want to test. Give me three choices: permission command interception, option selection capture, and tool call monitoring.
```

Optional Terminal 3 prompt:

```text
Read package.json and summarize the scripts.
```

Capture:

- Show the ding floating window with multiple instance dots.
- Expand ding to show multiple cards.
- Capture at least one `Using Bash` or `Using AskUserQuestion` state.
- Capture one instance entering `Action needed` while another remains running or idle.
- End with ding showing that multiple sessions are tracked together.

Do not show private paths, tokens, or personal content.
