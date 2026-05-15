## Why

目前 javrate-downloader 無法下載來自 motv.app 網站（如 `https://imagecdn.motv.app/vodplay/1617-1-1/`）的影片。原因有二：

1. **動態萃取無法觸發影片播放**：motv.app 的 m3u8 URL 不在 HTML 原始碼中，必須在頁面載入後點擊 "Play Video" 按鈕，影片播放器才會透過 JavaScript 向 CDN 發出 m3u8 請求。目前的 WebView 自動點擊邏輯可能無法正確辨識並觸發 motv.app 的播放按鈕。

2. **缺少 motv.multicdn.top CDN 適配器**：motv.app 使用的 CDN 域名為 `motv.multicdn.top`，其防盜鏈機制使用 `key=` 與 `time=` 參數（例如 `?key=46d184f5...&time=1778873615`），與現有的 BunnyCDN（`bcdn_token`）和 CloudFront（`Policy`）完全不同，導致 Auto-Relay 接力時無法正確拼接新 Token。

## What Changes

- **新增 MotvCDN 適配器**：在 cdn_adapter.rs 中新增 `MotvCdnAdapter`，實作 `CdnAdapter` trait，支援 `motv.multicdn.top` 域名的 `key=` + `time=` Token 匹配、拼接與過期判斷
- **增強動態萃取的播放按鈕觸發能力**：在 WebView 注入腳本中擴充播放按鈕選擇器，確保能觸發 motv.app 類型網站的 Video.js 播放按鈕
- **新增 MotvCDN 的 JS 擷取片段**：在 WebView 注入腳本中加入針對 motv.app 頁面結構的 m3u8 URL 拼湊邏輯
- **改進解析度選擇對話框**：不論萃取到一筆或多筆 m3u8 URL 都彈出選擇視窗，提供「僅複製 URL（不下載）」與「開始下載」兩種操作選項
- **對話框 URL 自動換行**：對話框中的 m3u8 URL 文字採用 `word-break: break-all` 樣式，確保超長網址可完整顯示

## Capabilities

### New Capabilities

（無 — 此變更修改既有 capability 而非新增）

### Modified Capabilities

- `m3u8-extraction`: 增強動態 WebView 萃取的播放按鈕觸發邏輯，覆蓋 motv.app 的播放器樣式；新增 MotvCDN adapter 支援 motv.multicdn.top 域名的 Token 拼接
- `downloader-ui`: 改進解析度選擇對話框 — 強制顯示（不論筆數）、新增「僅複製 URL」操作、URL 自動換行

## Impact

- Affected specs: m3u8-extraction（Modified）、downloader-ui（Modified）
- Affected code:
  - Modified: src-tauri/src/cdn_adapter.rs（新增 MotvCdnAdapter 結構體與註冊）、src-tauri/src/extraction.rs（擴充播放按鈕選擇器）、src/App.tsx 或相關 UI 元件（對話框行為與樣式）
  - New: （無）
  - Removed: （無）
