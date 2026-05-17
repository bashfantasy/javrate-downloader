# downloader-ui Specification

## Purpose

TBD - created by archiving change 'm3u8-relay-downloader'. Update Purpose after archive.

## Requirements

### Requirement: URL input area

The UI SHALL provide a task creation form where users can:
- Paste a video page URL into a text input field
- View and optionally modify the save directory path (pre-filled with a default download directory)
- View and optionally modify the output filename (pre-filled with a default name derived from the URL or page title, with `.mp4` extension)
- Click a "Start Download" button to submit

The system SHALL validate that the input URL is valid and the save directory exists before proceeding.

#### Scenario: User submits a valid URL with default save path

- **WHEN** user pastes a URL into the input field
- **AND** the save directory and filename are left at their default values
- **AND** clicks the "Start Download" button
- **THEN** the system SHALL create a new download task with the default save path
- **AND** the input fields SHALL be cleared for the next entry

#### Scenario: User customizes save path and filename

- **WHEN** user pastes a URL into the input field
- **AND** modifies the save directory via a folder picker or direct text input
- **AND** modifies the filename to a custom value
- **AND** clicks the "Start Download" button
- **THEN** the system SHALL create a new download task with the user-specified save path and filename

#### Scenario: User submits an invalid URL

- **WHEN** user enters text that is not a valid URL
- **AND** clicks the "Start Download" button
- **THEN** the system SHALL display an inline error message indicating the URL is invalid
- **AND** the system SHALL NOT create a task

---
### Requirement: Resolution selection dialog

After the m3u8 extraction module returns one or more URLs, the UI SHALL always display a modal dialog listing all available m3u8 URLs with their resolution labels, regardless of the number of results. The dialog SHALL NOT be skipped even when only a single URL is found.

Each m3u8 URL text in the dialog SHALL use CSS `word-break: break-all` (or equivalent) styling to ensure long URLs wrap within the dialog width and are fully visible without horizontal scrolling.

The dialog SHALL provide two action options for each selected URL:
- **Download**: proceed with downloading the selected m3u8 URL (existing behavior)
- **Copy URL**: copy the selected m3u8 URL to the system clipboard and close the dialog without starting a download

#### Scenario: Single m3u8 URL found

- **WHEN** m3u8 extraction finds exactly 1 URL
- **THEN** the system SHALL display the resolution selection dialog with that single URL listed
- **AND** the user SHALL choose to either download or copy the URL

#### Scenario: Multiple resolutions available

- **WHEN** m3u8 extraction finds 3 URLs with labels "720p", "1080p", and "Unknown resolution"
- **THEN** the system SHALL display a modal dialog listing all 3 options
- **AND** the user SHALL select one option
- **AND** the user SHALL choose to either download or copy the selected URL

#### Scenario: User copies URL without downloading

- **WHEN** the user selects an m3u8 URL in the dialog
- **AND** clicks the "Copy URL" button
- **THEN** the system SHALL copy the full m3u8 URL to the system clipboard
- **AND** the dialog SHALL close
- **AND** the system SHALL NOT create a download task

#### Scenario: Long URL wrapping

- **WHEN** the dialog displays an m3u8 URL that exceeds the dialog width
- **THEN** the URL text SHALL wrap to multiple lines within the dialog
- **AND** the complete URL SHALL be visible without horizontal scrolling


<!-- @trace
source: motv-cdn-support
updated: 2026-05-16
code:
  - src/styles/app.css
  - src/components/ResolutionDialog.tsx
  - src-tauri/src/extraction.rs
  - src/App.tsx
  - src-tauri/src/cdn_adapter.rs
-->

---
### Requirement: Task list display

The UI SHALL display all download tasks in a scrollable list. Each task item SHALL show:
- The video page URL (truncated if necessary)
- Current task state (with color-coded status badge)
- Progress bar with percentage
- Download speed and ETA
- Fragment progress (frag X/Y)
- Relay attempt count (when applicable)

#### Scenario: Task list shows all tasks

- **WHEN** there are 5 tasks in various states
- **THEN** the UI SHALL display all 5 tasks in the list
- **AND** each task SHALL show its current state and progress information

##### Example: Status badge colors

| State | Badge Color | Badge Text |
|---|---|---|
| Pending | Gray | 等待中 |
| Extracting | Blue | 解析中 |
| Selecting | Yellow | 選擇解析度 |
| Downloading | Green | 下載中 |
| Relaying | Orange | 接力中 |
| Paused | Gray | 已暫停 |
| Completed | Green | 已完成 |
| Cancelled | Red | 已取消 |
| Failed | Red | 失敗 |

---
### Requirement: Task control buttons

Each task in the list SHALL display context-appropriate action buttons based on its current state:
- **Downloading** state: Show Pause and Cancel buttons
- **Paused** state: Show Resume and Cancel buttons
- **Extracting/Relaying** state: Show Cancel button only
- **Completed/Cancelled/Failed** state: No action buttons

#### Scenario: Downloading task shows pause and cancel

- **WHEN** a task is in Downloading state
- **THEN** the UI SHALL display a Pause button and a Cancel button for that task

#### Scenario: Completed task shows no buttons

- **WHEN** a task is in Completed state
- **THEN** the UI SHALL NOT display any action buttons for that task

---
### Requirement: Real-time progress bar

Each downloading task SHALL display an animated progress bar that updates in real-time based on progress events from the backend. The progress bar SHALL incorporate the following visual enhancements:
- A gradient fill transitioning from a cool tone (e.g., blue) on the left to a warm tone (e.g., green) on the right as progress increases
- A subtle pulsing glow effect on the leading edge of the progress bar to indicate active downloading
- Smooth CSS transition animation (minimum 300ms ease-out) when the progress percentage changes
- The download percentage SHALL be displayed as a numerical label overlaid on or adjacent to the progress bar

When a task is in Relaying state, the progress bar glow effect SHALL change color (e.g., to orange) to visually indicate the relay process.

#### Scenario: Progress bar updates with smooth animation

- **WHEN** the backend emits a progress event changing from 40.0% to 45.3%
- **THEN** the progress bar fill SHALL smoothly animate from 40.0% to 45.3% over at least 300ms
- **AND** the gradient fill SHALL render across the filled portion
- **AND** the pulsing glow effect SHALL be visible at the leading edge
- **AND** the numerical label SHALL display "45.3%"

#### Scenario: Progress bar visual state during relay

- **WHEN** a task transitions to Relaying state
- **THEN** the progress bar glow effect SHALL change to an orange color
- **AND** the progress bar fill SHALL remain at the last recorded percentage

---
### Requirement: Application window layout

The application window SHALL use a single-page layout with:
- A fixed top section containing the URL input area with save path configuration
- A scrollable main section containing the task list
- A minimal, modern dark-themed design optimized for macOS

#### Scenario: Window layout structure

- **WHEN** the application launches
- **THEN** the URL input area (including save path and filename fields) SHALL be visible at the top of the window
- **AND** the task list SHALL fill the remaining space below
- **AND** the task list SHALL be scrollable when tasks exceed the visible area

---
### Requirement: Custom save path per task

Each download task SHALL store an individual save directory path and output filename. The system SHALL provide a default download directory (configurable in app settings) and a default filename derived from the video page URL or page title. Users SHALL be able to override both the directory and filename before starting the download.

The save directory picker SHALL use the native macOS folder selection dialog.

#### Scenario: Default save path applied

- **WHEN** user submits a URL without modifying the save path fields
- **THEN** the task SHALL use the default download directory
- **AND** the filename SHALL be automatically derived from the URL path or page title with `.mp4` extension

#### Scenario: User selects custom directory via folder picker

- **WHEN** user clicks the folder picker button next to the save directory field
- **THEN** the system SHALL open the native macOS folder selection dialog
- **AND** after the user selects a folder, the save directory field SHALL update to the selected path

#### Scenario: User enters custom filename

- **WHEN** user modifies the filename field to "my-video.mp4"
- **THEN** the task SHALL use "my-video.mp4" as the output filename

#### Scenario: Invalid save directory

- **WHEN** user enters a save directory path that does not exist
- **AND** clicks the "Start Download" button
- **THEN** the system SHALL display an error message indicating the directory does not exist
- **AND** the system SHALL NOT create the task