## ADDED Requirements

### Requirement: Automatic 403 relay trigger

The system SHALL automatically initiate the relay process when the download engine emits a relay-needed event (HTTP 403 detected). The relay process MUST NOT require any user interaction.

#### Scenario: Relay triggered by 403 detection

- **WHEN** the download engine detects an HTTP 403 error for a task in Downloading state
- **THEN** the system SHALL automatically transition the task to Relaying state
- **AND** initiate the token refresh and re-download sequence

### Requirement: Token refresh via re-extraction

During relay, the system SHALL re-extract m3u8 URLs from the original video page URL using the m3u8-extraction capability. The system SHALL select the m3u8 URL matching the resolution originally chosen by the user.

#### Scenario: Successful token refresh

- **WHEN** a task enters Relaying state
- **THEN** the system SHALL call the m3u8-extraction module with the original page URL
- **AND** select the m3u8 URL whose resolution matches the original selection
- **AND** use the new m3u8 URL to restart yt-dlp

#### Scenario: Resolution mismatch during refresh

- **WHEN** the re-extracted m3u8 URLs do not include a URL matching the original resolution
- **THEN** the system SHALL select the closest available resolution
- **AND** log a warning about the resolution mismatch

### Requirement: Seamless download restart

After obtaining a fresh m3u8 URL, the system SHALL restart the yt-dlp process with the new URL while keeping the original output file path. yt-dlp SHALL automatically resume from the last successfully downloaded fragment.

#### Scenario: Download resumes from last fragment

- **WHEN** the system restarts yt-dlp with a fresh m3u8 URL
- **THEN** yt-dlp SHALL detect the existing partial download file
- **AND** resume downloading from the next fragment after the last completed one
- **AND** the task SHALL transition back to Downloading state

### Requirement: Retry limit enforcement

The system SHALL enforce a maximum retry count of 50 relay attempts per task. When the retry limit is exceeded, the task SHALL transition to Failed state.

#### Scenario: Retry limit reached

- **WHEN** a task has undergone 50 relay attempts
- **AND** a new HTTP 403 error is detected
- **THEN** the system SHALL NOT initiate another relay
- **AND** the task SHALL transition to Failed state with an error message indicating the retry limit was exceeded

##### Example: Relay counter progression

| Relay Count | Action |
|---|---|
| 1 | Relay proceeds normally |
| 25 | Relay proceeds normally |
| 49 | Relay proceeds normally |
| 50 | Relay proceeds normally (last allowed attempt) |
| 51 | Task transitions to Failed |

### Requirement: Relay status reporting

During the relay process, the system SHALL report the current relay status to the frontend, including the relay attempt number and the current phase (re-extracting URL, restarting download).

#### Scenario: Relay progress visible in UI

- **WHEN** a task is in Relaying state
- **THEN** the system SHALL emit status events indicating the current relay phase
- **AND** include the relay attempt count (e.g., "Relay attempt 3/50: Re-extracting m3u8 URL...")
