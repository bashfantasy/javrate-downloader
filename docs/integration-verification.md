# Integration Verification

## External URL Check

Test URL:

`https://www.javrate.com/Movie/Detail/4b65e6c1-7c51-444a-be27-d11fe3d388aa.html`

Result: the request reached the site, but Cloudflare returned a block page instead of the movie detail HTML. The response did not contain an `.m3u8` URL and cannot be used as a stable automated integration fixture.

## Local Repeatable Coverage

The remaining integration scenarios are covered with local deterministic tests:

- Task creation with custom save path and filename
- Invalid URL and missing directory rejection
- Independent state updates for multiple tasks
- yt-dlp argument generation with output path and HTTP headers
- yt-dlp progress parsing, 403 detection, and completion detection
- Process registry preserving multiple task configs independently
- Relay retry limit behavior for attempts 1, 25, 49, 50, and 51
- Task persistence restore behavior, including Downloading/Relaying restored as Paused

Verification commands:

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```
