/**
 * Single-Page Dashboard
 * Consolidates all dashboard sections into a single scrollable page
 * Uses simple API fetches with caching - no WebSocket complexity
 */

import { api } from './api.js';

/**
 * SinglePageDashboard - Main controller for the consolidated dashboard view
 */
export class SinglePageDashboard {
  constructor(serverId, serverName) {
    this.serverId = serverId;
    this.serverName = serverName;
    this.charts = {};
    this.refreshInterval = null;
    this.lastRefreshTimes = new Map();

    // Simple cache with 5 minute TTL
    this.cache = new Map();
    this.cacheTTL = 5 * 60 * 1000;

    // Section state (filters, pagination, etc.)
    this.state = {
      dashboard: { period: 'day' },
      violations: { page: 1, severity: '' },
      rules: { editing: false }
    };

    this.loadState();
  }

  /**
   * Load state from sessionStorage
   */
  loadState() {
    try {
      const saved = sessionStorage.getItem(`dashboard_state_${this.serverId}`);
      if (saved) {
        Object.assign(this.state, JSON.parse(saved));
      }
    } catch (e) { /* ignore */ }
  }

  /**
   * Save state to sessionStorage
   */
  saveState() {
    try {
      sessionStorage.setItem(`dashboard_state_${this.serverId}`, JSON.stringify(this.state));
    } catch (e) { /* ignore */ }
  }

  /**
   * Cached API fetch with deduplication
   */
  async fetchCached(key, fetchFn, forceRefresh = false) {
    if (!forceRefresh && this.cache.has(key)) {
      const { data, timestamp } = this.cache.get(key);
      if (Date.now() - timestamp < this.cacheTTL) {
        return data;
      }
    }

    const data = await fetchFn();
    this.cache.set(key, { data, timestamp: Date.now() });
    return data;
  }

  /**
   * Clear cache for a section
   */
  clearCache(section = null) {
    if (section) {
      for (const key of this.cache.keys()) {
        if (key.startsWith(section)) {
          this.cache.delete(key);
        }
      }
    } else {
      this.cache.clear();
    }
  }

  /**
   * Format time ago
   */
  formatTimeAgo(date) {
    const seconds = Math.floor((Date.now() - date) / 1000);
    if (seconds < 60) return 'just now';
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.floor(hours / 24)}d ago`;
  }

  /**
   * Escape HTML to prevent XSS
   */
  escapeHtml(text) {
    if (!text) return '';
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  /**
   * Show toast notification
   */
  showToast(message, type = 'info') {
    const colors = {
      success: 'bg-green-600',
      error: 'bg-red-600',
      info: 'bg-gray-700'
    };

    const toast = document.createElement('div');
    toast.className = `fixed bottom-4 right-4 px-6 py-3 rounded-lg shadow-lg z-50 ${colors[type] || colors.info} text-white transition-opacity`;
    toast.textContent = message;
    document.body.appendChild(toast);

    setTimeout(() => {
      toast.style.opacity = '0';
      setTimeout(() => toast.remove(), 300);
    }, 3000);
  }

  /**
   * Render the main layout
   */
  render() {
    const app = document.getElementById('app');
    app.innerHTML = `
      ${this.renderNavbar()}
      <main class="main-content">
        <div class="max-w-7xl mx-auto px-4 py-6 space-y-10">
          ${this.renderDashboardSection()}
          ${this.renderViolationsSection()}
          ${this.renderRulesSection()}
          ${this.renderConfigSection()}
        </div>
      </main>
    `;

    this.setupEventListeners();
    this.loadAllSections();

    // Update refresh times every 30 seconds
    this.refreshInterval = setInterval(() => this.updateRefreshTimes(), 30000);
  }

  /**
   * Render navbar
   */
  renderNavbar() {
    return `
      <header class="navbar-header">
        <div class="navbar-left">
          <a href="#/servers" class="navbar-icon-btn" title="Back to servers">
            <svg class="navbar-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
            </svg>
          </a>
          <span class="navbar-title">${this.escapeHtml(this.serverName)}</span>
        </div>
        <div class="navbar-right">
          <a href="#dashboard-section" class="nav-link navbar-icon-btn" data-section="dashboard" title="Dashboard">
            <svg class="navbar-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
            </svg>
          </a>
          <a href="#violations-section" class="nav-link navbar-icon-btn" data-section="violations" title="Violations">
            <svg class="navbar-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
          </a>
          <a href="#rules-section" class="nav-link navbar-icon-btn" data-section="rules" title="Rules">
            <svg class="navbar-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
            </svg>
          </a>
          <a href="#config-section" class="nav-link navbar-icon-btn" data-section="config" title="Settings">
            <svg class="navbar-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
          </a>
          <button id="theme-toggle" class="navbar-icon-btn" title="Toggle theme">
            <svg class="navbar-icon theme-icon-sun hidden" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
            </svg>
            <svg class="navbar-icon theme-icon-moon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
            </svg>
          </button>
        </div>
      </header>
    `;
  }

  /**
   * Render section header
   */
  renderSectionHeader(id, title, iconColor, iconPath) {
    return `
      <div class="section-header" style="display: flex; flex-direction: row; align-items: center; justify-content: space-between;">
        <div class="section-header-left" style="display: flex; flex-direction: row; align-items: center; gap: 8px;">
          <div class="section-icon">
            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
              ${iconPath}
            </svg>
          </div>
          <div class="section-title-wrap">
            <h2 class="section-title">${title}</h2>
            <p id="${id}-refresh-time" class="section-refresh-time"></p>
          </div>
        </div>
        <button class="refresh-btn" data-section="${id}" title="Refresh">
          <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
          <span class="refresh-text">Refresh</span>
        </button>
      </div>
    `;
  }

  /**
   * Render loading state
   */
  renderLoading() {
    return `
      <div class="flex items-center justify-center py-12">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-500"></div>
        <span class="ml-3 text-gray-400">Loading...</span>
      </div>
    `;
  }

  /**
   * Render error state
   */
  renderError(section, message) {
    return `
      <div class="text-center py-12">
        <svg class="w-12 h-12 text-red-400 mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <p class="text-gray-300 mb-4">${message}</p>
        <button class="btn btn-primary retry-btn" data-section="${section}">Try Again</button>
      </div>
    `;
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // DASHBOARD SECTION
  // ═══════════════════════════════════════════════════════════════════════════

  renderDashboardSection() {
    return `
      <section id="dashboard-section" class="dashboard-section">
        ${this.renderSectionHeader('dashboard', 'Dashboard', 'indigo',
      '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />'
    )}
        
        <div class="flex justify-end mb-4">
          <select id="period-selector" class="form-select w-auto">
            <option value="hour" ${this.state.dashboard.period === 'hour' ? 'selected' : ''}>Last Hour</option>
            <option value="day" ${this.state.dashboard.period === 'day' ? 'selected' : ''}>Last 24 Hours</option>
            <option value="week" ${this.state.dashboard.period === 'week' ? 'selected' : ''}>Last Week</option>
            <option value="month" ${this.state.dashboard.period === 'month' ? 'selected' : ''}>Last Month</option>
          </select>
        </div>
        
        <div id="dashboard-content">${this.renderLoading()}</div>
      </section>
    `;
  }

  async loadDashboard(forceRefresh = false) {
    const content = document.getElementById('dashboard-content');
    if (!content) return;

    const period = this.state.dashboard.period;

    try {
      const [metrics, health] = await Promise.all([
        this.fetchCached(`dashboard:metrics:${period}`, () => api.getServerMetrics(this.serverId, period), forceRefresh),
        this.fetchCached('dashboard:health', () => api.getHealthMetrics(this.serverId), forceRefresh)
      ]);

      content.innerHTML = `
        <!-- Server Health Card with Total Violations -->
        <div class="health-card">
          <div class="health-card-header">
            <div class="health-card-left">
              <div class="health-icon">
                <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <div>
                <h3 class="health-title">Server Health</h3>
                <p class="health-violations">Total Violations: <span class="health-violations-count">${metrics.violations_total?.toLocaleString() || '0'}</span></p>
              </div>
            </div>
            <span class="health-score ${this.getHealthClass(health.health_score)}">${health.health_score}%</span>
          </div>
          <div class="health-progress-track">
            <div class="health-progress-bar ${this.getHealthClass(health.health_score)}" style="width: ${health.health_score}%"></div>
          </div>
        </div>

        <!-- Violations Over Time Chart -->
        <div class="card">
          <h3 class="chart-title">Violations Over Time</h3>
          <div class="chart-container">
            <canvas id="violations-chart"></canvas>
          </div>
        </div>
      `;

      // Wait for DOM to update, then render charts
      setTimeout(() => {
        this.renderCharts(metrics);
      }, 100);

      this.lastRefreshTimes.set('dashboard', Date.now());
      this.updateRefreshTime('dashboard');

    } catch (error) {
      console.error('Failed to load dashboard:', error);
      content.innerHTML = this.renderError('dashboard', 'Failed to load dashboard data');
    }
  }

  getHealthClass(score) {
    if (score >= 80) return 'health-good';
    if (score >= 60) return 'health-warning';
    return 'health-danger';
  }

  renderCharts(metrics) {
    console.log('renderCharts called with:', metrics);

    // Destroy existing chart first
    if (this.charts.violations) {
      this.charts.violations.destroy();
      this.charts.violations = null;
    }

    // Check if Chart.js is available
    if (typeof Chart === 'undefined') {
      console.error('Chart.js is not loaded!');
      return;
    }
    console.log('Chart.js version:', Chart.version);

    // Violations over time chart
    const violationsCanvas = document.getElementById('violations-chart');
    console.log('violationsCanvas:', violationsCanvas);

    if (violationsCanvas) {
      const timeSeries = metrics.time_series || [];
      console.log('Time series data:', timeSeries);

      if (timeSeries.length > 0) {
        const labels = timeSeries.map(d => {
          const date = new Date(d.timestamp);
          return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
        });
        // API returns 'violations', not 'count'
        const data = timeSeries.map(d => d.violations || d.count || 0);
        console.log('Chart labels:', labels, 'data:', data);

        try {
          this.charts.violations = new Chart(violationsCanvas, {
            type: 'line',
            data: {
              labels,
              datasets: [{
                label: 'Violations',
                data,
                borderColor: 'rgb(239, 68, 68)',
                backgroundColor: 'rgba(239, 68, 68, 0.1)',
                fill: true,
                tension: 0.3,
                pointRadius: 3,
                pointHoverRadius: 5
              }]
            },
            options: {
              responsive: true,
              maintainAspectRatio: false,
              plugins: {
                legend: { display: false }
              },
              scales: {
                x: {
                  grid: { color: 'rgba(255,255,255,0.1)' },
                  ticks: { color: '#9CA3AF', maxTicksLimit: 8 }
                },
                y: {
                  beginAtZero: true,
                  grid: { color: 'rgba(255,255,255,0.1)' },
                  ticks: { color: '#9CA3AF', stepSize: 1 }
                }
              }
            }
          });
          console.log('Violations chart created:', this.charts.violations);
        } catch (err) {
          console.error('Failed to create violations chart:', err);
        }
      } else {
        // No data - show placeholder text in parent div
        violationsCanvas.parentElement.innerHTML = '<p class="text-gray-500 text-center pt-24">No violation data for this period</p>';
      }
    }
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // VIOLATIONS SECTION
  // ═══════════════════════════════════════════════════════════════════════════

  renderViolationsSection() {
    const { severity } = this.state.violations;

    return `
      <section id="violations-section" class="dashboard-section border-t border-gray-700 pt-12">
        ${this.renderSectionHeader('violations', 'Violations', 'red',
      '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />'
    )}
        
        <div class="flex flex-wrap gap-4 mb-6">
          <select id="severity-filter" class="form-select">
            <option value="">All Severities</option>
            <option value="high" ${severity === 'high' ? 'selected' : ''}>High</option>
            <option value="medium" ${severity === 'medium' ? 'selected' : ''}>Medium</option>
            <option value="low" ${severity === 'low' ? 'selected' : ''}>Low</option>
          </select>
        </div>
        
        <div id="violations-content">${this.renderLoading()}</div>
      </section>
    `;
  }

  async loadViolations(forceRefresh = false) {
    const content = document.getElementById('violations-content');
    if (!content) return;

    const { page, severity } = this.state.violations;
    const perPage = 5; // Show 5 violations per page
    const cacheKey = `violations:${page}:${severity}:${perPage}`;

    try {
      const response = await this.fetchCached(cacheKey,
        () => api.getViolations(this.serverId, { page, severity, per_page: perPage }),
        forceRefresh
      );

      if (!response.violations?.length) {
        content.innerHTML = `
          <div class="text-center py-12 text-gray-400">
            <svg class="w-16 h-16 mx-auto mb-4 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <p class="text-lg">No violations found</p>
            <p class="text-sm mt-2">Adjust filters or check back later</p>
          </div>
        `;
        return;
      }

      const totalPages = Math.ceil(response.total / perPage);

      content.innerHTML = `
        <div class="space-y-4">
          ${response.violations.map(v => this.renderViolationCard(v)).join('')}
        </div>
        ${this.renderPagination(response.page, totalPages, 'violations')}
      `;

      this.lastRefreshTimes.set('violations', Date.now());
      this.updateRefreshTime('violations');

    } catch (error) {
      console.error('Failed to load violations:', error);
      content.innerHTML = this.renderError('violations', 'Failed to load violations');
    }
  }

  renderViolationCard(v) {
    const severityColors = {
      high: 'severity-high',
      medium: 'severity-medium',
      low: 'severity-low'
    };
    const severityClass = severityColors[v.severity?.toLowerCase()] || 'severity-low';

    const avatar = v.avatar
      ? `<img src="https://cdn.discordapp.com/avatars/${v.user_id}/${v.avatar}.png" class="violation-avatar" alt="">`
      : `<div class="violation-avatar violation-avatar-placeholder">${(v.username || 'U')[0].toUpperCase()}</div>`;

    return `
      <div class="violation-card ${severityClass}">
        <div class="violation-header">
          <div class="violation-user">
            ${avatar}
            <div class="violation-user-info">
              <span class="violation-username">${this.escapeHtml(v.username || 'Unknown')}</span>
              <span class="violation-userid">ID: ${v.user_id}</span>
            </div>
          </div>
          <div class="violation-meta">
            <span class="violation-time">${new Date(v.timestamp).toLocaleString()}</span>
            <span class="violation-severity-badge ${severityClass}">${v.severity}</span>
          </div>
        </div>
        <p class="violation-reason">${this.escapeHtml(v.reason || 'No reason provided')}</p>
        <div class="violation-details">
          <span>Detection: ${v.detection_type || 'Unknown'}</span>
          <span>Action: ${v.action_taken || 'None'}</span>
        </div>
      </div>
    `;
  }

  renderPagination(current, total, section) {
    if (total <= 1) return '';

    return `
      <div class="pagination">
        <button class="pagination-btn" data-section="${section}" data-page="${current - 1}" ${current <= 1 ? 'disabled' : ''}>
          <svg fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" /></svg>
          <span>Prev</span>
        </button>
        <span class="pagination-info">${current} / ${total}</span>
        <button class="pagination-btn" data-section="${section}" data-page="${current + 1}" ${current >= total ? 'disabled' : ''}>
          <span>Next</span>
          <svg fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" /></svg>
        </button>
      </div>
    `;
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // RULES SECTION
  // ═══════════════════════════════════════════════════════════════════════════

  renderRulesSection() {
    return `
      <section id="rules-section" class="dashboard-section border-t border-gray-700 pt-12">
        ${this.renderSectionHeader('rules', 'Rules', 'yellow',
      '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />'
    )}
        <div id="rules-content">${this.renderLoading()}</div>
      </section>
    `;
  }

  async loadRules(forceRefresh = false) {
    const content = document.getElementById('rules-content');
    if (!content) return;

    try {
      const response = await this.fetchCached('rules', () => api.getRules(this.serverId), forceRefresh);
      const isEditing = this.state.rules.editing;

      if (!response?.has_rules || !response.rules) {
        content.innerHTML = `
          <div class="card">
            <div class="rules-header">
              <h3 class="rules-title">Server Rules</h3>
              <button id="add-rules-btn" class="rules-save-btn">Add Rules</button>
            </div>
            <div class="rules-empty">
              <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
              </svg>
              <p>No custom rules configured</p>
              <p class="rules-empty-hint">Add rules to help the AI understand what's allowed</p>
            </div>
          </div>
        `;
        this.setupRulesListeners('');
        return;
      }

      if (isEditing) {
        content.innerHTML = `
          <div class="card">
            <div class="rules-header">
              <h3 class="rules-title">Edit Server Rules</h3>
              <div class="rules-actions">
                <button id="cancel-rules-btn" class="rules-cancel-btn">Cancel</button>
                <button id="save-rules-btn" class="rules-save-btn">Save</button>
              </div>
            </div>
            <p class="rules-description">Define your server's rules. The AI uses these to understand acceptable behavior.</p>
            <textarea id="rules-editor" class="rules-editor">${this.escapeHtml(response.rules)}</textarea>
          </div>
        `;
        this.setupRulesListeners(response.rules);
      } else {
        content.innerHTML = `
          <div class="card">
            <div class="rules-header">
              <div>
                <h3 class="rules-title">Server Rules</h3>
                ${response.updated_at ? `<span class="rules-updated">Updated ${new Date(response.updated_at).toLocaleString()}</span>` : ''}
              </div>
              <button id="edit-rules-btn" class="rules-edit-btn">Edit</button>
            </div>
            <pre class="rules-content">${this.escapeHtml(response.rules)}</pre>
          </div>
        `;
        this.setupRulesListeners(response.rules);
      }

      this.lastRefreshTimes.set('rules', Date.now());
      this.updateRefreshTime('rules');

    } catch (error) {
      console.error('Failed to load rules:', error);
      content.innerHTML = this.renderError('rules', 'Failed to load rules');
    }
  }

  setupRulesListeners(existingRules) {
    document.getElementById('edit-rules-btn')?.addEventListener('click', () => {
      this.state.rules.editing = true;
      this.loadRules();
    });

    document.getElementById('add-rules-btn')?.addEventListener('click', () => {
      this.state.rules.editing = true;
      this.clearCache('rules');
      // Render empty editor
      const content = document.getElementById('rules-content');
      content.innerHTML = `
        <div class="card">
          <div class="flex justify-between items-center mb-4">
            <h3 class="text-lg font-semibold text-gray-100">Add Server Rules</h3>
            <div class="flex gap-2">
              <button id="cancel-rules-btn" class="btn btn-secondary btn-sm">Cancel</button>
              <button id="save-rules-btn" class="btn btn-primary btn-sm">Save</button>
            </div>
          </div>
          <p class="text-gray-400 text-sm mb-3">Define your server's rules. The AI uses these to understand acceptable behavior.</p>
          <textarea id="rules-editor" class="form-textarea w-full h-64 font-mono text-sm" placeholder="1. Be respectful to all members
2. No spam or self-promotion
3. No NSFW content
4. Keep discussions on-topic"></textarea>
        </div>
      `;
      this.setupRulesListeners('');
    });

    document.getElementById('cancel-rules-btn')?.addEventListener('click', () => {
      this.state.rules.editing = false;
      this.clearCache('rules');
      this.loadRules();
    });

    document.getElementById('save-rules-btn')?.addEventListener('click', async () => {
      const rules = document.getElementById('rules-editor')?.value?.trim();
      if (!rules) {
        this.showToast('Please enter some rules', 'error');
        return;
      }

      const btn = document.getElementById('save-rules-btn');
      btn.disabled = true;
      btn.textContent = 'Saving...';

      try {
        await api.updateRules(this.serverId, rules);
        this.state.rules.editing = false;
        this.clearCache('rules');
        this.showToast('Rules saved!', 'success');
        this.loadRules();
      } catch (error) {
        console.error('Failed to save rules:', error);
        this.showToast('Failed to save rules', 'error');
        btn.disabled = false;
        btn.textContent = 'Save';
      }
    });
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // CONFIG SECTION
  // ═══════════════════════════════════════════════════════════════════════════

  renderConfigSection() {
    return `
      <section id="config-section" class="dashboard-section border-t border-gray-700 pt-12">
        ${this.renderSectionHeader('config', 'Configuration', 'green',
      '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" /><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />'
    )}
        <div id="config-content">${this.renderLoading()}</div>
      </section>
    `;
  }

  async loadConfig(forceRefresh = false) {
    const content = document.getElementById('config-content');
    if (!content) return;

    try {
      const config = await this.fetchCached('config', () => api.getConfig(this.serverId), forceRefresh);

      content.innerHTML = `
        <div class="card">
          <h3 class="text-lg font-semibold text-gray-100 mb-6">Bot Settings</h3>
          
          <div class="space-y-6">
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">Severity Threshold</label>
              <p class="text-xs text-gray-400 mb-2">Minimum confidence (0.0 - 1.0) for AI to flag a message. Lower = more catches but more false positives.</p>
              <input type="number" id="cfg-threshold" class="form-input w-full" min="0" max="1" step="0.1" value="${config.severity_threshold ?? 0.5}">
            </div>
            
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">Buffer Timeout (seconds)</label>
              <p class="text-xs text-gray-400 mb-2">How long to wait before sending buffered messages to AI.</p>
              <input type="number" id="cfg-timeout" class="form-input w-full" min="1" max="300" value="${config.buffer_timeout_secs ?? 30}">
            </div>
            
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">Buffer Threshold</label>
              <p class="text-xs text-gray-400 mb-2">Messages to collect before batch analysis.</p>
              <input type="number" id="cfg-buffer" class="form-input w-full" min="1" max="100" value="${config.buffer_threshold ?? 10}">
            </div>
            
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">Moderator Role ID</label>
              <p class="text-xs text-gray-400 mb-2">Discord role to ping for high-severity violations. Leave empty to disable.</p>
              <input type="text" id="cfg-modrole" class="form-input w-full" placeholder="e.g. 123456789012345678" value="${config.mod_role_id || ''}">
            </div>
            
            <button id="save-config-btn" class="btn btn-primary w-full">Save Configuration</button>
          </div>
        </div>
      `;

      document.getElementById('save-config-btn')?.addEventListener('click', () => this.saveConfig());

      this.lastRefreshTimes.set('config', Date.now());
      this.updateRefreshTime('config');

    } catch (error) {
      console.error('Failed to load config:', error);
      content.innerHTML = this.renderError('config', 'Failed to load configuration');
    }
  }

  async saveConfig() {
    const btn = document.getElementById('save-config-btn');
    btn.disabled = true;
    btn.textContent = 'Saving...';

    try {
      await api.updateConfig(this.serverId, {
        severity_threshold: parseFloat(document.getElementById('cfg-threshold')?.value),
        buffer_timeout_secs: parseInt(document.getElementById('cfg-timeout')?.value),
        buffer_threshold: parseInt(document.getElementById('cfg-buffer')?.value),
        mod_role_id: document.getElementById('cfg-modrole')?.value || null
      });

      this.clearCache('config');
      this.showToast('Configuration saved!', 'success');
    } catch (error) {
      console.error('Failed to save config:', error);
      this.showToast('Failed to save configuration', 'error');
    } finally {
      btn.disabled = false;
      btn.textContent = 'Save Configuration';
    }
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // EVENT HANDLING & UTILITIES
  // ═══════════════════════════════════════════════════════════════════════════

  setupEventListeners() {
    // Refresh buttons
    document.querySelectorAll('.refresh-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        const section = e.currentTarget.dataset.section;
        this.refreshSection(section);
      });
    });

    // Retry buttons (delegated)
    document.addEventListener('click', (e) => {
      const retryBtn = e.target.closest('.retry-btn');
      if (retryBtn) {
        this.refreshSection(retryBtn.dataset.section);
      }
    });

    // Period selector
    document.getElementById('period-selector')?.addEventListener('change', (e) => {
      this.state.dashboard.period = e.target.value;
      this.saveState();
      this.clearCache('dashboard');
      this.loadDashboard(true);
    });

    // Violation filters
    document.getElementById('severity-filter')?.addEventListener('change', (e) => {
      this.state.violations.severity = e.target.value;
      this.state.violations.page = 1;
      this.saveState();
      this.clearCache('violations');
      this.loadViolations(true);
    });

    // Pagination (delegated)
    document.addEventListener('click', (e) => {
      const pageBtn = e.target.closest('.pagination-btn');
      if (pageBtn && !pageBtn.disabled) {
        const section = pageBtn.dataset.section;
        const page = parseInt(pageBtn.dataset.page);
        this.state[section].page = page;
        this.saveState();
        this.clearCache(section);
        this.loadSection(section, true);
      }
    });

    // Smooth scroll for nav links
    document.querySelectorAll('.nav-link').forEach(link => {
      link.addEventListener('click', (e) => {
        e.preventDefault();
        const section = e.currentTarget.dataset.section;
        document.getElementById(`${section}-section`)?.scrollIntoView({ behavior: 'smooth' });
      });
    });
  }

  loadAllSections() {
    this.loadDashboard();
    this.loadViolations();
    this.loadRules();
    this.loadConfig();
  }

  loadSection(section, forceRefresh = false) {
    switch (section) {
      case 'dashboard': this.loadDashboard(forceRefresh); break;
      case 'violations': this.loadViolations(forceRefresh); break;
      case 'rules': this.loadRules(forceRefresh); break;
      case 'config': this.loadConfig(forceRefresh); break;
    }
  }

  refreshSection(section) {
    const btn = document.querySelector(`.refresh-btn[data-section="${section}"]`);
    if (btn) btn.classList.add('animate-spin');

    this.clearCache(section);
    this.loadSection(section, true);

    setTimeout(() => btn?.classList.remove('animate-spin'), 500);
  }

  updateRefreshTime(section) {
    const el = document.getElementById(`${section}-refresh-time`);
    const time = this.lastRefreshTimes.get(section);
    if (el && time) {
      el.textContent = `Updated ${this.formatTimeAgo(time)}`;
    }
  }

  updateRefreshTimes() {
    ['dashboard', 'violations', 'rules', 'config'].forEach(s => this.updateRefreshTime(s));
  }

  destroy() {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
    }
    Object.values(this.charts).forEach(c => c?.destroy());
    this.charts = {};
    this.cache.clear();
  }
}

// Export for router
export function renderSinglePageDashboard() {
  const serverId = sessionStorage.getItem('selectedServerId');
  const serverName = sessionStorage.getItem('selectedServerName');

  if (!serverId) {
    window.location.hash = '#/servers';
    return;
  }

  // Cleanup previous instance
  if (window.currentDashboard) {
    window.currentDashboard.destroy();
  }

  const dashboard = new SinglePageDashboard(serverId, serverName);
  dashboard.render();
  window.currentDashboard = dashboard;
}
