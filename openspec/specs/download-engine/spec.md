# download-engine Specification

## Purpose

TBD - created by archiving change 'm3u8-relay-downloader'. Update Purpose after archive.

## Requirements

### Requirement: yt-dlp subprocess spawning

The system SHALL spawn a yt-dlp child process with the following command structure:

```
yt-dlp -N <thread_count> \
  -o "<output_path>" \
  --add-header "Referer: <page_url>" \
  --add-header "Origin: <origin_domain>" \
  --add-header "User-Agent: <safari_user_agent>" \
  "<m3u8_url>"
```

The default thread count SHALL be 20. The output path SHALL be the full file path composed of the task's save directory and output filename as specified by the user (or their defaults). The save directory and filename are provided per-task at creation time.

#### Scenario: Successful process spawn

- **WHEN** a download task transitions to the Downloading state
- **THEN** the system SHALL spawn a yt-dlp child process with the configured parameters
- **AND** the system SHALL pipe stdout and stderr for monitoring

#### Scenario: yt-dlp not found

- **WHEN** the system attempts to spawn yt-dlp
- **AND** the yt-dlp binary is not found on the system PATH
- **THEN** the system SHALL report an error indicating that yt-dlp is not installed
- **AND** the task SHALL transition to the Failed state

*Note: On macOS, GUI applications do not inherit the shell PATH. The system MUST explicitly inject common Homebrew paths (e.g., `/opt/homebrew/bin:/usr/local/bin`) before spawning the subprocess to ensure `yt-dlp` can be located.*

---
### Requirement: Real-time progress parsing

The system SHALL parse yt-dlp stdout output line by line to extract download progress information. The system SHALL extract the following fields when available:
- Download percentage (0.0% to 100.0%)
- Downloaded size
- Download speed (e.g., 5.2MiB/s)
- Estimated time remaining (ETA)
- Current fragment and total fragments (frag X/Y)

#### Scenario: Standard progress line parsing

- **WHEN** yt-dlp outputs a line matching the pattern `[download] XX.X% of ~XX.XXMB at XX.XXMiB/s ETA XX:XX frag X/Y`
- **THEN** the system SHALL extract percentage, speed, ETA, and fragment progress
- **AND** emit a progress event to the frontend

##### Example: Progress line parsing

| yt-dlp Output | Percentage | Speed | ETA | Fragment |
|---|---|---|---|---|
| `[download]  45.3% of ~128.50MiB at 5.20MiB/s ETA 00:15 frag 92/203` | 45.3 | 5.20MiB/s | 00:15 | 92/203 |
| `[download] 100.0% of ~128.50MiB at 4.80MiB/s ETA 00:00 frag 203/203` | 100.0 | 4.80MiB/s | 00:00 | 203/203 |

#### Scenario: Non-progress output line

- **WHEN** yt-dlp outputs a line that does not match the progress pattern
- **THEN** the system SHALL ignore that line for progress tracking purposes

---
### Requirement: HTTP 403 error detection

The system SHALL monitor yt-dlp stdout and stderr for strings indicating an HTTP 403 Forbidden error. Upon detection, the system SHALL emit an event signaling that a token refresh is needed.

The detection patterns SHALL include: `HTTP Error 403`, `403 Forbidden`, `HTTP error 403`.

#### Scenario: 403 error detected in output

- **WHEN** yt-dlp outputs a line containing `HTTP Error 403`
- **THEN** the system SHALL emit a relay-needed event for the corresponding task
- **AND** the system SHALL wait for the yt-dlp process to terminate before proceeding

---
### Requirement: Process pause via SIGINT

The system SHALL support pausing a download by sending a SIGINT signal to the running yt-dlp child process. This causes yt-dlp to gracefully stop and preserve its partial download state.

#### Scenario: User pauses a downloading task

- **WHEN** user triggers pause on a task in Downloading state
- **THEN** the system SHALL send SIGINT to the yt-dlp child process
- **AND** wait for the process to exit
- **AND** the task SHALL transition to Paused state

---
### Requirement: Process resume by restart

The system SHALL support resuming a paused download by spawning a new yt-dlp child process with the same parameters and output file path. yt-dlp SHALL automatically detect the existing partial download and resume from where it left off.

#### Scenario: User resumes a paused task

- **WHEN** user triggers resume on a task in Paused state
- **THEN** the system SHALL spawn a new yt-dlp process with identical parameters
- **AND** yt-dlp SHALL resume downloading from the previously downloaded fragment
- **AND** the task SHALL transition to Downloading state

---
### Requirement: Process cancellation via SIGTERM

The system SHALL support cancelling a download by sending a SIGTERM signal to the running yt-dlp child process and cleaning up the partial download files.

#### Scenario: User cancels a downloading task

- **WHEN** user triggers cancel on a task in Downloading or Paused state
- **THEN** the system SHALL send SIGTERM to the yt-dlp child process (if running)
- **AND** the task SHALL transition to Cancelled state

---
### Requirement: Download completion detection

The system SHALL detect download completion when yt-dlp exits with exit code 0. However, the system MUST prioritize the `relay-needed` flag over the exit code, because `yt-dlp` may erroneously exit with code 0 if its Python environment crashes (e.g., `ValueError` on closed files) after exhausting fragment retries during a 403 Forbidden error.

#### Scenario: Download completes successfully

- **WHEN** yt-dlp exits with exit code 0
- **AND** no HTTP 403 error was detected during the run
- **THEN** the task SHALL transition to Completed state
- **AND** the system SHALL emit a completion event

#### Scenario: False completion masked by 403 error

- **WHEN** yt-dlp exits with exit code 0
- **AND** an HTTP 403 error was detected during the run (needs_relay is true)
- **THEN** the system SHALL NOT transition to Completed state
- **AND** the system SHALL emit a relay-needed event instead
