## MODIFIED Requirements

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
