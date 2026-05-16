## 1. 萃取引擎與 CDN 適配器開發 (Avjoy.me)

- [x] 1.1 在 `src-tauri/src/cdn_adapter.rs` 中實作 `Avjoy.me CDN adapter` 並註冊。該適配器需能匹配 `media-cdn*.avjoy.me` 網域，並根據 URL 中的 Unix timestamp 段落（如 `/1778925984/`）判斷 Token 是否過期。驗證：執行 `cargo test cdn_adapter` 通過相關單元測試。
- [x] 1.2 在 `src-tauri/src/extraction.rs` 的 `dynamic_capture_script` 中，實作 `Avjoy.me MP4 and M3U8 extraction` 邏輯。擴充對 `video` 標籤 `currentSrc` 的掃描，確保在 `.m3u8` 為廣告時能提取指向 `avjoy.me` 的 `.mp4` 主影片位址。驗證：執行 `cargo test extraction` 確保 Regex 與標籤提取正確。
- [x] 1.3 更新 `parse_resolution_label` 函式，使其能辨識 Avjoy MP4 特有的畫質後綴（例如 `_1080p.mp4`）。驗證：手動測試該函式對 `75842_1080p.mp4` 的回傳值為 "1080p"。

## 2. 自動接力 (Auto-Relay) 與驗證

- [x] 2.1 確保 `Avjoy.me CDN adapter` 的 `patch_url` 能夠正確處理 Token 刷新後的網址更新。驗證：在單元測試中模擬過期網址與新網址的替換過程。
- [x] 2.2 整合測試：在開發模式下啟動應用程式，輸入 avjoy.me 影片網址，驗證解析出的清單中包含 1080p 的 MP4 項目，且點擊下載後能順利啟動下載任務。驗證：手動操作 UI 並檢查下載目錄中產出的檔案。
