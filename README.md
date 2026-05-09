# Javrate Downloader (M3U8 Relay Engine)

目前許多影音網站針對 HLS (m3u8) 串流採用了極端嚴格的防盜鏈機制——播放清單內的 .ts 切片綁定的 Token 壽命極短（約 1~2 分鐘）。傳統單線程下載工具（如 ffmpeg）無法在 Token 過期前抓完所有切片，導致下載中斷並報錯 403 Forbidden。

本專案旨在開發一款 macOS (Apple Silicon 最佳化) 的桌面應用程式，透過自動化網頁解析、多線程併發下載與全自動接力續傳機制，徹底解決超短命 Token 導致的下載失敗問題。

## ✨ 核心特色

* **自動網頁嗅探**：只要輸入影片播放頁面網址，系統會透過隱藏的 Tauri WebView 自動攔截並提取帶有授權 Token 的 m3u8 連結。
* **多線程暴力下載**：底層整合 `yt-dlp`，預設開啟 20 條線程併發下載片段，最大化頻寬利用率。
* **獨家「自動接力 (Auto Relay)」機制**：
  * 即時監聽下載日誌，一旦偵測到 `403 Forbidden`（Token 過期）。
  * 系統會自動暫停、無縫重新載入網頁取得全新 Token。
  * 透過智慧 URL Patch 技術將新 Token 補丁到舊有進度上，實現 0 衝突斷點續傳。
* **原生 macOS 體驗**：使用 Tauri (React + Rust) 打造，體積輕巧，專為 Apple Silicon 最佳化。

## 🚀 系統需求

* **作業系統**：macOS (Apple Silicon M1/M2/M3 推薦)
* **依賴套件**：必須透過 Homebrew 安裝 `yt-dlp` 和 `ffmpeg`

```bash
brew install yt-dlp ffmpeg
```

## 🛠️ 開發與建置

本專案採用 Tauri 框架開發，前端使用 React + TypeScript，後端使用 Rust。

### 安裝依賴
```bash
npm install
```

### 開發模式
```bash
npm run tauri dev
```

### 打包正式版應用程式 (.app)
```bash
npm run tauri build
```
打包完成後的應用程式會產生於 `src-tauri/target/release/bundle/macos/` 目錄下。

## 📝 技術細節 (Spec-Driven)

本專案採用 **Spec-Driven Development (SDD)** 模式開發，所有系統架構與狀態機設計皆記錄於 `openspec/` 目錄中。
針對特定邊界狀況（如 `yt-dlp` 於 Token 過期時可能引發的退出碼誤判），皆有嚴謹的防護與日誌監控機制。
