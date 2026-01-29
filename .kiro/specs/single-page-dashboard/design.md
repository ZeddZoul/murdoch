# Design Document: Single-Page Dashboard Consolidation

## Overview

This design consolidates the multi-page Murdoch dashboard into a single-page layout where all sections (Dashboard, Violations, Rules, Config) are visible on one scrollable page. This eliminates navigation overhead and provides moderators with a comprehensive view of all moderation data at once.

### Core Problems Solved

1. **Navigation Overhead**: Eliminates need to switch between pages to see different data
2. **Context Switching**: Reduces cognitive load by keeping all information visible
3. **Workflow Efficiency**: Enables faster response to violations by showing everything at once
4. **Mobile Usability**: Provides better mobile experience with vertical scrolling instead of navigation
5. **Information Density**: Maximizes information visibility for power users

### Design Principles

- **Priority-Based Layout**: Most important sections (violations) at top, configuration at bottom
- **Progressive Disclosure**: Use collapsible sections for optional content
- **Smooth Navigation**: Implement smooth scrolling and section anchors
- **State Preservation**: Maintain section states when scrolling
- **Performance**: Lazy load content below the fold
- **Accessibility**: Maintain keyboard navigation and screen reader support

## Architecture

### Page Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                         Fixed Navbar                            │
│  [Logo] [Server Name] [Dashboard] [Violations] [Rules] [Config]│
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│                    Dashboard Section (Top)                      │
│  - Health Score Widget                                          │
│  - Metrics Cards (Messages, Violations, Response Time)          │
│  - Activity Charts (Time Series, Distribution)                  │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│                   Violations Section                            │
│  - Recent Violations List                                       │
│  - Filters (Severity, Type, User)                               │
│  - Pagination Controls                                          │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│                     Rules Section                               │
│  - Active Rules List                                            │
│  - Add/Edit/Delete Controls                                     │
│  - Rule Effectiveness Metrics                                   │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│                    Config Section (Bottom)                      │
│  - Server Settings                                              │
│  - Notification Preferences                                     │
│  - RBAC Settings                                                │
└─────────────────────────────────────────────────────────────────┘
```


### Data Flow

**Initial Page Load**:
1. User navigates to /dashboard
2. Router loads single-page layout
3. Render all section containers with loading states
4. Fetch data for visible sections (Dashboard, Violations)
5. Lazy load data for below-fold sections (Rules, Config)
6. Connect WebSocket for real-time updates
7. Initialize Intersection Observer for scroll tracking

**Section Navigation**:
1. User clicks navbar link (e.g., "Violations")
2. JavaScript scrolls smoothly to section anchor
3. Update URL hash (#violations)
4. Update navbar active indicator
5. Trigger lazy load if section not yet loaded

**Real-Time Updates**:
1. WebSocket receives violation event
2. Update Dashboard metrics
3. Prepend new violation to Violations section
4. Show toast notification
5. Update section refresh timestamps

## Components and Interfaces

### 1. Single-Page Layout Manager

**Purpose**: Orchestrate the single-page layout, section loading, and navigation

**Interface**:

```javascript
class SinglePageDashboard {
  constructor(serverId, serverName) {
    this.serverId = serverId;
    this.serverName = serverName;
    this.sections = new Map();
    this.intersectionObserver = null;
    this.activeSection = null;
  }

  /**
   * Initialize the single-page dashboard
   */
  async init() {
    this.renderLayout();
    this.setupIntersectionObserver();
    this.setupSmoothScrolling();
    this.loadVisibleSections();
    this.setupWebSocketHandlers();
  }

  /**
   * Render the complete page layout with all section containers
   */
  renderLayout() {
    // Render navbar with section links
    // Render section containers with loading states
    // Set up section refresh buttons
  }

  /**
   * Set up Intersection Observer to track visible sections
   */
  setupIntersectionObserver() {
    const options = {
      root: null,
      rootMargin: '-100px 0px -80% 0px',
      threshold: 0
    };

    this.intersectionObserver = new IntersectionObserver((entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          this.onSectionVisible(entry.target.id);
        }
      });
    }, options);

    // Observe all section elements
    document.querySelectorAll('[data-section]').forEach(section => {
      this.intersectionObserver.observe(section);
    });
  }

  /**
   * Handle section becoming visible
   */
  onSectionVisible(sectionId) {
    this.activeSection = sectionId;
    this.updateNavbarActiveIndicator(sectionId);
    this.updateUrlHash(sectionId);
    this.lazyLoadSection(sectionId);
  }

  /**
   * Smooth scroll to a section
   */
  scrollToSection(sectionId) {
    const section = document.getElementById(sectionId);
    if (section) {
      section.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  }

  /**
   * Load data for a specific section
   */
  async loadSection(sectionId) {
    const section = this.sections.get(sectionId);
    if (!section || section.loaded) return;

    section.loading = true;
    this.showSectionLoading(sectionId);

    try {
      await section.loadData();
      section.loaded = true;
      section.lastRefresh = new Date();
      this.renderSection(sectionId);
    } catch (error) {
      this.showSectionError(sectionId, error);
    } finally {
      section.loading = false;
    }
  }

  /**
   * Refresh a specific section
   */
  async refreshSection(sectionId) {
    const section = this.sections.get(sectionId);
    if (!section) return;

    section.loaded = false;
    await this.loadSection(sectionId);
  }
}
```


### 2. Section Components

**Purpose**: Encapsulate each dashboard section with its own data loading and rendering logic

**Dashboard Section**:

```javascript
class DashboardSection {
  constructor(serverId) {
    this.serverId = serverId;
    this.data = null;
  }

  async loadData() {
    const [metrics, healthMetrics, topOffenders] = await Promise.all([
      api.getServerMetrics(this.serverId, 'day'),
      api.getHealthMetrics(this.serverId),
      api.getTopOffenders(this.serverId, 'day')
    ]);

    this.data = { metrics, healthMetrics, topOffenders };
  }

  render() {
    return `
      <div class="section-header">
        <h2>Dashboard</h2>
        <button class="section-refresh-btn" data-section="dashboard">
          <svg>...</svg>
        </button>
      </div>
      <div class="section-content">
        ${this.renderMetricsCards()}
        ${this.renderHealthWidget()}
        ${this.renderCharts()}
      </div>
    `;
  }

  renderMetricsCards() { /* ... */ }
  renderHealthWidget() { /* ... */ }
  renderCharts() { /* ... */ }
}
```

**Violations Section**:

```javascript
class ViolationsSection {
  constructor(serverId) {
    this.serverId = serverId;
    this.data = null;
    this.filters = {
      severity: '',
      type: '',
      userId: ''
    };
    this.currentPage = 1;
  }

  async loadData() {
    const response = await api.getViolations(
      this.serverId,
      this.currentPage,
      this.filters
    );
    this.data = response;
  }

  render() {
    return `
      <div class="section-header">
        <h2>Violations</h2>
        <button class="section-refresh-btn" data-section="violations">
          <svg>...</svg>
        </button>
      </div>
      <div class="section-content">
        ${this.renderFilters()}
        ${this.renderViolationsList()}
        ${this.renderPagination()}
      </div>
    `;
  }

  renderFilters() { /* ... */ }
  renderViolationsList() { /* ... */ }
  renderPagination() { /* ... */ }

  applyFilters(filters) {
    this.filters = { ...this.filters, ...filters };
    this.currentPage = 1;
    this.loadData();
  }
}
```

**Rules Section**:

```javascript
class RulesSection {
  constructor(serverId) {
    this.serverId = serverId;
    this.data = null;
  }

  async loadData() {
    const [rules, effectiveness] = await Promise.all([
      api.getRules(this.serverId),
      api.getRuleEffectiveness(this.serverId)
    ]);
    this.data = { rules, effectiveness };
  }

  render() {
    return `
      <div class="section-header">
        <h2>Rules</h2>
        <button class="section-refresh-btn" data-section="rules">
          <svg>...</svg>
        </button>
      </div>
      <div class="section-content">
        ${this.renderRulesList()}
        ${this.renderAddRuleButton()}
        ${this.renderEffectivenessMetrics()}
      </div>
    `;
  }

  renderRulesList() { /* ... */ }
  renderAddRuleButton() { /* ... */ }
  renderEffectivenessMetrics() { /* ... */ }
}
```

**Config Section**:

```javascript
class ConfigSection {
  constructor(serverId) {
    this.serverId = serverId;
    this.data = null;
  }

  async loadData() {
    const config = await api.getConfig(this.serverId);
    this.data = config;
  }

  render() {
    return `
      <div class="section-header">
        <h2>Configuration</h2>
        <button class="section-refresh-btn" data-section="config">
          <svg>...</svg>
        </button>
      </div>
      <div class="section-content">
        ${this.renderServerSettings()}
        ${this.renderNotificationPreferences()}
        ${this.renderRBACSettings()}
      </div>
    `;
  }

  renderServerSettings() { /* ... */ }
  renderNotificationPreferences() { /* ... */ }
  renderRBACSettings() { /* ... */ }
}
```


### 3. Smooth Scrolling Navigation

**Purpose**: Provide smooth scrolling between sections with URL hash updates

**Implementation**:

```javascript
class SectionNavigator {
  constructor() {
    this.scrolling = false;
    this.scrollTimeout = null;
  }

  /**
   * Set up smooth scrolling for navbar links
   */
  setupSmoothScrolling() {
    document.querySelectorAll('[data-scroll-to]').forEach(link => {
      link.addEventListener('click', (e) => {
        e.preventDefault();
        const sectionId = link.dataset.scrollTo;
        this.scrollToSection(sectionId);
      });
    });

    // Handle initial hash on page load
    if (window.location.hash) {
      const sectionId = window.location.hash.substring(1);
      setTimeout(() => this.scrollToSection(sectionId), 100);
    }
  }

  /**
   * Scroll to a section with smooth animation
   */
  scrollToSection(sectionId) {
    const section = document.getElementById(sectionId);
    if (!section) return;

    this.scrolling = true;

    // Calculate offset for fixed navbar
    const navbarHeight = document.querySelector('nav').offsetHeight;
    const sectionTop = section.offsetTop - navbarHeight - 20;

    window.scrollTo({
      top: sectionTop,
      behavior: 'smooth'
    });

    // Update URL hash without triggering scroll
    history.replaceState(null, null, `#${sectionId}`);

    // Reset scrolling flag after animation
    clearTimeout(this.scrollTimeout);
    this.scrollTimeout = setTimeout(() => {
      this.scrolling = false;
    }, 1000);
  }

  /**
   * Update navbar active indicator based on current section
   */
  updateActiveIndicator(sectionId) {
    // Don't update during programmatic scrolling
    if (this.scrolling) return;

    document.querySelectorAll('[data-scroll-to]').forEach(link => {
      if (link.dataset.scrollTo === sectionId) {
        link.classList.add('active');
      } else {
        link.classList.remove('active');
      }
    });
  }
}
```

### 4. Lazy Loading Strategy

**Purpose**: Optimize initial page load by deferring below-fold content

**Implementation**:

```javascript
class LazyLoader {
  constructor() {
    this.loadedSections = new Set();
    this.loadingPromises = new Map();
  }

  /**
   * Check if a section should be lazy loaded
   */
  shouldLazyLoad(sectionId) {
    // Dashboard and Violations load immediately
    const immediateLoad = ['dashboard', 'violations'];
    return !immediateLoad.includes(sectionId);
  }

  /**
   * Lazy load a section when it becomes visible
   */
  async lazyLoadSection(sectionId, section) {
    // Skip if already loaded or loading
    if (this.loadedSections.has(sectionId)) return;
    if (this.loadingPromises.has(sectionId)) {
      return this.loadingPromises.get(sectionId);
    }

    // Create loading promise
    const loadPromise = (async () => {
      try {
        await section.loadData();
        this.loadedSections.add(sectionId);
        return true;
      } catch (error) {
        console.error(`Failed to lazy load section ${sectionId}:`, error);
        throw error;
      } finally {
        this.loadingPromises.delete(sectionId);
      }
    })();

    this.loadingPromises.set(sectionId, loadPromise);
    return loadPromise;
  }

  /**
   * Preload sections that are likely to be viewed next
   */
  preloadNextSections(currentSectionId) {
    const sectionOrder = ['dashboard', 'violations', 'rules', 'config'];
    const currentIndex = sectionOrder.indexOf(currentSectionId);

    if (currentIndex >= 0 && currentIndex < sectionOrder.length - 1) {
      const nextSectionId = sectionOrder[currentIndex + 1];
      // Preload next section in background
      setTimeout(() => {
        const section = this.sections.get(nextSectionId);
        if (section && !this.loadedSections.has(nextSectionId)) {
          this.lazyLoadSection(nextSectionId, section);
        }
      }, 500);
    }
  }
}
```


### 5. State Management

**Purpose**: Preserve section states (filters, pagination, collapsed state) across scrolling and page reloads

**Implementation**:

```javascript
class SectionStateManager {
  constructor() {
    this.states = new Map();
    this.storageKey = 'murdoch_section_states';
  }

  /**
   * Save section state to memory and sessionStorage
   */
  saveState(sectionId, state) {
    this.states.set(sectionId, state);

    // Persist to sessionStorage
    const allStates = {};
    this.states.forEach((value, key) => {
      allStates[key] = value;
    });
    sessionStorage.setItem(this.storageKey, JSON.stringify(allStates));
  }

  /**
   * Get section state from memory or sessionStorage
   */
  getState(sectionId) {
    // Check memory first
    if (this.states.has(sectionId)) {
      return this.states.get(sectionId);
    }

    // Check sessionStorage
    const stored = sessionStorage.getItem(this.storageKey);
    if (stored) {
      try {
        const allStates = JSON.parse(stored);
        if (allStates[sectionId]) {
          this.states.set(sectionId, allStates[sectionId]);
          return allStates[sectionId];
        }
      } catch (error) {
        console.error('Failed to parse section states:', error);
      }
    }

    return null;
  }

  /**
   * Clear all section states
   */
  clearStates() {
    this.states.clear();
    sessionStorage.removeItem(this.storageKey);
  }

  /**
   * Save collapsed state to localStorage (persists across sessions)
   */
  saveCollapsedState(sectionId, collapsed) {
    const key = `murdoch_section_collapsed_${sectionId}`;
    localStorage.setItem(key, collapsed.toString());
  }

  /**
   * Get collapsed state from localStorage
   */
  getCollapsedState(sectionId) {
    const key = `murdoch_section_collapsed_${sectionId}`;
    const stored = localStorage.getItem(key);
    return stored === 'true';
  }
}
```

### 6. Keyboard Navigation

**Purpose**: Enable efficient keyboard-based navigation between sections

**Implementation**:

```javascript
class KeyboardNavigator {
  constructor(sectionNavigator) {
    this.sectionNavigator = sectionNavigator;
    this.sectionOrder = ['dashboard', 'violations', 'rules', 'config'];
    this.shortcuts = {
      'Alt+1': 'dashboard',
      'Alt+2': 'violations',
      'Alt+3': 'rules',
      'Alt+4': 'config',
      '?': 'help'
    };
  }

  /**
   * Set up keyboard event listeners
   */
  setupKeyboardNavigation() {
    document.addEventListener('keydown', (e) => {
      // Check for keyboard shortcuts
      const key = this.getKeyCombo(e);
      const sectionId = this.shortcuts[key];

      if (sectionId === 'help') {
        e.preventDefault();
        this.showKeyboardShortcutsHelp();
        return;
      }

      if (sectionId) {
        e.preventDefault();
        this.sectionNavigator.scrollToSection(sectionId);
        return;
      }

      // Arrow key navigation
      if (e.key === 'ArrowDown' && e.altKey) {
        e.preventDefault();
        this.navigateToNextSection();
      } else if (e.key === 'ArrowUp' && e.altKey) {
        e.preventDefault();
        this.navigateToPreviousSection();
      }
    });
  }

  /**
   * Get keyboard shortcut combo string
   */
  getKeyCombo(e) {
    const parts = [];
    if (e.altKey) parts.push('Alt');
    if (e.ctrlKey) parts.push('Ctrl');
    if (e.shiftKey) parts.push('Shift');
    parts.push(e.key);
    return parts.join('+');
  }

  /**
   * Navigate to the next section
   */
  navigateToNextSection() {
    const currentSection = this.sectionNavigator.activeSection;
    const currentIndex = this.sectionOrder.indexOf(currentSection);

    if (currentIndex >= 0 && currentIndex < this.sectionOrder.length - 1) {
      const nextSection = this.sectionOrder[currentIndex + 1];
      this.sectionNavigator.scrollToSection(nextSection);
    }
  }

  /**
   * Navigate to the previous section
   */
  navigateToPreviousSection() {
    const currentSection = this.sectionNavigator.activeSection;
    const currentIndex = this.sectionOrder.indexOf(currentSection);

    if (currentIndex > 0) {
      const prevSection = this.sectionOrder[currentIndex - 1];
      this.sectionNavigator.scrollToSection(prevSection);
    }
  }

  /**
   * Show keyboard shortcuts help overlay
   */
  showKeyboardShortcutsHelp() {
    const overlay = document.createElement('div');
    overlay.className = 'keyboard-shortcuts-overlay';
    overlay.innerHTML = `
      <div class="keyboard-shortcuts-modal">
        <h3>Keyboard Shortcuts</h3>
        <div class="shortcuts-list">
          <div class="shortcut-item">
            <kbd>Alt</kbd> + <kbd>1</kbd>
            <span>Jump to Dashboard</span>
          </div>
          <div class="shortcut-item">
            <kbd>Alt</kbd> + <kbd>2</kbd>
            <span>Jump to Violations</span>
          </div>
          <div class="shortcut-item">
            <kbd>Alt</kbd> + <kbd>3</kbd>
            <span>Jump to Rules</span>
          </div>
          <div class="shortcut-item">
            <kbd>Alt</kbd> + <kbd>4</kbd>
            <span>Jump to Config</span>
          </div>
          <div class="shortcut-item">
            <kbd>Alt</kbd> + <kbd>↓</kbd>
            <span>Next Section</span>
          </div>
          <div class="shortcut-item">
            <kbd>Alt</kbd> + <kbd>↑</kbd>
            <span>Previous Section</span>
          </div>
          <div class="shortcut-item">
            <kbd>?</kbd>
            <span>Show this help</span>
          </div>
        </div>
        <button class="btn btn-primary" onclick="this.closest('.keyboard-shortcuts-overlay').remove()">
          Close
        </button>
      </div>
    `;

    document.body.appendChild(overlay);

    // Close on Escape key
    const closeHandler = (e) => {
      if (e.key === 'Escape') {
        overlay.remove();
        document.removeEventListener('keydown', closeHandler);
      }
    };
    document.addEventListener('keydown', closeHandler);

    // Close on click outside
    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) {
        overlay.remove();
        document.removeEventListener('keydown', closeHandler);
      }
    });
  }
}
```


## Data Models

### Section State Model

```javascript
{
  sectionId: string,
  loaded: boolean,
  loading: boolean,
  lastRefresh: Date,
  collapsed: boolean,
  filters: {
    // Section-specific filters
  },
  pagination: {
    currentPage: number,
    perPage: number
  }
}
```

### Section Configuration

```javascript
{
  id: string,
  title: string,
  icon: string,
  priority: number, // 1 = top, 4 = bottom
  lazyLoad: boolean,
  collapsible: boolean,
  refreshable: boolean,
  component: SectionComponent
}
```

## Correctness Properties

_A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees._

### Property 1: Section Order Consistency

_For any_ page load, the sections SHALL appear in the order: Dashboard, Violations, Rules, Config from top to bottom.

**Validates: Requirements 1.2, 2.1, 2.2, 2.3, 2.4**

### Property 2: Navbar Active Indicator Accuracy

_For any_ visible section, the navbar SHALL highlight the corresponding navigation link as active.

**Validates: Requirements 3.2, 3.3**

### Property 3: Smooth Scroll Navigation

_For any_ navbar link click, the page SHALL scroll smoothly to the corresponding section within 1 second.

**Validates: Requirements 3.1**

### Property 4: URL Hash Synchronization

_For any_ section that becomes visible through scrolling, the URL hash SHALL update to reflect that section's ID.

**Validates: Requirements 3.4, 14.5**

### Property 5: Section State Persistence

_For any_ filter or pagination change in a section, the state SHALL persist when scrolling to other sections and returning.

**Validates: Requirements 8.1, 8.2, 8.3**

### Property 6: Lazy Loading Efficiency

_For any_ below-fold section, data SHALL NOT be loaded until the section becomes visible or is explicitly navigated to.

**Validates: Requirements 7.2, 7.3**

### Property 7: Real-Time Update Propagation

_For any_ WebSocket event (violation, metrics update, config change), all relevant sections SHALL update their displayed data.

**Validates: Requirements 6.5**

### Property 8: Section Refresh Independence

_For any_ section refresh action, only that section's data SHALL be reloaded without affecting other sections.

**Validates: Requirements 15.1, 15.3**

### Property 9: Mobile Layout Adaptation

_For any_ screen width less than 768 pixels, all sections SHALL stack vertically with full width and mobile-optimized controls.

**Validates: Requirements 5.1, 5.2, 5.3, 5.4**

### Property 10: Keyboard Navigation Functionality

_For any_ keyboard shortcut (Alt+1, Alt+2, Alt+3, Alt+4), the page SHALL scroll to the corresponding section.

**Validates: Requirements 10.2**

### Property 11: Backward Compatibility Redirects

_For any_ old route (/dashboard, /violations, /rules, /config), the page SHALL load the single-page layout and scroll to the corresponding section.

**Validates: Requirements 14.1, 14.2, 14.3, 14.4**

### Property 12: Section Loading State Visibility

_For any_ section that is loading data, a loading indicator SHALL be visible within that section without blocking other sections.

**Validates: Requirements 11.1, 11.4**

### Property 13: Accessibility Compliance

_For any_ section, the HTML SHALL use semantic section elements with proper ARIA labels and heading hierarchy.

**Validates: Requirements 13.1, 13.4**


## Error Handling

### Error Categories

**1. Section Load Errors**

- Network failure → Show error message with retry button in section
- API error (4xx, 5xx) → Display error details and retry option
- Timeout → Show timeout message and retry button
- Partial failure → Load successful sections, show errors for failed ones

**2. Navigation Errors**

- Invalid section ID → Scroll to top (dashboard section)
- Hash not found → Ignore and maintain current position
- Scroll interrupted → Allow user control, don't force scroll

**3. State Persistence Errors**

- sessionStorage full → Clear old states, save new ones
- localStorage unavailable → Fall back to memory-only state
- Invalid stored state → Reset to default state

**4. WebSocket Errors**

- Connection lost → Fall back to polling, show disconnected indicator
- Update failure → Log error, don't crash section
- Invalid event data → Log warning, ignore event

### Graceful Degradation Strategy

```
Feature                 | Dependency Failed     | Degraded Behavior
------------------------|----------------------|------------------
Smooth Scrolling        | CSS not supported    | Jump scroll (instant)
Lazy Loading            | IntersectionObserver | Load all sections immediately
State Persistence       | Storage unavailable  | Memory-only state (session)
Keyboard Shortcuts      | Event listener fails | Mouse navigation only
Section Refresh         | API down             | Show error, keep old data
Real-Time Updates       | WebSocket down       | Polling fallback
```

## Testing Strategy

### Unit Tests

**Single-Page Layout Manager**:
- Section initialization
- Intersection Observer setup
- Section visibility tracking
- URL hash updates
- Navbar active indicator updates

**Section Components**:
- Data loading
- Rendering with empty data
- Rendering with populated data
- Filter application
- Pagination

**Smooth Scrolling**:
- Scroll to section by ID
- Scroll offset calculation
- Hash update without triggering scroll
- Active indicator update

**State Management**:
- Save and retrieve section state
- sessionStorage persistence
- localStorage collapsed state
- State clearing

**Keyboard Navigation**:
- Shortcut detection
- Section navigation
- Help overlay display

### Integration Tests

**Full Page Load Flow**:
1. Navigate to /dashboard
2. Verify all section containers rendered
3. Verify Dashboard and Violations sections loaded
4. Verify Rules and Config sections show loading state
5. Scroll to Rules section
6. Verify Rules section loads data
7. Verify URL hash updates to #rules

**Section Navigation Flow**:
1. Click "Violations" in navbar
2. Verify smooth scroll to Violations section
3. Verify URL hash updates to #violations
4. Verify navbar highlights "Violations" as active
5. Verify section data is visible

**State Persistence Flow**:
1. Apply filters in Violations section
2. Scroll to Rules section
3. Scroll back to Violations section
4. Verify filters are still applied
5. Refresh page
6. Verify filters restored from sessionStorage

**Real-Time Update Flow**:
1. Connect WebSocket
2. Trigger violation event
3. Verify Dashboard metrics update
4. Verify new violation appears in Violations section
5. Verify toast notification shown

### Property Tests

**Property 1: Section Order Consistency**

```javascript
test('sections always appear in correct order', () => {
  // For any page load
  renderSinglePageDashboard(serverId);

  // Get all section elements
  const sections = document.querySelectorAll('[data-section]');
  const sectionIds = Array.from(sections).map(s => s.id);

  // Verify order
  expect(sectionIds).toEqual(['dashboard', 'violations', 'rules', 'config']);
});
```

**Property 2: Navbar Active Indicator Accuracy**

```javascript
test('navbar highlights active section', () => {
  // For any visible section
  const sectionIds = ['dashboard', 'violations', 'rules', 'config'];

  sectionIds.forEach(sectionId => {
    // Scroll to section
    scrollToSection(sectionId);

    // Wait for intersection observer
    waitFor(() => {
      // Verify navbar link is active
      const activeLink = document.querySelector('[data-scroll-to].active');
      expect(activeLink.dataset.scrollTo).toBe(sectionId);
    });
  });
});
```

**Property 3: Section State Persistence**

```javascript
test('section state persists across scrolling', () => {
  // For any section with filters
  const filters = { severity: 'high', type: 'ai' };

  // Apply filters in Violations section
  applyFilters('violations', filters);

  // Scroll to another section
  scrollToSection('rules');

  // Scroll back to Violations
  scrollToSection('violations');

  // Verify filters are still applied
  const currentFilters = getFilters('violations');
  expect(currentFilters).toEqual(filters);
});
```

**Property 4: Lazy Loading Efficiency**

```javascript
test('below-fold sections not loaded until visible', () => {
  // For any below-fold section
  const belowFoldSections = ['rules', 'config'];

  // Load page
  renderSinglePageDashboard(serverId);

  // Verify sections not loaded
  belowFoldSections.forEach(sectionId => {
    const section = getSectionState(sectionId);
    expect(section.loaded).toBe(false);
  });

  // Scroll to Rules section
  scrollToSection('rules');

  // Wait for lazy load
  waitFor(() => {
    const section = getSectionState('rules');
    expect(section.loaded).toBe(true);
  });
});
```

**Property 5: Keyboard Navigation Functionality**

```javascript
test('keyboard shortcuts navigate to sections', () => {
  // For any keyboard shortcut
  const shortcuts = [
    { key: 'Alt+1', section: 'dashboard' },
    { key: 'Alt+2', section: 'violations' },
    { key: 'Alt+3', section: 'rules' },
    { key: 'Alt+4', section: 'config' }
  ];

  shortcuts.forEach(({ key, section }) => {
    // Trigger keyboard shortcut
    triggerKeyboardShortcut(key);

    // Wait for scroll
    waitFor(() => {
      // Verify section is visible
      const sectionElement = document.getElementById(section);
      expect(isElementInViewport(sectionElement)).toBe(true);
    });
  });
});
```


## CSS and Styling

### Section Separation

```css
/* Section containers */
.dashboard-section {
  padding: 2rem 0;
  border-bottom: 2px solid var(--color-border);
  min-height: 100vh;
}

.dashboard-section:last-child {
  border-bottom: none;
}

/* Section headers */
.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1.5rem;
  padding-bottom: 1rem;
  border-bottom: 1px solid var(--color-border-light);
}

.section-header h2 {
  font-size: 2rem;
  font-weight: 700;
  color: var(--color-text-primary);
}

/* Section refresh button */
.section-refresh-btn {
  padding: 0.5rem;
  border-radius: 0.5rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: all 0.2s;
}

.section-refresh-btn:hover {
  background: var(--color-bg-tertiary);
  color: var(--color-text-primary);
}

.section-refresh-btn svg {
  width: 1.25rem;
  height: 1.25rem;
}

/* Loading states */
.section-loading {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 300px;
}

.section-loading-spinner {
  width: 3rem;
  height: 3rem;
  border: 3px solid var(--color-border);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Error states */
.section-error {
  padding: 2rem;
  text-align: center;
  background: var(--color-error-bg);
  border: 1px solid var(--color-error-border);
  border-radius: 0.5rem;
}

.section-error-title {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--color-error);
  margin-bottom: 0.5rem;
}

.section-error-message {
  color: var(--color-text-secondary);
  margin-bottom: 1rem;
}
```

### Navbar Active Indicator

```css
/* Navbar links */
.navbar-link {
  position: relative;
  padding: 0.75rem 1rem;
  color: var(--color-text-secondary);
  text-decoration: none;
  transition: color 0.2s;
}

.navbar-link:hover {
  color: var(--color-text-primary);
}

.navbar-link.active {
  color: var(--color-primary);
}

.navbar-link.active::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: var(--color-primary);
}
```

### Mobile Responsive Styles

```css
/* Mobile layout */
@media (max-width: 768px) {
  .dashboard-section {
    padding: 1.5rem 0;
    min-height: auto;
  }

  .section-header h2 {
    font-size: 1.5rem;
  }

  /* Stack cards vertically */
  .metrics-cards {
    grid-template-columns: 1fr !important;
  }

  /* Simplify charts */
  .chart-container {
    height: 250px !important;
  }

  /* Make tables scrollable */
  .table-container {
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
  }

  /* Larger touch targets */
  .btn,
  .form-input,
  .form-select {
    min-height: 44px;
  }
}
```

### Keyboard Shortcuts Overlay

```css
.keyboard-shortcuts-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.8);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 9999;
}

.keyboard-shortcuts-modal {
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 0.5rem;
  padding: 2rem;
  max-width: 500px;
  width: 90%;
}

.keyboard-shortcuts-modal h3 {
  font-size: 1.5rem;
  font-weight: 700;
  margin-bottom: 1.5rem;
  color: var(--color-text-primary);
}

.shortcuts-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  margin-bottom: 1.5rem;
}

.shortcut-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.shortcut-item kbd {
  display: inline-block;
  padding: 0.25rem 0.5rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 0.25rem;
  font-family: monospace;
  font-size: 0.875rem;
  color: var(--color-text-primary);
}

.shortcut-item span {
  color: var(--color-text-secondary);
}
```

### Print Styles

```css
@media print {
  /* Hide interactive elements */
  nav,
  .section-refresh-btn,
  .btn,
  .form-input,
  .form-select {
    display: none !important;
  }

  /* Remove borders and backgrounds */
  .dashboard-section {
    border-bottom: 1px solid #ccc;
    page-break-inside: avoid;
  }

  /* Ensure charts print */
  .chart-container canvas {
    max-width: 100%;
    height: auto !important;
  }

  /* Black text for readability */
  body {
    color: #000;
    background: #fff;
  }

  /* Add page breaks between sections */
  .dashboard-section {
    page-break-after: always;
  }

  .dashboard-section:last-child {
    page-break-after: auto;
  }
}
```

## Performance Optimizations

### 1. Intersection Observer Optimization

- Use `rootMargin` to trigger lazy loading before section is fully visible
- Disconnect observer for sections that have already loaded
- Use `threshold: 0` for immediate detection

### 2. Chart Rendering Optimization

- Destroy old Chart.js instances before creating new ones
- Use `maintainAspectRatio: false` for better mobile performance
- Limit data points for time series charts (max 100 points)
- Use `decimation` plugin for large datasets

### 3. DOM Manipulation Optimization

- Use `documentFragment` for batch DOM updates
- Minimize reflows by batching style changes
- Use `requestAnimationFrame` for smooth animations
- Debounce scroll event handlers (100ms)

### 4. Data Fetching Optimization

- Use `Promise.all()` for parallel API calls
- Cache API responses in memory (5-minute TTL)
- Implement request deduplication for concurrent identical requests
- Use HTTP caching headers (ETag, Cache-Control)

### 5. Memory Management

- Clean up event listeners when sections are destroyed
- Destroy Chart.js instances when sections are refreshed
- Clear old WebSocket event handlers
- Limit stored states to last 10 sections

## Accessibility Features

### 1. Semantic HTML

```html
<main role="main">
  <section id="dashboard" aria-labelledby="dashboard-heading" data-section="dashboard">
    <h2 id="dashboard-heading">Dashboard</h2>
    <!-- Content -->
  </section>

  <section id="violations" aria-labelledby="violations-heading" data-section="violations">
    <h2 id="violations-heading">Violations</h2>
    <!-- Content -->
  </section>

  <section id="rules" aria-labelledby="rules-heading" data-section="rules">
    <h2 id="rules-heading">Rules</h2>
    <!-- Content -->
  </section>

  <section id="config" aria-labelledby="config-heading" data-section="config">
    <h2 id="config-heading">Configuration</h2>
    <!-- Content -->
  </section>
</main>
```

### 2. Skip Links

```html
<nav>
  <a href="#main-content" class="skip-link">Skip to main content</a>
  <a href="#dashboard" class="skip-link">Skip to Dashboard</a>
  <a href="#violations" class="skip-link">Skip to Violations</a>
  <a href="#rules" class="skip-link">Skip to Rules</a>
  <a href="#config" class="skip-link">Skip to Config</a>
</nav>
```

### 3. ARIA Live Regions

```html
<div aria-live="polite" aria-atomic="true" class="sr-only" id="section-announcer">
  <!-- Announce section changes to screen readers -->
</div>
```

### 4. Focus Management

- Maintain visible focus indicators (2px outline)
- Trap focus in modals and overlays
- Return focus to trigger element when closing modals
- Ensure logical tab order through sections

### 5. Screen Reader Announcements

```javascript
function announceSectionChange(sectionName) {
  const announcer = document.getElementById('section-announcer');
  announcer.textContent = `Now viewing ${sectionName} section`;
}
```

