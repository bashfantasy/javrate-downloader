## 1. MotvCDN adapter 實作

- [x] 1.1 實作 MotvCDN adapter：在 src-tauri/src/cdn_adapter.rs 新增 MotvCdnAdapter 結構體，實作 CdnAdapter trait 的 name()（回傳 "MotvCDN"）和 matches()（判斷 URL 是否包含 `motv.multicdn.top`）方法
- [x] 1.2 實作 MotvCdnAdapter 的 patch_url() 方法：使用正則表達式提取新 URL 的 `key=` 和 `time=` 參數值，替換到舊 URL 上，保留影片路徑不變
- [x] 1.3 實作 MotvCdnAdapter 的 is_expired() 方法：解析 URL 中 `time=` 參數的 Unix 時間戳，與當前時間比較判斷是否過期
- [x] 1.4 實作 MotvCdnAdapter 的 js_extraction_snippet() 方法：回傳 JavaScript 片段，在 WebView 中從 motv.app 頁面結構嘗試主動拼湊 m3u8 URL（檢查 Video.js player.src() 配置、頁面 script 標籤中的影片源設定等）
- [x] 1.5 在 ALL_ADAPTERS 陣列中將 MotvCdnAdapter 註冊到 GenericAdapter 之前（確保優先於通用適配器匹配）
- [x] 1.6 為 MotvCdnAdapter 新增單元測試：matches() 正確匹配/不匹配、patch_url() Token 替換、is_expired() 過期判斷

## 2. 動態萃取播放按鈕觸發增強（Dynamic JavaScript m3u8 extraction）

- [x] 2.1 增強 Dynamic JavaScript m3u8 extraction：在 src-tauri/src/extraction.rs 的 dynamic_capture_script() 函數中擴充播放按鈕 CSS 選擇器清單，新增覆蓋 motv.app 播放器樣式的選擇器（如 Video.js 的 `.video-js .vjs-play-control`、通用的 poster overlay 和 play icon 覆蓋層）
- [x] 2.2 增強主動輪詢 DOM 的 JavaScript 邏輯（第 131~156 行的 tokio::spawn 區塊），在輪詢時也嘗試觸發播放按鈕點擊，而非僅搜尋 DOM 文字中的 m3u8 URL
- [x] 2.3 確認 CDN adapter provides JS extraction snippet 機制正常運作：驗證 MotvCdnAdapter 的 js_extraction_snippet 能在 WebView 中成功執行並透過 report() 回報 m3u8 URL

## 3. Resolution selection dialog 改進

- [x] 3.1 修改 Resolution selection dialog 行為：移除「單筆 URL 自動跳過」邏輯，改為不論萃取到一筆或多筆 m3u8 URL 均彈出選擇對話框
- [x] 3.2 新增「僅複製 URL」按鈕：在對話框中增加 "Copy URL" 按鈕，點擊後將選中的 m3u8 URL 複製到系統剪貼簿並關閉對話框，不啟動下載任務
- [x] 3.3 修正對話框中 URL 文字的 CSS 樣式：對 m3u8 URL 文字設定 `word-break: break-all`，確保超長網址在對話框內自動換行、完整顯示

## 4. 整合測試與驗證

- [x] 4.1 使用 motv.app 測試 URL（https://imagecdn.motv.app/vodplay/1617-1-1/）進行端對端測試：確認 App 可成功萃取 m3u8 URL 並開始下載
- [x] 4.2 驗證 Auto-Relay 接力在 motv.multicdn.top 的 Token 過期場景下正確運作：MotvCdnAdapter 的 patch_url() 能正確替換 key/time 參數
- [x] 4.3 回歸測試：確認既有的 javrate.com（BunnyCDN）下載流程不受影響
- [x] 4.4 對話框測試：驗證單筆 URL 也會彈出對話框、「複製 URL」功能正常將 URL 寫入剪貼簿、超長 URL 正確換行顯示

