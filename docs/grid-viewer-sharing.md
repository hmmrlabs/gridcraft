# Hammer Sheets sharing

Updated: 2026-10-19. Revision: M1.10, Hammer Sheets branding and sharing domain.

Share → Share with Hammer Sheets creates an XLSX snapshot locally. Copy Link copies a self-contained `#xlsx=` link; Open Viewer / Shorten Link opens the browser editor. The viewer can optionally create encrypted short links with a 30-day expiration. Desktop sharing itself makes no network request.

The `file.shareGridViewer` query command returns `{url, bytes}` for workbooks up to 48 KiB, or `{requiresUpload: true, bytes, maxBytes}` up to 16 MiB without dialogs, saving the document, or changing undo history. It is available through the engine, CLI and MCP. Menu invocation commits the current cell edit before snapshotting.

All workbook sheets, including hidden sheets, are included. Anyone with the complete link can read them. Self-contained links are encoded, not encrypted, and cannot be revoked. The 48 KiB XLSX limit matches GridLink. Larger workbooks open a protected-sharing explanation, not a broken inline link. When the host supplies `Services::protected_share`, Continue passes a locally encoded XLSX snapshot into its consent/sign-in/password flow. Opening or canceling this dialog invokes no sharing service. Hosts must require explicit consent and submission before upload. The desktop host now implements this flow with a background worker for WorkOS requests, password derivation, encryption, and upload. Its separate dialog state never enters workbook data, preferences, or command parameters. Hosts without that service explain the limitation and offer Open Hammer Sheets. Workbooks over 16 MiB still require download.

Links currently target `https://sheets.hammerlabs.ai/grid` on the Hammer Sheets domain. No third-party shortening service receives links. Preserve original files because unsupported XLSX features may not round-trip.

The browser protected-sharing flow is implemented in the separate private viewer repository but remains undeployed pending WorkOS configuration and end-to-end verification. Current production legacy short links have a 1.5 MiB limit and use a fragment key. The native flow compiles but genuine WorkOS sign-in, authenticated uploads, and recipient interoperability still require end-to-end verification.

File > Home now exposes an optional host-owned **My sheets** library. Sign-in is optional;
creating, opening, editing, downloading, and local saving remain available to guests.
File > Save As > **Save on Hammer Sheets…** passes an XLSX snapshot to the same protected-save
flow regardless of snapshot size. It does not mark the local workbook saved. The library lists
only the signed-in user's encrypted snapshots, with size and expiration; names remain private.
