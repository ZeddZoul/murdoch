# Requirements Document

## Introduction

This document specifies a web-based dashboard for the Murdoch Discord moderation bot. The dashboard provides server administrators with a visual interface to monitor moderation activity, configure bot settings, manage server rules, and review violation history. Authentication is handled via Discord OAuth to ensure only authorized server administrators can access their server's dashboard.

## Glossary

- **Dashboard**: The web-based administrative interface for Murdoch
- **Web_Server**: The Axum-based HTTP server serving the dashboard and API
- **OAuth_Handler**: Component managing Discord OAuth2 authentication flow
- **API_Router**: Component handling REST API requests from the frontend
- **Session_Manager**: Component managing authenticated user sessions
- **Frontend**: The HTML/CSS/JavaScript user interface
- **Server_Selector**: UI component allowing users to switch between servers they administer

## Requirements

### Requirement 1: Discord OAuth Authentication

**User Story:** As a server administrator, I want to log in with my Discord account, so that I can securely access the dashboard for servers I manage.

#### Acceptance Criteria

1. WHEN a user visits the dashboard without authentication, THE Web_Server SHALL redirect them to the Discord OAuth authorization page
2. WHEN Discord returns an authorization code, THE OAuth_Handler SHALL exchange it for access and refresh tokens
3. WHEN tokens are obtained, THE Session_Manager SHALL create a secure session cookie
4. WHEN a user is authenticated, THE Dashboard SHALL only show servers where the user has ADMINISTRATOR permission
5. WHEN a session expires, THE Session_Manager SHALL attempt to refresh using the refresh token
6. IF token refresh fails, THEN THE Session_Manager SHALL redirect the user to re-authenticate
7. WHEN a user clicks logout, THE Session_Manager SHALL invalidate the session and clear cookies

### Requirement 2: Server Selection

**User Story:** As a user managing multiple servers, I want to select which server to view, so that I can manage each server's moderation settings independently.

#### Acceptance Criteria

1. WHEN a user is authenticated, THE Server_Selector SHALL display a list of servers where the user is an administrator
2. WHEN a user selects a server, THE Dashboard SHALL load that server's metrics and configuration
3. WHEN a user has no administrable servers, THE Dashboard SHALL display an appropriate message
4. THE Server_Selector SHALL persist the last selected server in the session
5. WHEN the bot is not present in a server, THE Dashboard SHALL indicate this and provide an invite link

### Requirement 3: Metrics Dashboard

**User Story:** As a server administrator, I want to see moderation metrics visualized in charts, so that I can understand trends and patterns in my server's moderation activity.

#### Acceptance Criteria

1. THE Dashboard SHALL display a line chart showing messages processed over time
2. THE Dashboard SHALL display a pie chart showing violations by detection type (Regex vs AI)
3. THE Dashboard SHALL display a bar chart showing violations by severity level
4. THE Dashboard SHALL display key metrics cards: total messages, total violations, average response time
5. WHEN a time period is selected (hour/day/week/month), THE Dashboard SHALL update all charts accordingly
6. THE Dashboard SHALL auto-refresh metrics every 60 seconds
7. WHEN hovering over chart elements, THE Dashboard SHALL display detailed tooltips

### Requirement 4: Violation History

**User Story:** As a server administrator, I want to browse recent violations, so that I can review moderation actions and identify problematic users.

#### Acceptance Criteria

1. THE Dashboard SHALL display a paginated table of recent violations
2. WHEN viewing violations, THE Dashboard SHALL show: timestamp, user, reason, severity, detection type, action taken
3. THE Dashboard SHALL support filtering violations by severity level
4. THE Dashboard SHALL support filtering violations by detection type
5. THE Dashboard SHALL support filtering violations by user
6. WHEN clicking a violation, THE Dashboard SHALL show full details including message content hash
7. THE Dashboard SHALL support exporting violations to CSV

### Requirement 5: Server Rules Management

**User Story:** As a server administrator, I want to edit my server's moderation rules through the dashboard, so that I can customize moderation without using Discord commands.

#### Acceptance Criteria

1. THE Dashboard SHALL display the current server rules in an editable text area
2. WHEN rules are modified and saved, THE API_Router SHALL update the rules in the database
3. WHEN rules are saved, THE Dashboard SHALL display a success confirmation
4. IF rules save fails, THEN THE Dashboard SHALL display an error message
5. THE Dashboard SHALL provide a "Reset to Default" button to clear custom rules
6. THE Dashboard SHALL show when rules were last updated and by whom
7. THE Dashboard SHALL provide example rules templates that users can insert

### Requirement 6: Bot Configuration

**User Story:** As a server administrator, I want to adjust bot settings through the dashboard, so that I can fine-tune moderation behavior for my server.

#### Acceptance Criteria

1. THE Dashboard SHALL display current configuration: severity threshold, buffer timeout, buffer threshold
2. WHEN configuration is modified and saved, THE API_Router SHALL update the database
3. THE Dashboard SHALL validate configuration values before submission
4. IF configuration values are invalid, THEN THE Dashboard SHALL display validation errors
5. THE Dashboard SHALL provide tooltips explaining each configuration option
6. WHEN configuration is saved, THE Dashboard SHALL display a success confirmation

### Requirement 7: User Warnings Management

**User Story:** As a server administrator, I want to view and manage user warnings, so that I can review escalation status and clear warnings when appropriate.

#### Acceptance Criteria

1. THE Dashboard SHALL display a searchable list of users with active warnings
2. WHEN viewing a user, THE Dashboard SHALL show: current warning level, violation history, kicked status
3. THE Dashboard SHALL provide a button to clear warnings for a user
4. WHEN warnings are cleared, THE Dashboard SHALL log the action with the admin who cleared them
5. THE Dashboard SHALL support bulk clearing of warnings older than a specified date

### Requirement 8: API Security

**User Story:** As a system operator, I want the API to be secure, so that unauthorized users cannot access or modify server data.

#### Acceptance Criteria

1. THE API_Router SHALL validate session tokens on every request
2. THE API_Router SHALL verify the user has ADMINISTRATOR permission for the requested server
3. THE API_Router SHALL rate limit requests to prevent abuse
4. THE API_Router SHALL log all configuration changes with user ID and timestamp
5. IF an unauthorized request is made, THEN THE API_Router SHALL return 401 or 403 status
6. THE Web_Server SHALL serve the dashboard over HTTPS in production

### Requirement 9: Responsive Design

**User Story:** As a server administrator, I want to access the dashboard from any device, so that I can monitor my server on mobile or desktop.

#### Acceptance Criteria

1. THE Frontend SHALL adapt layout for desktop, tablet, and mobile screen sizes
2. THE Frontend SHALL maintain usability on screens as small as 320px wide
3. THE Frontend SHALL use a consistent color scheme matching Discord's dark theme
4. THE Frontend SHALL provide clear visual feedback for loading states
5. THE Frontend SHALL display error states gracefully with retry options

