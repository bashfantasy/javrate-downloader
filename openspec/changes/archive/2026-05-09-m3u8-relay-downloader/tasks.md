## 1. 專案初始化與環境建置

- [x] 1.1 使用 Tauri v2 + React + TypeScript 初始化專案結構（D1: 前端框架選用 Tauri + React + TypeScript），配置 package.json 與 Cargo.toml
- [x] 1.2 配置 tauri.conf.json，設定應用程式名稱、視窗尺寸與 macOS 權限（網路存取、檔案系統存取）
- [x] 1.3 建立前後端目錄架構：src/ 放置 React 元件、src-tauri/src/ 放置 Rust 後端邏輯，按模組分資料夾

## 2. m3u8 萃取模組（m3u8-extraction）

- [x] 2.1 實作 static HTML m3u8 extraction：使用 reqwest 發送 HTTP GET 請求，以正則表達式從回應 HTML 中萃取所有 .m3u8 URL，包含 Referer 與 User-Agent 標頭設定
- [x] 2.2 實作 dynamic JavaScript m3u8 extraction：利用 Tauri 內建 WebView 開啟隱藏視窗（D2: m3u8 萃取策略採雙模式架構），攔截網路請求擷取 .m3u8 URL，設定 30 秒載入超時
- [x] 2.3 實作 resolution label parsing：從 m3u8 URL 路徑段或查詢參數解析解析度標籤（720p、1080p 等），無法辨識時標記為 "Unknown resolution"
- [x] 2.4 實作 token refresh extraction：支援重新萃取 m3u8 URL 以取得新 Token，使用與首次萃取相同的策略（靜態或動態模式）

## 3. 下載引擎模組（download-engine）

- [x] 3.1 實作 yt-dlp subprocess spawning：透過 tokio::process 產生 yt-dlp 子進程（D3: yt-dlp 子進程管理策略），配置 -N 20 多線程、使用任務指定的存檔路徑與檔名作為輸出路徑、HTTP Header（Referer/Origin/User-Agent），並 pipe stdout/stderr
- [x] 3.2 實作 real-time progress parsing：逐行解析 yt-dlp stdout 輸出，使用正則匹配提取下載百分比、速度、ETA、frag X/Y，透過 Tauri Event 推送至前端
- [x] 3.3 實作 HTTP 403 error detection：監聽 yt-dlp 輸出中的 `HTTP Error 403`、`403 Forbidden`、`HTTP error 403` 字串，偵測到時發出 relay-needed 事件
- [x] 3.4 實作 process pause via SIGINT：發送 SIGINT 信號給 yt-dlp 子進程以優雅暫停，等待進程退出
- [x] 3.5 實作 process resume by restart：以相同參數和輸出路徑重新產生 yt-dlp 子進程，利用斷點續傳接續下載
- [x] 3.6 實作 process cancellation via SIGTERM：發送 SIGTERM 信號終止 yt-dlp 子進程
- [x] 3.7 實作 download completion detection：偵測 yt-dlp 以 exit code 0 退出且進度達 100% 時，標記任務完成
- [x] 3.8 實作 yt-dlp not found 錯誤處理：啟動時檢查 yt-dlp 是否在系統 PATH 上可用，不可用時報錯

## 4. 自動接力模組（auto-relay）

- [x] 4.1 實作 automatic 403 relay trigger：接收 download-engine 的 relay-needed 事件，自動將任務狀態轉為 Relaying（D4: Auto-Relay 接力續傳流程）
- [x] 4.2 實作 token refresh via re-extraction：呼叫 m3u8-extraction 模組重新取得帶新 Token 的 m3u8 URL，選擇與原任務相同解析度的 URL
- [x] 4.3 實作 seamless download restart：以新 m3u8 URL 和原輸出路徑重啟 yt-dlp 進程，確認斷點續傳正常運作
- [x] 4.4 實作 retry limit enforcement：記錄每個任務的接力次數，超過 50 次上限時將任務標記為 Failed
- [x] 4.5 實作 relay status reporting：在接力過程中向前端發送狀態事件，包含接力次數和目前階段（重新萃取 URL / 重啟下載）
- [x] 4.6 處理 resolution mismatch during refresh：重新萃取的 m3u8 URL 不包含原始解析度時，選擇最接近的解析度並記錄警告

## 5. 任務管理模組（task-management）

- [x] 5.1 實作 task creation from URL：使用者提交 URL 時建立新任務，分配唯一 ID，初始化為 Pending 狀態，儲存使用者指定的存檔目錄與檔名（或使用預設值）
- [x] 5.2 實作 task state machine：建立完整的任務狀態機（D5: 任務狀態機設計），包含所有合法狀態轉換，拒絕無效轉換並記錄警告
- [x] 5.3 實作 task progress tracking：維護每個任務的即時進度欄位（百分比、速度、ETA、fragment 進度、接力次數），接收 download-engine 的進度事件更新
- [x] 5.4 實作 multiple concurrent tasks 支援：確保多個任務可獨立併發運行，各自擁有獨立的 yt-dlp 子進程和狀態
- [x] 5.5 實作 task persistence across app restart：將任務清單序列化至磁碟，App 重啟時還原非完成任務，Downloading/Relaying 狀態的任務還原為 Paused

## 6. 前端 UI 介面（downloader-ui）

- [x] 6.1 實作 application window layout：建立單頁式版面配置（固定頂部輸入區含存檔路徑設定 + 可捲動任務清單），採用現代深色主題設計
- [x] 6.2 實作 URL input area：包含 URL 文字輸入欄位、存檔目錄欄位（預填預設路徑）、檔名欄位（預填自動衍生名稱）與「開始下載」按鈕，驗證 URL 格式與目錄存在性，提交後清空輸入欄位
- [x] 6.3 實作 custom save path per task：整合 macOS 原生資料夾選擇對話框（透過 Tauri dialog API），讓使用者可點擊資料夾圖示選擇存檔目錄，並支援直接編輯路徑文字
- [x] 6.4 實作 resolution selection dialog：當萃取到多個 m3u8 URL 時顯示模態對話框，列出所有可用解析度供選擇；僅有單一 URL 時自動跳過
- [x] 6.5 實作 task list display：顯示所有下載任務，每筆包含 URL、存檔路徑、色彩編碼的狀態標籤、進度條、下載速度、ETA、fragment 進度、接力次數
- [x] 6.6 實作 task control buttons：根據任務狀態動態顯示對應操作按鈕（Downloading→暫停/取消、Paused→恢復/取消、Extracting/Relaying→取消、Completed/Cancelled/Failed→無按鈕）
- [x] 6.7 實作 real-time progress bar：監聽後端進度事件，即時更新進度條，包含漸層色彩填充（藍→綠）、前緣脈衝光暈效果、平滑 CSS 過渡動畫（≥300ms ease-out）、百分比數字標籤，Relaying 狀態時光暈變橘色
- [x] 6.8 建立 Tauri Command 橋接層：定義前後端通訊的 Tauri Command（建立任務含存檔路徑/檔名、暫停、恢復、取消、資料夾選擇對話框）與 Event（進度更新、狀態變更）

## 7. 整合測試與驗證

- [x] 7.1 端對端測試：從 URL 輸入（含自訂存檔路徑與檔名）到下載完成的完整流程驗證
- [x] 7.2 接力續傳測試：模擬 403 中斷場景，驗證自動接力與斷點續傳正確運作
- [x] 7.3 多任務並行測試：同時執行多個下載任務，驗證互不干擾
- [x] 7.4 暫停/恢復測試：驗證暫停後恢復能正確續傳
- [x] 7.5 邊界條件測試：無效 URL、yt-dlp 未安裝、網路中斷、無效存檔目錄等異常情境
- [x] 7.6 自訂存檔路徑測試：驗證預設路徑、資料夾選擇器、自訂檔名、目錄不存在時的錯誤提示等場景
