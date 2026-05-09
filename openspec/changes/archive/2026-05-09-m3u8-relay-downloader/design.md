## Context

本專案為全新的 macOS 桌面應用程式，旨在解決影音網站 HLS (m3u8) 串流的極端防盜鏈問題。目標使用者需要下載帶有超短命 Token（約 1~2 分鐘）的 m3u8 串流，傳統工具無法在 Token 過期前完成下載。

目前專案尚無任何程式碼，需從零開始建構。核心依賴 yt-dlp CLI 工具作為底層下載引擎，應用程式負責自動化管理 Token 刷新與斷點續傳。

## Goals / Non-Goals

**Goals:**

- 建構一個輕量的 macOS 原生桌面 App，使用者體驗流暢
- 全自動處理 403 Token 過期問題，使用者無需手動介入
- 即時顯示下載進度，支援多任務並行管理
- 支援靜態 HTML 與動態 JavaScript 渲染兩種 m3u8 萃取模式

**Non-Goals:**

- 不支援 Windows / Linux 跨平台
- 不內建影片轉檔、批次匯入、帳號登入、yt-dlp 自動更新功能
- 不實作自訂的 HLS 下載器（完全依賴 yt-dlp）

## Decisions

### D1: 前端框架選用 Tauri + React + TypeScript

**選擇**：Tauri v2（Rust 後端 + React 前端）

**理由**：
- 相較 Electron，Tauri 打包體積極小（約 5~10 MB vs 100+ MB），且原生使用 macOS WebKit，記憶體佔用低
- Rust 後端可安全管理子進程生命週期（spawn/signal/kill），效能優異
- React + TypeScript 前端開發效率高，生態成熟

**備選方案**：
- Electron：Node.js 生態熟悉度高，但體積與記憶體佔用過大
- Swift 原生：效能最佳但開發週期長，且需額外學習成本

### D2: m3u8 萃取策略採雙模式架構

**選擇**：先嘗試靜態 HTTP GET 解析，失敗時降級至 Headless 瀏覽器攔截

**理由**：
- 多數網站的 m3u8 URL 可從 HTML source 中直接以正則表達式萃取，速度快且資源消耗低
- 少數網站的 m3u8 由 JavaScript 動態生成，此時需透過 Headless 瀏覽器（Playwright 或 Tauri WebView）攔截 Network 請求
- 雙模式架構兼顧速度與相容性

**實作細節**：
- 靜態模式：Rust 端使用 reqwest 發送 HTTP GET，正則搜尋回應 HTML 中的 .m3u8 URL
- 動態模式：利用 Tauri 內建的 WebView 開啟隱藏視窗，攔截所有網路請求並過濾 .m3u8 URL

### D3: yt-dlp 子進程管理策略

**選擇**：Rust 端透過 tokio::process 管理 yt-dlp 子進程

**理由**：
- Rust 的 tokio 非同步運行時可高效管理多個併發子進程
- 可精確控制進程信號（SIGINT 暫停、SIGTERM 取消）
- 透過 stdout pipe 非同步讀取 yt-dlp 輸出並解析進度資訊

**進度解析**：
- 監聽 yt-dlp stdout 每一行輸出
- 正則匹配 `[download] XX.X% of ~XX.XXMB at XX.XXMiB/s ETA XX:XX frag X/Y` 格式
- 解析後透過 Tauri Event 推送至前端更新 UI

### D4: Auto-Relay 接力續傳流程

**選擇**：事件驅動的自動重試迴圈

**流程**：
1. 監聽 yt-dlp stderr/stdout，偵測到 `HTTP Error 403` 字串時觸發接力
2. 中止當前 yt-dlp 進程（等待其自然結束或 SIGTERM）
3. 自動重新請求原始影片網頁 URL，以相同的萃取策略（D2）取得帶新 Token 的 m3u8 URL
4. 驗證新 m3u8 URL 的解析度規格與原任務一致
5. 以原輸出檔路徑重啟 yt-dlp，利用其斷點續傳特性從上次中斷處接續
6. 重試次數上限設為 50 次（每次 403 算一次），超過則標記任務失敗

### D5: 任務狀態機設計

**狀態定義**：

```
Pending → Extracting → Selecting → Downloading → Completed
                                        ↓ ↑
                                     Relaying (403 接力中)
                                        ↓
                                      Paused
                                        ↓
                                     Cancelled
                                        ↓
                                      Failed
```

- `Pending`：任務已建立，等待開始
- `Extracting`：正在萃取 m3u8 URL
- `Selecting`：萃取到多個 m3u8 URL，等待使用者選擇解析度
- `Downloading`：yt-dlp 正在下載中
- `Relaying`：偵測到 403，正在重新取得 Token 並接力
- `Paused`：使用者手動暫停（yt-dlp 已收到 SIGINT）
- `Completed`：下載完成
- `Cancelled`：使用者取消任務
- `Failed`：重試次數超過上限或其他不可恢復錯誤

### D6: CDN 適配器架構 (CDN Adapter)

**選擇**：實作 `CdnAdapter` Trait 來抽象化不同 CDN 平台的特殊處理邏輯。

**理由**：
- 不同影音平台的 CDN (如 BunnyCDN, CloudFront) 的防盜鏈 Token 參數結構與過期時間計算方式差異極大。
- 透過抽象出 `patch_url`、`is_expired`、`js_extraction_snippet` 等方法，核心下載引擎無需關心具體的 Token 拼接邏輯。
- 支援任意數量的適配器，未來新增平台只需實作新的 Adapter 並註冊到 `ALL_ADAPTERS` 中即可。
- 內建 `GenericAdapter` 兜底處理未知平台，提供廣泛相容性。

## Risks / Trade-offs

- **[網站結構變更]** → 不同網站的 HTML 結構差異大，m3u8 萃取正則可能失效。緩解：提供雙模式萃取，動態模式作為萬能後備
- **[yt-dlp 版本相容性]** → App 依賴系統已安裝的 yt-dlp，版本差異可能導致 stdout 格式不同。緩解：啟動時檢查 yt-dlp 版本，進度解析使用寬鬆正則
- **[Token 刷新頻率過高]** → 若網站 Token 壽命極短（< 30 秒），即使多線程也無法在一次 Token 內下載足夠片段，接力次數可能極多。緩解：允許使用者調整線程數（-N 參數），並設定合理的重試上限
- **[WebView 動態萃取的穩定性]** → Headless WebView 載入複雜網頁可能出現超時或渲染問題。緩解：設定 30 秒載入超時，失敗時提示使用者手動貼入 m3u8 URL
