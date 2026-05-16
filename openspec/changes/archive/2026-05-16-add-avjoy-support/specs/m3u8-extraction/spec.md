## ADDED Requirements

### Requirement: Avjoy.me MP4 and M3U8 extraction

The system SHALL support extracting video sources from `avjoy.me`. Since avjoy.me SHALL use HLS for advertisements and direct MP4 for main content, the extraction logic SHALL:
- Prioritize `.mp4` URLs found in `video` tags or `source` tags if they belong to the `avjoy.me` or `media-cdn*.avjoy.me` domains.
- Automatically filter out known advertisement CDNs (e.g., `growcdnssedge.com`, `nexusriftcore4.cyou`) when identifying the main video content.
- Capture the `currentSrc` of the primary video element in the headless WebView after any pre-roll advertisements have been skipped or finished loading.

#### Scenario: Avjoy main video is MP4

- **WHEN** user provides an `avjoy.me` video page URL
- **AND** the primary video player loads an MP4 file from `media-cdn3.avjoy.me`
- **THEN** the system SHALL extract this MP4 URL as a download candidate
- **AND** label it with the appropriate resolution (e.g., 1080p)

##### Example: Avjoy source identification

| Detected URL | Domain Type | Action |
|---|---|---|
| `https://media-hls.growcdnssedge.com/.../240p.m3u8` | Ad CDN | Ignore or label as Ad |
| `https://media-cdn3.avjoy.me/.../75842_1080p.mp4` | Main Content | **Extract as primary candidate** |

#### Scenario: Resolution parsing from Avjoy MP4

- **WHEN** an extracted MP4 URL from Avjoy contains a resolution suffix like `_1080p.mp4` or `_720p.mp4`
- **THEN** the system SHALL correctly parse the resolution (1080p, 720p) and display it in the selection dialog.

### Requirement: Avjoy.me CDN adapter

The system SHALL include a CDN adapter for `avjoy.me` content domains (e.g., `media-cdn*.avjoy.me`). The adapter SHALL:
- Match URLs belonging to Avjoy's content delivery network.
- Detect expiration based on the Unix timestamp segment in the URL (e.g., `/1778925984/` in the path).
- Support token refresh by re-extracting the URL from the original page.

#### Scenario: Avjoy URL expiration detection

- **WHEN** an Avjoy MP4 URL contains a Unix timestamp in its path
- **AND** that timestamp is in the past
- **THEN** the adapter SHALL report the URL as expired, triggering a relay if needed.
