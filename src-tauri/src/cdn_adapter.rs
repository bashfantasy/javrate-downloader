//! CDN 適配器模組 — 可插拔的 CDN Token 拼接邏輯
//!
//! 不同的影音 CDN 平台使用不同的防盜鏈機制（Token、Signed URL 等）。
//! 透過適配器模式，讓接力（Relay）機制能夠針對不同 CDN 平台的 Token
//! 進行精準的拼接與替換，而不需要修改核心下載邏輯。
//!
//! 新增 CDN 支援只需：
//! 1. 實作 `CdnAdapter` trait
//! 2. 在 `ALL_ADAPTERS` 陣列中註冊

use regex::Regex;

// ─────────────────────────────────────────────
//  Trait 定義
// ─────────────────────────────────────────────

/// CDN 適配器 — 每個 CDN 平台各自實作 Token 拼接邏輯
pub trait CdnAdapter: Send + Sync {
    /// 適配器名稱（用於日誌）
    fn name(&self) -> &'static str;

    /// 判斷此適配器是否適用於給定的 URL
    fn matches(&self, url: &str) -> bool;

    /// 將新 URL 的認證資訊嫁接到舊 URL 上，保留畫質路徑。
    /// 回傳 `Some(patched_url)` 表示成功拼接，`None` 表示不適用。
    fn patch_url(&self, new_url: &str, old_url: &str) -> Option<String>;

    /// 回傳動態擷取時注入的 JavaScript 片段。
    /// 這段 JS 會在隱藏 WebView 中執行，用於從 HTML 原始碼中
    /// 主動拼湊出帶有 Token 的 M3U8 網址。
    /// `report` 函數在呼叫環境中已預先定義，直接呼叫即可回報 URL。
    fn js_extraction_snippet(&self) -> Option<&'static str> {
        None
    }

    /// 判斷 URL 是否已過期（例如 Token 中的 expires 時間戳已過期）
    fn is_expired(&self, url: &str) -> bool {
        // 預設檢查通用的 expires= 參數（支援 & 和 / 作為分隔符）
        if let Some(idx) = url.find("expires=") {
            let start = idx + 8;
            let end = url[start..]
                .find(|c: char| c == '&' || c == '/' || c == ' ')
                .map(|i| start + i)
                .unwrap_or(url.len());
            if let Ok(expires) = url[start..end].parse::<u64>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                return expires < now;
            }
        }
        false
    }
}

// ─────────────────────────────────────────────
//  通用工具：畫質路徑嫁接
// ─────────────────────────────────────────────

/// 從 URL 中擷取畫質路徑片段（如 `/720p/video.m3u8`）
fn extract_resolution_path(url: &str) -> Option<&str> {
    let re = Regex::new(r"/(?:2160|1440|1080|720|540|480|360|240)p/[^/]+\.m3u8$").unwrap();
    re.find(url).map(|m| m.as_str())
}

/// 將舊 URL 的畫質路徑嫁接到新 URL 上（替換新 URL 的最後一段路徑）
fn graft_resolution_path(new_url: &str, old_url: &str) -> Option<String> {
    let res_path = extract_resolution_path(old_url)?;

    // 如果新 URL 已經包含畫質路徑，不需要嫁接
    if extract_resolution_path(new_url).is_some() {
        return Some(new_url.to_string());
    }

    let mut patched = new_url.to_string();
    if let Some(idx) = patched.rfind('/') {
        patched.truncate(idx);
        patched.push_str(res_path);
        Some(patched)
    } else {
        None
    }
}

// ─────────────────────────────────────────────
//  BunnyCDN 適配器
// ─────────────────────────────────────────────

/// BunnyCDN 防盜鏈機制適配器
/// 適用於 videocdn.avking.xyz、*.b-cdn.net 等 BunnyCDN 節點
pub struct BunnyCdnAdapter;

impl CdnAdapter for BunnyCdnAdapter {
    fn name(&self) -> &'static str {
        "BunnyCDN"
    }

    fn matches(&self, url: &str) -> bool {
        url.contains("bcdn_token=")
            || url.contains("b-cdn.net")
            || url.contains("avking")
    }

    fn patch_url(&self, new_url: &str, old_url: &str) -> Option<String> {
        // 優先嫁接畫質路徑
        if let Some(grafted) = graft_resolution_path(new_url, old_url) {
            return Some(grafted);
        }

        // 如果沒有畫質路徑，退回只替換 token 與 expires
        let re_token = Regex::new(r"bcdn_token=[^&]+").unwrap();
        let re_expires = Regex::new(r"expires=\d+").unwrap();

        let mut patched = old_url.to_string();
        let mut changed = false;

        if let Some(new_token) = re_token.find(new_url) {
            patched = re_token.replace(&patched, new_token.as_str()).to_string();
            changed = true;
        }
        if let Some(new_expires) = re_expires.find(new_url) {
            patched = re_expires.replace(&patched, new_expires.as_str()).to_string();
            changed = true;
        }

        if changed {
            Some(patched)
        } else {
            None
        }
    }

    fn js_extraction_snippet(&self) -> Option<&'static str> {
        // NOTE: 這段 JS 會被注入到隱藏 WebView 中，
        // 從 HTML 原始碼中直接拼湊出帶有 BunnyCDN Token 的完整 M3U8 網址
        Some(r#"
          const tokenMatch = html.match(/bcdn_token=[a-zA-Z0-9\-_]+&expires=\d+&token_path=(%2F[^&"'\s\\]+%2F)/);
          if (tokenMatch) {
              const fullTokenStr = tokenMatch[0];
              const uuidPath = decodeURIComponent(tokenMatch[1]);
              // 嘗試從已知的 CDN 域名拼出完整網址
              const cdnHosts = html.match(/https?:\/\/[a-zA-Z0-9._-]*(?:b-cdn\.net|avking\.[a-z]+)/gi);
              const host = cdnHosts && cdnHosts[0] ? cdnHosts[0] : "https://videocdn.avking.xyz";
              const syntheticUrl = host + "/" + fullTokenStr + uuidPath + "playlist.m3u8";
              report(syntheticUrl);
          }
        "#)
    }
}

// ─────────────────────────────────────────────
//  CloudFront Signed URL 適配器
// ─────────────────────────────────────────────

/// AWS CloudFront Signed URL 適配器
/// 適用於 *.cloudfront.net 且帶有 Policy / Signature / Key-Pair-Id 參數的 URL
pub struct CloudFrontAdapter;

impl CdnAdapter for CloudFrontAdapter {
    fn name(&self) -> &'static str {
        "CloudFront"
    }

    fn matches(&self, url: &str) -> bool {
        url.contains("cloudfront.net") && url.contains("Policy=")
    }

    fn patch_url(&self, new_url: &str, old_url: &str) -> Option<String> {
        // 嫁接畫質路徑
        if let Some(grafted) = graft_resolution_path(new_url, old_url) {
            return Some(grafted);
        }

        // 將新 URL 的簽名參數替換到舊 URL 上
        let re_policy = Regex::new(r"Policy=[^&]+").unwrap();
        let re_sig = Regex::new(r"Signature=[^&]+").unwrap();
        let re_key = Regex::new(r"Key-Pair-Id=[^&]+").unwrap();

        let mut patched = old_url.to_string();
        let mut changed = false;

        for re in [&re_policy, &re_sig, &re_key] {
            if let Some(new_val) = re.find(new_url) {
                patched = re.replace(&patched, new_val.as_str()).to_string();
                changed = true;
            }
        }

        if changed { Some(patched) } else { None }
    }

    fn is_expired(&self, url: &str) -> bool {
        // CloudFront 的 Policy 是 base64 編碼，無法直接解析過期時間
        // 但如果有 expires 參數就用通用邏輯
        if url.contains("expires=") {
            // 委託給 trait 預設實作
            if let Some(idx) = url.find("expires=") {
                let start = idx + 8;
                let end = url[start..].find('&').map(|i| start + i).unwrap_or(url.len());
                if let Ok(expires) = url[start..end].parse::<u64>() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    return expires < now;
                }
            }
        }
        false
    }
}

// ─────────────────────────────────────────────
//  通用適配器（Fallback）
// ─────────────────────────────────────────────

/// 通用適配器 — 當沒有任何特化適配器匹配時，作為兜底方案。
/// 僅嘗試嫁接畫質路徑，不做任何 Token 拼接。
pub struct GenericAdapter;

impl CdnAdapter for GenericAdapter {
    fn name(&self) -> &'static str {
        "Generic"
    }

    fn matches(&self, _url: &str) -> bool {
        true // 永遠匹配（作為兜底）
    }

    fn patch_url(&self, new_url: &str, old_url: &str) -> Option<String> {
        // 只嘗試嫁接畫質路徑，其餘直接使用新 URL
        graft_resolution_path(new_url, old_url)
    }

    fn is_expired(&self, _url: &str) -> bool {
        false // 沒有 Token 機制，永不過期
    }
}

// ─────────────────────────────────────────────
//  適配器調度器
// ─────────────────────────────────────────────

/// 所有已註冊的 CDN 適配器（按優先順序排列，GenericAdapter 必須在最後）
static ALL_ADAPTERS: &[&dyn CdnAdapter] = &[
    &BunnyCdnAdapter,
    &CloudFrontAdapter,
    &GenericAdapter,
];

/// 根據 URL 自動選擇最適合的 CDN 適配器
pub fn select_adapter(url: &str) -> &'static dyn CdnAdapter {
    ALL_ADAPTERS
        .iter()
        .find(|a| a.matches(url))
        .copied()
        .unwrap_or(&GenericAdapter)
}

/// 將新 URL 的認證資訊嫁接到舊 URL 上（對外統一入口）
pub fn patch_m3u8_url(new_url: &str, old_url: &str) -> String {
    let adapter = select_adapter(old_url);
    println!("🔌 使用 CDN 適配器: {} (URL: {}...)", adapter.name(), &old_url[..old_url.len().min(60)]);

    adapter
        .patch_url(new_url, old_url)
        .unwrap_or_else(|| new_url.to_string())
}

/// 判斷 URL 是否已過期
pub fn is_url_expired(url: &str) -> bool {
    let adapter = select_adapter(url);
    adapter.is_expired(url)
}

/// 收集所有適配器的 JS 擷取片段，合併成一段完整的 JavaScript
pub fn all_js_extraction_snippets() -> String {
    ALL_ADAPTERS
        .iter()
        .filter_map(|a| a.js_extraction_snippet())
        .collect::<Vec<_>>()
        .join("\n")
}

// ─────────────────────────────────────────────
//  測試
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bunny_cdn_matches() {
        assert!(BunnyCdnAdapter.matches("https://videocdn.avking.xyz/bcdn_token=abc&expires=999/video.m3u8"));
        assert!(BunnyCdnAdapter.matches("https://vz-a59c6881-d86.b-cdn.net/abc/playlist.m3u8"));
        assert!(!BunnyCdnAdapter.matches("https://example.com/video.m3u8"));
    }

    #[test]
    fn bunny_cdn_patches_resolution() {
        let old = "https://videocdn.avking.xyz/bcdn_token=OLD&expires=1000&token_path=%2Fabc%2F/abc/720p/video.m3u8";
        let new = "https://videocdn.avking.xyz/bcdn_token=NEW&expires=2000&token_path=%2Fabc%2F/abc/playlist.m3u8";
        let patched = patch_m3u8_url(new, old);
        assert_eq!(patched, "https://videocdn.avking.xyz/bcdn_token=NEW&expires=2000&token_path=%2Fabc%2F/abc/720p/video.m3u8");
    }

    #[test]
    fn bunny_cdn_patches_token_only() {
        let old = "https://videocdn.avking.xyz/bcdn_token=OLD&expires=1000/stream.m3u8";
        let new = "https://videocdn.avking.xyz/bcdn_token=NEW&expires=2000/stream.m3u8";
        let patched = patch_m3u8_url(new, old);
        assert_eq!(patched, "https://videocdn.avking.xyz/bcdn_token=NEW&expires=2000/stream.m3u8");
    }

    #[test]
    fn cloudfront_matches() {
        assert!(CloudFrontAdapter.matches("https://d1234.cloudfront.net/video.m3u8?Policy=abc&Signature=xyz&Key-Pair-Id=K1"));
        assert!(!CloudFrontAdapter.matches("https://example.com/video.m3u8"));
    }

    #[test]
    fn generic_always_matches() {
        assert!(GenericAdapter.matches("https://anything.com/anything.m3u8"));
    }

    #[test]
    fn selects_correct_adapter() {
        assert_eq!(select_adapter("https://x.b-cdn.net/v.m3u8").name(), "BunnyCDN");
        assert_eq!(select_adapter("https://d1.cloudfront.net/v.m3u8?Policy=a").name(), "CloudFront");
        assert_eq!(select_adapter("https://example.com/v.m3u8").name(), "Generic");
    }

    #[test]
    fn expired_url_detected() {
        // BunnyCDN URL 帶有過期的 expires（GenericAdapter 不檢查過期）
        assert!(is_url_expired("https://cdn.avking.xyz/bcdn_token=abc&expires=1000000000/v.m3u8"));
        assert!(!is_url_expired("https://cdn.avking.xyz/bcdn_token=abc&expires=9999999999/v.m3u8"));
        // 通用 URL 永不過期
        assert!(!is_url_expired("https://example.com/video.m3u8"));
    }

    #[test]
    fn new_url_with_resolution_not_grafted() {
        let old = "https://cdn.example.com/abc/720p/video.m3u8";
        let new = "https://cdn.example.com/abc/1080p/video.m3u8";
        let patched = patch_m3u8_url(new, old);
        // 新 URL 已經有畫質路徑，不應該被覆蓋
        assert_eq!(patched, "https://cdn.example.com/abc/1080p/video.m3u8");
    }
}
