## Why

目前許多影音網站針對 HLS (m3u8) 串流採用了極端嚴格的防盜鏈機制——播放清單內的 `.ts` 切片綁定的 Token 壽命極短（約 1~2 分鐘）。傳統單線程下載工具（如 `ffmpeg`）無法在 Token 過期前抓完所有切片，導致下載中斷並報錯 `403 Forbidden`。

本專案旨在開發一款 macOS (Apple Silicon 最佳化) 的桌面應用程式，透過自動化網頁解析、多線程併發下載與全自動接力續傳機制，徹底解決超短命 Token 導致的下載失敗問題。

## What Changes

- **新增桌面應用程式**：使用 Tauri (React + TypeScript) 建構原生 macOS 桌面 App
- **任務輸入與 m3u8 自動萃取**：使用者僅需輸入影片網頁 URL，App 自動爬取頁面 HTML 並萃取所有 `.m3u8` 網址，支援多解析度選擇
- **多任務下載管理介面**：支援同時管理多筆下載任務，每筆任務即時顯示精美動畫進度條（百分比、下載速度、預估時間、frag X/Y），包含漸層色彩、脈衝光暈與平滑過渡動畫
- **自訂存檔路徑與檔名**：每個下載任務提供預設存檔路徑與檔名，使用者可在建立任務時修改儲存目錄與自訂檔名，並提供「一鍵貼上」便利按鈕
- **任務操作控制**：每筆任務支援暫停 (Pause)、恢復 (Resume)、取消 (Cancel) 操作
- **全自動無縫接力引擎 (Auto-Relay Engine)**：偵測 yt-dlp 輸出的 HTTP 403 錯誤後，自動等待舊下載程序安全結束（避免檔案衝突），重新爬取原網頁取得新 Token 的 m3u8 URL，無縫重啟 yt-dlp 斷點續傳
- **可擴充 CDN 適配器 (CDN Adapter)**：模組化的適配器架構，支援針對不同 CDN 平台 (如 BunnyCDN) 制定特有的 Token 嫁接與過期檢測邏輯，相容性極高
- **底層引擎**：使用 yt-dlp 作為下載核心，支援多線程暴力拉取（-N 20）與 HTTP Header 偽裝（Referer、Origin、User-Agent）

## Non-Goals

- 不支援 Windows / Linux 平台（僅限 macOS）
- 不內建影片轉檔功能（yt-dlp 會自動合併為 mp4）
- 不提供批次匯入 URL 清單功能（僅支援單一 URL 逐筆加入）
- 不處理需要帳號登入才能觀看的付費影片
- 不內建 yt-dlp 的自動更新機制（使用者需自行安裝與更新）

## Capabilities

### New Capabilities

- `m3u8-extraction`: 從影片網頁 URL 自動萃取 m3u8 播放清單網址，支援靜態 HTML 解析與動態 JavaScript 渲染（透過 Headless 瀏覽器攔截網路請求）兩種模式
- `download-engine`: 基於 yt-dlp 的多線程下載引擎，管理子進程生命週期，解析 stdout 日誌取得即時進度數據，支援暫停（SIGINT）、恢復（重啟進程）、取消操作
- `auto-relay`: 全自動接力續傳模組，監聽 yt-dlp 輸出偵測 403 錯誤，自動重新獲取新 Token 的 m3u8 URL 並無縫重啟下載
- `task-management`: 多任務佇列管理，支援任務新增、狀態追蹤、進度即時更新與操作控制（暫停/恢復/取消）
- `downloader-ui`: macOS 桌面應用 UI，包含任務輸入區（含一鍵貼上功能）、多解析度選擇對話框、任務清單與進度顯示（包含中文 ETA 與 Fragment 標示）
- `cdn-adapter`: CDN 適配器架構，抽象化不同平台的 M3U8 URL 嫁接與過期檢測邏輯

### Modified Capabilities

（無）

## Impact

- Affected specs: 6 個新 capability specs（m3u8-extraction、download-engine、auto-relay、task-management、downloader-ui、cdn-adapter）
- Affected code:
  - New: src/ (React 前端原始碼)、src-tauri/ (Tauri Rust 後端原始碼)、package.json、Cargo.toml、tauri.conf.json
  - Modified: （無，為全新專案）
  - Removed: （無）
