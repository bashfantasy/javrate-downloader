use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{webview::WebviewWindowBuilder, AppHandle, WebviewUrl};
use tokio::sync::mpsc;
use tokio::time::{sleep_until, Instant};
use url::Url;

pub const SAFARI_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15";
pub const UNKNOWN_RESOLUTION: &str = "Unknown resolution";

const DYNAMIC_CAPTURE_HOST: &str = "m3u8-capture.internal";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExtractionStrategy {
    Static,
    Dynamic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct M3u8Option {
    pub url: String,
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub strategy: ExtractionStrategy,
    pub options: Vec<M3u8Option>,
}

pub async fn extract_from_page(app: &AppHandle, page_url: &str) -> Result<ExtractionResult> {
    // NOTE: 靜態萃取失敗（如 HTTP 403）屬於正常情況，應優雅降級到動態 WebView 萃取
    let static_options = extract_static(page_url).await.unwrap_or_default();
    if !static_options.is_empty() {
        return Ok(ExtractionResult {
            strategy: ExtractionStrategy::Static,
            options: static_options,
        });
    }

    let dynamic_options = extract_dynamic_webview(app, page_url).await?;
    if dynamic_options.is_empty() {
        return Err(anyhow!(
            "No m3u8 URL was found within 30 seconds. Provide an m3u8 URL manually."
        ));
    }

    Ok(ExtractionResult {
        strategy: ExtractionStrategy::Dynamic,
        options: dynamic_options,
    })
}

pub async fn extract_static(page_url: &str) -> Result<Vec<M3u8Option>> {
    let origin = origin_from_url(page_url)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(SAFARI_USER_AGENT)
        .build()
        .context("failed to build HTTP client")?;

    let html = client
        .get(page_url)
        .header(reqwest::header::REFERER, origin)
        .send()
        .await
        .context("failed to request page")?
        .error_for_status()
        .context("page request returned an error status")?
        .text()
        .await
        .context("failed to read page HTML")?;

    Ok(extract_m3u8_urls_from_html(&html)
        .into_iter()
        .map(|url| M3u8Option {
            resolution: parse_resolution_label(&url),
            url,
        })
        .collect())
}

pub async fn extract_dynamic_webview(app: &AppHandle, page_url: &str) -> Result<Vec<M3u8Option>> {
    let target = Url::parse(page_url).context("invalid page URL")?;
    let found_urls = Arc::new(Mutex::new(BTreeSet::new()));
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();
    let label = format!("dynamic-m3u8-{}", uuid::Uuid::new_v4());

    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(target))
        .title("m3u8 extraction")
        .visible(false)
        .user_agent(SAFARI_USER_AGENT)
        .initialization_script(&dynamic_capture_script())
        .on_web_resource_request({
            let found_urls = Arc::clone(&found_urls);
            let tx = tx.clone();
            move |request, _response| {
                let uri = request.uri().to_string();
                if uri.contains(".m3u8") || uri.contains(DYNAMIC_CAPTURE_HOST) {
                    println!("🕸️ [WebResourceRequest] 攔截到 M3U8: {}", uri);
                }
                // NOTE: 同時明確攔截 avjoy MP4 資源請求
                if uri.contains(".avjoy.") && uri.contains(".mp4") {
                    println!("🎬 [WebResourceRequest] 攔截到 Avjoy MP4: {}", uri);
                }
                record_m3u8_candidate(&uri, &found_urls, &tx);
            }
        })
        .on_navigation({
            let found_urls = Arc::clone(&found_urls);
            let tx = tx.clone();
            move |url| {
                println!("🧭 [Navigation] 嘗試跳轉到: {}", url.as_str());
                if (url.scheme() == "http" || url.scheme() == "https")
                    && url.host_str() == Some(DYNAMIC_CAPTURE_HOST)
                {
                    if let Some((_, captured_url)) = url.query_pairs().find(|(key, _)| key == "url")
                    {
                        println!("🎯 [Navigation] 成功捕獲透過跳轉傳遞的 m3u8: {}", captured_url);
                        record_m3u8_candidate(&captured_url, &found_urls, &tx);
                    }
                    return false;
                }
                matches!(url.scheme(), "http" | "https")
            }
        })
        .build()
        .context("failed to create hidden WebView for dynamic extraction")?;

    // 主動輪詢 DOM、iframes 與 video 標籤（同時支援 m3u8 與 avjoy MP4）
    let window_clone = window.clone();
    tokio::spawn(async move {
        for _ in 0..15 { // 30 秒內最多輪詢 15 次
            tokio::time::sleep(Duration::from_secs(2)).await;
            let script = r#"
                (function() {
                    try {
                        const AD_CDNS = ['growcdnssedge.com', 'nexusriftcore4.cyou', 'saawsedge.com', 'bkcdn.net'];
                        const isAd = (u) => AD_CDNS.some(p => u.includes(p));
                        const report = (url) => {
                            if (url) window.location.href = 'http://m3u8-capture.internal/?url=' + encodeURIComponent(url);
                        };
                        // 掃描 HTML 中的 m3u8
                        let text = document.documentElement.outerHTML;
                        let m3u8s = text.match(/https?:\/\/[^\s"'<>\\]+?\.m3u8(?:[^\s"'<>\\]*)?/gi);
                        if (m3u8s) m3u8s.forEach(u => {
                            if (!isAd(u)) report(u);
                        });
                        // 掃描 video 標籤的 currentSrc（主要是 avjoy MP4）
                        document.querySelectorAll('video').forEach(v => {
                            const src = v.currentSrc || v.src || '';
                            if (src && src.includes('.mp4') && !isAd(src)) report(src);
                            v.querySelectorAll('source').forEach(s => {
                                if (s.src && s.src.includes('.mp4') && !isAd(s.src)) report(s.src);
                            });
                        });
                        // NOTE: 優先點擊 Skip Ad 按鈕（avjoy 使用 .vast-skip-button.enabled）
                        // 讓廣告儘快結束，縮短主影片 currentSrc 載入等待時間
                        document.querySelectorAll('.vast-skip-button.enabled, [class*="skip-ad" i], [id*="skip-ad" i], button.skip, .skipButton').forEach(el => {
                            if (el.offsetParent !== null && typeof el.click === 'function') el.click();
                        });
                        // NOTE: 絕對不可主動點擊播放按鈕 (div[class*="play"] 等 UI 元素)
                        // 因為成人網站會在畫面上覆蓋 Popunder 廣告（例如 Stripchat），點擊會開啟新分頁並載入額外的 M3U8 串流，導致下載到錯誤的影片。
                    } catch(e) {}
                })();
            "#;
            let _ = window_clone.eval(script);
        }
    });

    wait_for_dynamic_captures(&mut rx).await;
    let _ = window.destroy();

    let urls = found_urls
        .lock()
        .expect("dynamic URL mutex poisoned")
        .clone();
    Ok(options_from_urls(urls.into_iter()))
}

pub fn extract_m3u8_urls_from_html(html: &str) -> Vec<String> {
    const AD_CDNS: &[&str] = &["growcdnssedge.com", "nexusriftcore4.cyou", "saawsedge.com", "bkcdn.net"];
    let re = Regex::new(r#"https?://[^\s"'<>\\]+?\.m3u8(?:\?[^\s"'<>\\]*)?"#).expect("valid regex");
    re.find_iter(html)
        .map(|m| {
            m.as_str()
                .trim_end_matches(['.', ',', ';'])
                .replace("&amp;", "&")
        })
        .filter(|url| !AD_CDNS.iter().any(|cdn| url.contains(cdn)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn dynamic_capture_script() -> String {
    let cdn_snippets = crate::cdn_adapter::all_js_extraction_snippets();
    let template = r#"
(() => {
  if (window.__JAVRATE_M3U8_CAPTURE_INSTALLED__) return;
  window.__JAVRATE_M3U8_CAPTURE_INSTALLED__ = true;
  const seen = new Set();
  const pattern = /\.m3u8(?:[?#][^\s"'<>\\]*)?/i;
  const AD_CDN_PATTERNS = ['growcdnssedge.com', 'nexusriftcore4.cyou', 'saawsedge.com', 'bkcdn.net'];
  const isAdUrl = (url) => AD_CDN_PATTERNS.some(p => url.includes(p));

  const report = (value) => {
    try {
      if (!value) return;
      const strValue = String(value);
      // NOTE: 同時支援 .m3u8 與來自 Avjoy 的直接 .mp4 影片網址
      const isM3u8 = /\.m3u8(?:[?#][^\s"'<>\\]*)?/i.test(strValue);
      const isMp4 = /\.mp4(?:[?#][^\s"'<>\\]*)?/i.test(strValue);
      if (!isM3u8 && !isMp4) return;
      if (isAdUrl(strValue)) return; // 過濾廣告

      const absoluteUrl = new URL(strValue, document.baseURI).href;
      if (seen.has(absoluteUrl)) return;
      seen.add(absoluteUrl);
      window.location.href = "http://m3u8-capture.internal/?url=" + encodeURIComponent(absoluteUrl);
    } catch (_) {}
  };
  const originalFetch = window.fetch;
  if (typeof originalFetch === "function") {
    window.fetch = function(input, init) {
      report(typeof input === "string" ? input : input && input.url);
      return originalFetch.apply(this, arguments).then((response) => {
        report(response && response.url);
        return response;
      });
    };
  }
  const originalOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function(method, url) {
    report(url);
    return originalOpen.apply(this, arguments);
  };
  const scanResources = () => {
    try {
      const scanWindow = (win) => {
        try {
          win.performance.getEntriesByType("resource").forEach((entry) => report(entry.name));
          win.document.querySelectorAll("a, video, source").forEach((node) => report(node.href || node.src));
          
          const html = win.document.documentElement.outerHTML;
          
          // 動態注入所有已註冊 CDN 適配器的 JS 擷取邏輯
          CDN_EXTRACTION_SNIPPETS_PLACEHOLDER

          const matches = html.match(/https?:\/\/[^\s"'<>\\]+?\.m3u8(?:[^\s"'<>\\]*)?/gi);
          if (matches) {
            matches.forEach(m => {
                report(m);
                // 嘗試解析母清單找出子清單
                if (!seen.has(m + "_parsed")) {
                    seen.add(m + "_parsed");
                    fetch(m).then(res => res.text()).then(text => {
                        const lines = text.split('\n');
                        lines.forEach(line => {
                            line = line.trim();
                            if (line && !line.startsWith('#') && line.includes('.m3u8')) {
                                try {
                                    const base = new URL(m);
                                    const subUrl = new URL(line, m);
                                    subUrl.search = base.search; // 保留原始的 token 等 query 參數
                                    report(subUrl.href);
                                } catch(e) {}
                            }
                        });
                    }).catch(() => {});
                }
            });
          }

          win.document.querySelectorAll("video").forEach(v => {
            const src = v.currentSrc || v.src || '';
            if (src && src.includes('.mp4')) report(src);
          });

          win.document.querySelectorAll("video").forEach((v) => {
            v.muted = true;
            const _ = v.play();
          });
          // NOTE: 優先點擊 Skip Ad 按鈕，讓廣告提早結束，加速主影片 currentSrc 載入
          win.document.querySelectorAll('.vast-skip-button.enabled, [class*="skip-ad" i], [id*="skip-ad" i], button.skip, .skipButton').forEach(el => {
            if (el.offsetParent !== null && typeof el.click === 'function') el.click();
          });
          // NOTE: 不點擊任何網頁上的播放按鈕 UI，因為成人網站會在上面覆蓋透明的彈出式廣告 (Popunder)
          // 這些點擊會導致開啟新的分頁（如 Stripchat 等直播網站），進而讓我們抓到錯誤的 M3U8 串流。

          Array.from(win.document.querySelectorAll("iframe")).forEach(f => {
            if (f.contentWindow) scanWindow(f.contentWindow);
          });
        } catch (e) {} // 跨域 iframe 會在此拋出錯誤
      };
      scanWindow(window);
    } catch (_) {}
  };
  setInterval(scanResources, 250);
  document.addEventListener("DOMContentLoaded", () => {
      // 強制重載 iframe 以破壞快取
      document.querySelectorAll("iframe").forEach(iframe => {
          try {
              let src = iframe.src;
              if (src && !src.includes("_t=")) {
                  let buster = "_t=" + Date.now();
                  iframe.src = src.includes("?") ? src + "&" + buster : src + "?" + buster;
              }
          } catch(e) {}
      });
      scanResources();
  });
  scanResources();
})();
"#;
    template.replace("CDN_EXTRACTION_SNIPPETS_PLACEHOLDER", &cdn_snippets)
}

fn record_m3u8_candidate(
    candidate: &str,
    found_urls: &Arc<Mutex<BTreeSet<String>>>,
    tx: &mpsc::UnboundedSender<()>,
) {
    // NOTE: 收集 .m3u8 網址
    let mut urls = extract_m3u8_urls_from_html(candidate);
    // NOTE: 同時收集 Avjoy 的直接 MP4 網址（僅限非廣告 CDN）
    urls.extend(extract_avjoy_mp4_urls(candidate));

    if urls.is_empty() {
        return;
    }

    let mut found = found_urls.lock().expect("dynamic URL mutex poisoned");
    let mut inserted = false;
    for url in urls {
        inserted |= found.insert(url);
    }
    drop(found);

    if inserted {
        let _ = tx.send(());
    }
}

/// 從文字中擷取屬於 Avjoy 主影片 CDN 的 MP4 網址，並過濾廣告 CDN。
/// Avjoy 主影片域名格式：media-cdn*.avjoy.me
fn extract_avjoy_mp4_urls(text: &str) -> Vec<String> {
    const AD_CDNS: &[&str] = &["growcdnssedge.com", "nexusriftcore4.cyou", "saawsedge.com", "bkcdn.net"];
    let re = Regex::new(r#"https?://[^\s"'<>\\]+?\.mp4(?:\?[^\s"'<>\\]*)?"|https?://[^\s"'<>\\]+?\.mp4"#)
        .expect("valid regex");
    re.find_iter(text)
        .map(|m| m.as_str().trim_matches('"').trim_end_matches(['.', ',', ';']).to_string())
        .filter(|url| {
            // 必須屬於 avjoy CDN，且不是已知廣告節點
            url.contains(".avjoy.") && !AD_CDNS.iter().any(|cdn| url.contains(cdn))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

async fn wait_for_dynamic_captures(rx: &mut mpsc::UnboundedReceiver<()>) {
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut quiet_until: Option<Instant> = None;

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        if let Some(quiet) = quiet_until {
            if now >= quiet {
                break;
            }
        }

        let next_wake = quiet_until.unwrap_or(deadline).min(deadline);
        tokio::select! {
            message = rx.recv() => {
                if message.is_none() {
                    break;
                }
                quiet_until = Some(Instant::now() + Duration::from_secs(5));
            }
            _ = sleep_until(next_wake) => {}
        }
    }
}

fn options_from_urls(urls: impl Iterator<Item = String>) -> Vec<M3u8Option> {
    urls.filter_map(|url| {
        // 透過 CDN 適配器判斷 URL 是否已過期
        if crate::cdn_adapter::is_url_expired(&url) {
            println!("⏳ 忽略已過期的 URL: {}", url);
            return None;
        }
        Some(M3u8Option {
            resolution: parse_resolution_label(&url),
            url,
        })
    })
    .collect()
}

pub fn parse_resolution_label(url: &str) -> String {
    // NOTE: 優先比對 _1080p.mp4 / _720p.mp4 這類 Avjoy 後綴格式
    let re_suffix = Regex::new(r"(?i)_((?:2160|1440|1080|720|540|480|360|240)p)\.mp4(?:[?#]|$)")
        .expect("valid regex");
    if let Some(caps) = re_suffix.captures(url) {
        if let Some(m) = caps.get(1) {
            return m.as_str().to_ascii_lowercase();
        }
    }

    // 一般 m3u8 路徑中的解析度片段（如 /720p/ 或 720p 字串）
    let re = Regex::new(r"(?i)(?:^|[^\d])((?:2160|1440|1080|720|540|480|360|240)p)(?:[^\d]|$)")
        .expect("valid regex");
    re.captures(url)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_ascii_lowercase())
        .unwrap_or_else(|| {
            // 如果找不到解析度，試著從 URL 中提取檔名
            if let Ok(parsed) = url::Url::parse(url) {
                if let Some(segments) = parsed.path_segments() {
                    if let Some(last) = segments.last() {
                        if !last.is_empty() {
                            return last.to_string();
                        }
                    }
                }
            }
            UNKNOWN_RESOLUTION.to_string()
        })
}

pub fn choose_resolution(options: &[M3u8Option], target: &str) -> Option<M3u8Option> {
    options
        .iter()
        .find(|option| option.resolution == target)
        .cloned()
        .or_else(|| closest_resolution(options, target))
}

fn closest_resolution(options: &[M3u8Option], target: &str) -> Option<M3u8Option> {
    let target_value = target.trim_end_matches('p').parse::<i32>().ok()?;
    options
        .iter()
        .filter_map(|option| {
            let value = option
                .resolution
                .trim_end_matches('p')
                .parse::<i32>()
                .ok()?;
            Some(((value - target_value).abs(), option.clone()))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, option)| option)
}

pub fn origin_from_url(page_url: &str) -> Result<String> {
    let url = Url::parse(page_url).context("invalid page URL")?;
    let origin = url.origin().ascii_serialization();
    if origin == "null" {
        return Err(anyhow!("page URL must include an HTTP origin"));
    }
    Ok(origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_distinct_m3u8_urls() {
        let html = r#"
            <script>
            const a = "https://cdn.example.com/video/720p/index.m3u8?token=abc";
            const b = "https://cdn.example.com/video/1080p/index.m3u8?token=def";
            </script>
        "#;
        let urls = extract_m3u8_urls_from_html(html);
        assert_eq!(urls.len(), 2);
        assert!(
            urls.contains(&"https://cdn.example.com/video/720p/index.m3u8?token=abc".to_string())
        );
        assert!(
            urls.contains(&"https://cdn.example.com/video/1080p/index.m3u8?token=def".to_string())
        );
    }

    #[test]
    fn parses_resolution_labels_from_paths() {
        // M3U8 路徑格式
        assert_eq!(
            parse_resolution_label("https://cdn.example.com/video/720p/index.m3u8?token=abc"),
            "720p"
        );
        assert_eq!(
            parse_resolution_label("https://cdn.example.com/video/1080p/index.m3u8?token=def"),
            "1080p"
        );
        assert_eq!(
            parse_resolution_label("https://cdn.example.com/video/stream.m3u8?token=ghi"),
            "stream.m3u8"
        );
        // Avjoy MP4 後綴格式 (_1080p.mp4)
        assert_eq!(
            parse_resolution_label("https://media-cdn3.avjoy.me/video/TOKEN/1778925984/75842_1080p.mp4"),
            "1080p"
        );
        assert_eq!(
            parse_resolution_label("https://media-cdn3.avjoy.me/video/TOKEN/1778925984/75842_720p.mp4"),
            "720p"
        );
    }

    #[test]
    fn dynamic_capture_script_hooks_common_js_request_sources() {
        let script = dynamic_capture_script();
        assert!(script.contains("window.fetch"));
        assert!(script.contains("XMLHttpRequest.prototype.open"));
        assert!(script.contains("performance.getEntriesByType"));
        assert!(script.contains(DYNAMIC_CAPTURE_HOST));
    }

    #[test]
    fn records_m3u8_candidates_from_resource_urls() {
        let found = Arc::new(Mutex::new(BTreeSet::new()));
        let (tx, mut rx) = mpsc::unbounded_channel();
        record_m3u8_candidate(
            "https://cdn.example.com/video/720p/index.m3u8?token=abc",
            &found,
            &tx,
        );

        assert!(rx.try_recv().is_ok());
        let urls = found.lock().unwrap();
        assert!(urls.contains("https://cdn.example.com/video/720p/index.m3u8?token=abc"));
    }

    #[test]
    fn chooses_matching_resolution_for_token_refresh() {
        let options = vec![
            M3u8Option {
                url: "https://cdn.example.com/video/720p/index.m3u8?token=fresh-a".into(),
                resolution: "720p".into(),
            },
            M3u8Option {
                url: "https://cdn.example.com/video/1080p/index.m3u8?token=fresh-b".into(),
                resolution: "1080p".into(),
            },
        ];

        let selected = choose_resolution(&options, "1080p").unwrap();
        assert_eq!(
            selected.url,
            "https://cdn.example.com/video/1080p/index.m3u8?token=fresh-b"
        );
    }
}
