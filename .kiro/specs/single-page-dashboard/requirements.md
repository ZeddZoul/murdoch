# Requirements Document: Single-Page Dashboard Consolidation

## Introduction

This document specifies requirements for consolidating the multi-page Murdoch dashboard into a single-page view where all navigation sections (Dashboard, Violations, Rules, Config) are visible simultaneously. This improves moderator workflow by eliminating page navigation and providing a comprehensive overview at a glance.

## Glossary

- **Dashboard_Section**: The metrics and analytics area showing health scores, violation trends, and charts
- **Violations_Section**: The list of recent violations with user information and actions
- **Rules_Section**: The configuration area for moderation rules and thresholds
- **Config_Section**: The server configuration settings area
- **Single_Page_Layout**: A vertically scrollable page containing all sections in a logical order
- **Section_Navigation**: Quick-jump links or anchors to scroll to specific sections
- **Priority_Sections**: Violations and warnings displayed at the top for immediate visibility
- **Secondary_Sections**: Rules and config displayed at the bottom for less frequent access
- **Responsive_Layout**: Layout that adapts to mobile and desktop screen sizes
- **Section_Anchor**: HTML anchor link enabling direct navigation to a specific section

## Requirements

### Requirement 1: Single-Page Layout Structure

**User Story:** As a moderator, I want to see all dashboard information on one page, so that I can monitor everything without switching between pages.

#### Acceptance Criteria

1. WHEN a moderator loads the dashboard, THEN THE Single_Page_Layout SHALL display all sections in a single vertically scrollable page
2. THE Single_Page_Layout SHALL organize sections in priority order: Dashboard_Section, Violations_Section, Rules_Section, Config_Section
3. WHEN the page loads, THEN THE Dashboard_Section SHALL be visible at the top without scrolling
4. THE Single_Page_Layout SHALL maintain the existing navbar with section quick-jump links
5. THE Single_Page_Layout SHALL preserve all existing functionality from individual pages

### Requirement 2: Priority Section Placement

**User Story:** As a moderator, I want violations and warnings at the top, so that I can see critical information immediately.

#### Acceptance Criteria

1. THE Dashboard_Section SHALL be positioned at the top of the page showing key metrics
2. THE Violations_Section SHALL be positioned immediately below the Dashboard_Section
3. THE Rules_Section SHALL be positioned below the Violations_Section
4. THE Config_Section SHALL be positioned at the bottom of the page
5. WHEN viewing on mobile, THEN THE Priority_Sections SHALL remain at the top in the same order

### Requirement 3: Section Navigation

**User Story:** As a moderator, I want to quickly jump to specific sections, so that I can navigate efficiently on a long page.

#### Acceptance Criteria

1. WHEN a moderator clicks a navbar link, THEN THE Single_Page_Layout SHALL scroll smoothly to the corresponding section
2. THE navbar SHALL update the active link indicator based on the currently visible section
3. WHEN a moderator scrolls manually, THEN THE navbar SHALL highlight the section currently in view
4. THE Section_Navigation SHALL use Section_Anchor links for direct URL access (e.g., #violations)
5. WHEN a moderator shares a URL with a section anchor, THEN THE page SHALL load and scroll to that section

### Requirement 4: Visual Section Separation

**User Story:** As a moderator, I want clear visual separation between sections, so that I can easily distinguish different areas.

#### Acceptance Criteria

1. WHEN sections are displayed, THEN THE Single_Page_Layout SHALL use distinct background colors or borders to separate sections
2. THE Single_Page_Layout SHALL display section headers with clear typography and spacing
3. WHEN scrolling between sections, THEN THE visual separation SHALL remain clear and consistent
4. THE Single_Page_Layout SHALL use consistent padding and margins between sections
5. THE Single_Page_Layout SHALL maintain WCAG 2.1 AA contrast ratios for all section separators

### Requirement 5: Responsive Mobile Layout

**User Story:** As a moderator on mobile, I want the single-page layout to work well on small screens, so that I can monitor from anywhere.

#### Acceptance Criteria

1. WHEN viewing on screens smaller than 768 pixels wide, THEN THE Single_Page_Layout SHALL stack sections vertically with full width
2. WHEN viewing on mobile, THEN THE section headers SHALL remain visible and readable
3. WHEN viewing on mobile, THEN THE charts and tables SHALL adapt to mobile-optimized layouts
4. THE Single_Page_Layout SHALL maintain touch-friendly controls (minimum 44px targets) on mobile
5. THE Single_Page_Layout SHALL achieve a Lighthouse mobile score of at least 90

### Requirement 6: Section Content Preservation

**User Story:** As a moderator, I want all existing features to work in the single-page layout, so that I don't lose functionality.

#### Acceptance Criteria

1. THE Dashboard_Section SHALL display all metrics, charts, and analytics from the original dashboard page
2. THE Violations_Section SHALL display the violations list with pagination, filtering, and user information
3. THE Rules_Section SHALL display all rule configuration options with edit and delete functionality
4. THE Config_Section SHALL display all server configuration settings with save functionality
5. THE Single_Page_Layout SHALL preserve all real-time WebSocket updates for each section

### Requirement 7: Performance Optimization

**User Story:** As a moderator, I want the single-page layout to load quickly, so that I can start monitoring without delays.

#### Acceptance Criteria

1. WHEN the page loads, THEN THE Single_Page_Layout SHALL load all sections within 3 seconds on a standard connection
2. THE Single_Page_Layout SHALL use lazy loading for charts and heavy content below the fold
3. WHEN scrolling to a section, THEN THE content SHALL be rendered and interactive within 500 milliseconds
4. THE Single_Page_Layout SHALL maintain smooth scrolling performance (60fps) on desktop and mobile
5. THE Single_Page_Layout SHALL cache section data to minimize redundant API calls

### Requirement 8: Section State Management

**User Story:** As a moderator, I want section states to persist, so that my filters and selections remain when scrolling.

#### Acceptance Criteria

1. WHEN a moderator applies filters in the Violations_Section, THEN THE filters SHALL persist when scrolling to other sections
2. WHEN a moderator changes the time period in the Dashboard_Section, THEN THE selection SHALL persist during the session
3. WHEN a moderator edits a rule in the Rules_Section, THEN THE edit state SHALL be preserved if they scroll away
4. THE Single_Page_Layout SHALL use sessionStorage to persist section states across page reloads
5. WHEN a moderator refreshes the page, THEN THE section states SHALL be restored from sessionStorage

### Requirement 9: Collapsible Sections (Optional)

**User Story:** As a moderator, I want to collapse sections I'm not using, so that I can focus on relevant information.

#### Acceptance Criteria

1. WHERE a moderator wants to collapse a section, THE Single_Page_Layout SHALL provide collapse/expand controls for each section
2. WHEN a section is collapsed, THEN THE Single_Page_Layout SHALL show only the section header and a preview of key information
3. WHEN a section is expanded, THEN THE Single_Page_Layout SHALL show all section content
4. THE Single_Page_Layout SHALL persist collapsed/expanded states in localStorage
5. WHEN a moderator reloads the page, THEN THE collapsed/expanded states SHALL be restored

### Requirement 10: Keyboard Navigation

**User Story:** As a moderator using keyboard navigation, I want to navigate between sections efficiently, so that I can work without a mouse.

#### Acceptance Criteria

1. WHEN a moderator presses Tab, THEN THE focus SHALL move logically through interactive elements within each section
2. WHEN a moderator presses a keyboard shortcut (e.g., Alt+1, Alt+2), THEN THE page SHALL scroll to the corresponding section
3. THE Single_Page_Layout SHALL display keyboard shortcuts in a help overlay accessible via "?" key
4. WHEN a moderator uses arrow keys, THEN THE page SHALL scroll smoothly between sections
5. THE Single_Page_Layout SHALL maintain visible focus indicators for all interactive elements

### Requirement 11: Section Loading States

**User Story:** As a moderator, I want to see loading indicators for each section, so that I know when data is being fetched.

#### Acceptance Criteria

1. WHEN a section is loading data, THEN THE Single_Page_Layout SHALL display a loading spinner within that section
2. WHEN a section fails to load, THEN THE Single_Page_Layout SHALL display an error message with a retry button
3. WHEN a section loads successfully, THEN THE loading indicator SHALL be replaced with the section content
4. THE Single_Page_Layout SHALL allow other sections to load independently without blocking
5. WHEN all sections are loaded, THEN THE page SHALL display a "fully loaded" indicator in the navbar

### Requirement 12: Print-Friendly Layout

**User Story:** As a moderator, I want to print the dashboard, so that I can create physical reports.

#### Acceptance Criteria

1. WHEN a moderator prints the page, THEN THE Single_Page_Layout SHALL use print-optimized styles
2. WHEN printing, THEN THE Single_Page_Layout SHALL include all sections with clear page breaks
3. WHEN printing, THEN THE Single_Page_Layout SHALL hide interactive elements (buttons, inputs) that don't make sense on paper
4. WHEN printing, THEN THE Single_Page_Layout SHALL display charts and tables in a print-friendly format
5. THE Single_Page_Layout SHALL include a "Print Dashboard" button in the navbar

### Requirement 13: Accessibility Compliance

**User Story:** As a moderator using assistive technology, I want the single-page layout to be accessible, so that I can use the dashboard effectively.

#### Acceptance Criteria

1. THE Single_Page_Layout SHALL use semantic HTML5 section elements with proper ARIA labels
2. THE Single_Page_Layout SHALL provide skip links to jump between major sections
3. THE Single_Page_Layout SHALL announce section changes to screen readers when scrolling
4. THE Single_Page_Layout SHALL maintain logical heading hierarchy (h1, h2, h3) throughout all sections
5. THE Single_Page_Layout SHALL achieve WCAG 2.1 AA compliance for all interactive elements

### Requirement 14: Backward Compatibility

**User Story:** As a moderator with bookmarked URLs, I want old page URLs to redirect to the correct section, so that my bookmarks still work.

#### Acceptance Criteria

1. WHEN a moderator navigates to /dashboard, THEN THE Single_Page_Layout SHALL load and scroll to the Dashboard_Section
2. WHEN a moderator navigates to /violations, THEN THE Single_Page_Layout SHALL load and scroll to the Violations_Section
3. WHEN a moderator navigates to /rules, THEN THE Single_Page_Layout SHALL load and scroll to the Rules_Section
4. WHEN a moderator navigates to /config, THEN THE Single_Page_Layout SHALL load and scroll to the Config_Section
5. THE Single_Page_Layout SHALL update the URL hash when scrolling to reflect the current section

### Requirement 15: Section Refresh Controls

**User Story:** As a moderator, I want to refresh individual sections, so that I can update specific data without reloading the entire page.

#### Acceptance Criteria

1. WHEN a moderator clicks a section refresh button, THEN THE Single_Page_Layout SHALL reload only that section's data
2. THE Single_Page_Layout SHALL display a refresh button in each section header
3. WHEN a section is refreshing, THEN THE Single_Page_Layout SHALL show a loading indicator without hiding existing content
4. WHEN a section refresh fails, THEN THE Single_Page_Layout SHALL display an error message and keep the old data visible
5. THE Single_Page_Layout SHALL track the last refresh time for each section and display it in the section header
