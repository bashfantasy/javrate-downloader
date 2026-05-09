## ADDED Requirements

### Requirement: Task creation from URL

The system SHALL create a new download task when the user submits a video page URL along with a save directory path and output filename. Each task SHALL be assigned a unique identifier and initialized in the Pending state. The task SHALL store the save directory and filename as part of its configuration.

If the user does not specify a save directory, the system SHALL use the default download directory. If the user does not specify a filename, the system SHALL derive one from the URL path or page title with `.mp4` extension.

#### Scenario: New task created with default save path

- **WHEN** user submits a video page URL via the input field without modifying the save path
- **THEN** the system SHALL create a new task with a unique ID
- **AND** the task SHALL be initialized in Pending state with the default save directory and auto-derived filename
- **AND** the task SHALL appear in the task list immediately

#### Scenario: New task created with custom save path and filename

- **WHEN** user submits a video page URL with a custom save directory and filename
- **THEN** the system SHALL create a new task storing the user-specified save directory and filename
- **AND** the task SHALL be initialized in Pending state

### Requirement: Task state machine

Each task SHALL follow the state machine defined below:

- **Pending** → Extracting (automatic on task start)
- **Extracting** → Selecting (when multiple m3u8 URLs found) OR Downloading (when single m3u8 URL found)
- **Selecting** → Downloading (after user selects resolution)
- **Downloading** → Completed (on success) OR Relaying (on 403) OR Paused (user pause) OR Cancelled (user cancel) OR Failed (unrecoverable error)
- **Relaying** → Downloading (after successful relay)
- **Paused** → Downloading (user resume) OR Cancelled (user cancel)

Invalid state transitions SHALL be rejected and logged as errors.

#### Scenario: Valid state transition

- **WHEN** a task in Downloading state receives a pause command
- **THEN** the task SHALL transition to Paused state

#### Scenario: Invalid state transition rejected

- **WHEN** a task in Completed state receives a pause command
- **THEN** the system SHALL reject the transition
- **AND** log a warning indicating the invalid state transition

##### Example: State transition matrix

| Current State | Pause | Resume | Cancel | 403 Detected | Complete |
|---|---|---|---|---|---|
| Pending | reject | reject | accept→Cancelled | reject | reject |
| Downloading | accept→Paused | reject | accept→Cancelled | accept→Relaying | accept→Completed |
| Paused | reject | accept→Downloading | accept→Cancelled | reject | reject |
| Relaying | reject | reject | accept→Cancelled | reject | reject |
| Completed | reject | reject | reject | reject | reject |
| Failed | reject | reject | reject | reject | reject |

### Requirement: Task progress tracking

Each task SHALL maintain the following progress fields, updated in real-time from yt-dlp output:
- Download percentage (0.0 to 100.0)
- Download speed (string, e.g., "5.2MiB/s")
- Estimated time remaining (string, e.g., "00:15")
- Current fragment number
- Total fragment count
- Relay attempt count

#### Scenario: Progress fields updated during download

- **WHEN** the download engine emits a progress event
- **THEN** the task's progress fields SHALL be updated with the latest values
- **AND** the frontend SHALL be notified of the update

### Requirement: Multiple concurrent tasks

The system SHALL support multiple download tasks running concurrently. Each task SHALL operate independently with its own yt-dlp subprocess and state.

#### Scenario: Two tasks downloading simultaneously

- **WHEN** task A is in Downloading state
- **AND** user creates task B with a different URL
- **THEN** task B SHALL proceed through its lifecycle independently
- **AND** both tasks SHALL report their progress independently

### Requirement: Task persistence across app restart

The system SHALL persist the task list (including state, progress, and configuration) to disk. Upon app restart, the system SHALL restore all non-completed tasks.

Tasks in Downloading or Relaying state at shutdown SHALL be restored in Paused state.

#### Scenario: App restarts with pending tasks

- **WHEN** the app is closed while tasks exist
- **AND** the app is reopened
- **THEN** all non-completed tasks SHALL be restored
- **AND** tasks that were Downloading or Relaying SHALL be restored as Paused
