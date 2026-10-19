# Grid Viewer sharing

Updated: 2026-10-19. Revision: M1.3, initial fork integration.

Share → Share with Grid Viewer creates an XLSX snapshot locally. Copy Link copies a self-contained `#xlsx=` link; Open Viewer / Shorten Link opens the browser editor. The viewer can optionally create encrypted short links with a 30-day expiration. Desktop sharing itself makes no network request.

The `file.shareGridViewer` query command returns `{url, bytes}` without dialogs, saving the document, or changing undo history. It is available through the engine, CLI and MCP. Menu invocation commits the current cell edit before snapshotting.

All workbook sheets, including hidden sheets, are included. Anyone with the complete link can read them. Self-contained links are encoded, not encrypted, and cannot be revoked. The 48 KiB XLSX limit matches GridLink; larger workbooks report an error and can be downloaded and opened directly in the viewer for encrypted short-link creation (up to 1.5 MiB).

Links currently target `https://grid-viewer.pages.dev/grid` while the custom domain awaits DNS activation. No third-party shortening service receives links. Preserve original files because unsupported XLSX features may not round-trip.
