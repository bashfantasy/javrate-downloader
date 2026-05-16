# M1 Mac 極端防盜鏈 HLS/m3u8 接力下載器 - 核心技術與需求規格文件

## 1. 專案背景與痛點
目前許多影音網站針對 HLS (m3u8) 串流採用了極端嚴格的防盜鏈機制：
- **超短命 Token**：播放清單內的 `.ts` 切片綁定的 Token 壽命極短（約 1~2 分鐘）。
- **常規工具失效**：傳統單線程下載工具（如 `ffmpeg`）無法在 Token 過期前抓完所有切片，導致下載中斷並報錯 `403 Forbidden`。

本專案旨在開發一款 macOS (Apple Silicon 最佳化) 的桌面應用程式，透過自動化抓取與 `yt-dlp` 的斷點續傳機制，徹底解決上述痛點。

## 2. 核心破解技術 (接力續傳大法)
本專案的核心建立在多線程併發與斷點續傳特性上。

### 底層引擎與指令
使用編譯給 macOS 的 `yt-dlp`，核心指令架構如下：
```bash
yt-dlp -N 20 \
-o "output.mp4" \
--add-header "Referer: [PAGE_URL]" \
--add-header "Origin: [DOMAIN]" \
--add-header "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.2 Safari/605.1.15" \
"[M3U8_URL_WITH_TOKEN]"
```
- **多線程暴力拉取 (`-N 20`)**：最大化利用頻寬，搶在死線前下載最多片段。
- **斷點接力續傳**：利用 `yt-dlp` 遇到 403 中斷後，暫存檔會保留的特性。替換新的滿血 Token 網址再次執行，即可無縫續傳。

## 3. 系統功能與 UI 需求 (System Requirements)

### 3.1 任務輸入與解析模組
- **單一網址輸入**：使用者只需輸入影片的原始網頁網址 (例如：`https://www.javrate.com/Movie/Detail/da045d78-...`)。
- **自動萃取 m3u8**：App 在背景爬取該網頁的 Source HTML，找出所有 `.m3u8` 網址。
- **多重解析選擇**：若解析出多個 m3u8 網址（例如不同解析度 720p, 1080p），需跳出選單供使用者選擇要下載哪一個。

### 3.2 下載任務清單與控制介面 (UI)
- **多任務清單**：支援隨時加入新任務，介面需條列顯示所有下載任務。
- **即時進度顯示**：解析 `yt-dlp` 的 stdout 日誌，在介面上即時更新進度條（百分比、下載速度、預估時間與 frag X/Y）。
- **任務操作控制**：每一筆任務皆需具備 **暫停 (Pause)**、**恢復 (Resume)**、**取消 (Cancel)** 三大功能。

### 3.3 全自動無縫接力模組 (Auto-Relay Engine)
這是本 App 擺脫人工操作的核心亮點，完全自動化處理 403 斷線問題：
1. **中斷偵測**：監聽 `yt-dlp` 輸出，一旦偵測到 `HTTP Error 403: Forbidden`，立即中止該次進程。
2. **自動重新獲取 Token**：
   - App 自動在背景**重新請求**最初輸入的「影片網頁網址」。
   - 再次萃取出帶有最新 Token 的 `.m3u8` 網址。
   - 確保新萃取的 m3u8 畫質規格與原任務相符。
3. **無縫重啟**：將全新的 m3u8 網址帶入 `yt-dlp` 指令，針對原暫存檔執行續傳，直到 100% 完檔，全程使用者無感。

## 4. 技術架構選型建議
基於以上需求，建議的技術堆疊如下：

### 前端介面層 (Frontend UI)
- **Tauri (React + TypeScript)**：
  - 體積極小，跨平台能力強，且與原生 macOS 整合度極高（內部使用 WKOnyx/WebKit）。
  - 使用 React 刻畫任務清單、進度條與操作按鈕非常快速。
- 或 **Electron**：如果對 Node.js 生態最熟悉，這會是最快產出的方案。

### 核心邏輯層 (Backend Logic)
- 負責管理下載任務隊列 (Task Queue) 的狀態。
- 建立子進程 (`Child Process`) 呼叫 `yt-dlp` 並透過 `stdout` 監聽。
- **暫停邏輯**：發送 SIGINT (`Ctrl+C`) 信號給該 `yt-dlp` 進程。
- **恢復邏輯**：帶著原參數重啟進程。
- **背景爬蟲模組**：負責發送 HTTP GET 請求解析 HTML。

## 5. 開發實作難點預警 (Pitfalls)
- **動態渲染的 m3u8**：如果該網站的 m3u8 網址是由 JavaScript 動態產生（也就是直接對 URL 發送 GET 請求拿到的 Source HTML 裡面沒有 `.m3u8` 字串），爬蟲模組就不能只用簡單的 HTTP GET。此時必須利用 Headless 瀏覽器（如 Playwright 或 Tauri 內建的 WebView）來攔截網路請求 (Network Interception) 才能精準拿到包含 Token 的 m3u8。
