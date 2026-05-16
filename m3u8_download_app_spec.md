# Javrate Downloader (接力下載器) - 核心技術與需求規格文件

## 1. 專案背景與痛點
目前許多成人或影音網站（如 Javrate、Motv、Avjoy 等）採用了極端嚴格的防盜鏈機制：
- **超短命 Token**：播放清單內的 `.ts` 切片或主影片綁定的 Token 壽命極短（約 1~2 分鐘）。
- **常規工具失效**：傳統單線程下載工具（如 `ffmpeg`）無法在 Token 過期前抓完所有切片，導致下載中斷並報錯 `403 Forbidden`。
- **複雜的動態載入與廣告**：真實影片網址常被埋藏在混淆的 JS 或是大量的假廣告/彈出式廣告 (Popunder) 之中。

本專案旨在開發一款基於 Tauri (Apple Silicon 最佳化) 的桌面應用程式，透過自動化抓取與 `yt-dlp` 的斷點續傳機制，徹底解決上述痛點。

## 2. 核心破解技術 (接力續傳大法)

### 2.1 底層引擎與指令
使用內建的 `yt-dlp`，核心指令架構如下：
```bash
yt-dlp -N 20 \
-o "output.mp4" \
--add-header "Referer: [PAGE_URL]" \
--add-header "Origin: [DOMAIN]" \
--add-header "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.2 Safari/605.1.15" \
"[MEDIA_URL_WITH_TOKEN]"
```
- **多線程暴力拉取 (`-N 20`)**：最大化利用頻寬，搶在死線前下載最多片段。
- **斷點接力續傳**：利用 `yt-dlp` 遇到 403 中斷後，暫存檔會保留的特性。當遇 403 時，程式會自動重新萃取新的 Token 網址再次執行，達成無縫續傳。
- **標頭偽裝 (Referer/Origin)**：許多網站（如 Avjoy）若未帶入正確的 `Referer`，會直接回傳 5 秒鐘的反盜鏈假影片。

### 2.2 動態 WebView 解析與反廣告機制
為了對抗動態生成的 URL 與惡意廣告，系統採用隱藏的 **Tauri WebView** 載入目標網頁，並注入腳本擷取真實資源：
1. **防彈出廣告 (Popunder Protection)**：
   - 腳本**絕不主動點擊**畫面上的播放按鈕 UI，因為這會觸發暗藏的廣告分頁，導致下載器錯誤捕捉到色情直播（如 Stripchat）的 `.m3u8` 串流。
   - 改為在背景透過 JS 調用 `video.play()` 來促使真實網址載入。
2. **CDN 廣告黑名單過濾**：
   - 建立嚴格的 `AD_CDNS` 黑名單（如 `growcdnssedge.com`, `saawsedge.com` 等），在前端腳本與 Rust 後端雙重過濾，確保最終獲取的絕對是主影片網址。
3. **混合資源支援**：
   - 同時支援萃取標準 `.m3u8` 播放清單與直接的 `.mp4` 資源（例如 Avjoy 的 `media-cdn*.avjoy.me` 格式）。

## 3. 系統功能與 UI 需求 (System Requirements)

### 3.1 任務輸入與解析模組
- **單一網址輸入**：使用者只需輸入影片的原始網頁網址。
- **多重解析選擇**：若解析出多個資源網址（例如不同解析度 720p, 1080p），跳出視窗供使用者選擇要下載哪一個。

### 3.2 下載任務清單與控制介面 (UI)
- **多任務清單**：支援隨時加入新任務，介面需條列顯示所有下載任務。
- **即時進度顯示**：解析 `yt-dlp` 的 stdout 日誌，即時更新進度條（百分比、下載速度、預估時間）。
- **任務操作控制**：每一筆任務皆需具備 **暫停 (Pause)**、**恢復 (Resume)**、**取消 (Cancel)** 功能。

### 3.3 全自動無縫接力模組 (Auto-Relay Engine)
1. **中斷偵測**：監聽 `yt-dlp` 輸出，一旦偵測到 `HTTP Error 403: Forbidden`，立即中止該次進程。
2. **自動重新獲取 Token**：透過背景 WebView 再次載入原網頁，取得帶有新 Token 的相同規格網址。
3. **無縫重啟**：將全新網址帶入 `yt-dlp` 針對原暫存檔續傳，直到 100% 完檔，全程使用者無感。

## 4. 技術架構選型
- **前端介面層 (Frontend UI)**：React + TypeScript + Vite
- **核心框架 (Backend Framework)**：Tauri (Rust)
- **下載引擎**：`yt-dlp` (作為外部執行檔綁定)
- **資源擷取器**：Rust `reqwest` (靜態解析) + Tauri `WebViewWindow` (動態解析與 JS 注入)
