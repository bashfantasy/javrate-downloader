## Why

使用者提出需要下載第三種類型的影音網址 `https://avjoy.me/video/*`。這將擴展目前的應用程式，讓它除了原本支援的網站之外，還能從 `avjoy.me` 網頁原始碼中萃取出對應的 `.m3u8` 網址並支援多線程接力下載。

## What Changes

- 在 m3u8 萃取邏輯中，新增針對 `avjoy.me` 網域的網頁原始碼解析與 Regex 規則。
- 確保 Auto-Relay 接力續傳引擎能夠處理 `avjoy.me` 的網頁重新整理與 Token 更新。

## Non-Goals (optional)

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `m3u8-extraction`: 擴展現有的解析能力，新增支援從 `avjoy.me` 網頁結構中提取 `.m3u8`。

## Impact

- Affected specs: `m3u8-extraction`
- Affected code:
  - Modified: `src-tauri/src/extraction.rs`
