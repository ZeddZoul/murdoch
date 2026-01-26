/**
 * Main Application Entry Point
 * Handles page rendering and application initialization
 */

import { auth } from './auth.js';
import { api } from './api.js';
import { router } from './router.js';

// Global state
let currentServer = null;
let autoRefreshInterval = null;
let clientId = null;

// Fetch client config on load
async function loadClientConfig() {
  try {
    const response = await fetch('/api/config');
    const config = await response.json();
    clientId = config.client_id;
  } catch (error) {
    console.error('Failed to load client config:', error);
    clientId = 'YOUR_CLIENT_ID'; // Fallback
  }
}

/**
 * Get color for health score
 */
function getHealthScoreColor(score) {
  if (score >= 90) return '#10b981'; // green
  if (score >= 70) return '#f59e0b'; // yellow
  if (score >= 50) return '#f97316'; // orange
  return '#ef4444'; // red
}

/**
 * Render trend indicator with arrow
 */
function renderTrendIndicator(changePercent) {
  if (changePercent === 0) {
    return '<div class="text-xs text-gray-500 mt-1">→ No change</div>';
  }

  const isPositive = changePercent > 0;
  const color = isPositive ? 'text-red-400' : 'text-green-400';
  const arrow = isPositive ? '↑' : '↓';

  return `<div class="text-xs ${color} mt-1">${arrow} ${Math.abs(changePercent).toFixed(1)}% from previous period</div>`;
}

/**
 * Render the navigation bar
 */
function renderNavbar(serverName) {
  return `
    <nav class="fixed top-0 left-0 right-0 bg-gray-800 border-b border-gray-700 z-50">
      <div class="max-w-7xl mx-auto px-4">
        <div class="flex items-center justify-between h-16">
          <div class="flex items-center gap-6">
            <h1 class="text-xl font-bold text-indigo-400">Murdoch</h1>
            <span class="text-gray-400">|</span>
            <span class="text-gray-300">${serverName || 'Dashboard'}</span>
          </div>
          
          <div class="flex items-center gap-4">
            <a href="#/dashboard" class="text-gray-300 hover:text-white px-3 py-2 rounded-md text-sm font-medium">Dashboard</a>
            <a href="#/violations" class="text-gray-300 hover:text-white px-3 py-2 rounded-md text-sm font-medium">Violations</a>
            <a href="#/rules" class="text-gray-300 hover:text-white px-3 py-2 rounded-md text-sm font-medium">Rules</a>
            <a href="#/config" class="text-gray-300 hover:text-white px-3 py-2 rounded-md text-sm font-medium">Config</a>
            <a href="#/warnings" class="text-gray-300 hover:text-white px-3 py-2 rounded-md text-sm font-medium">Warnings</a>
            <button onclick="window.router.navigate('/servers')" class="btn btn-secondary btn-sm">
              Change Server
            </button>
          </div>
        </div>
      </div>
    </nav>
  `;
}

/**
 * Render the login page
 */
function renderLoginPage() {
  const app = document.getElementById('app');
  app.innerHTML = `
    <div class="flex items-center justify-center min-h-screen bg-gradient-to-br from-gray-900 via-gray-800 to-gray-900">
      <div class="card max-w-md w-full mx-4 text-center fade-in">
        <div class="mb-8">
          <h1 class="text-4xl font-bold text-indigo-400 mb-2">Murdoch</h1>
          <p class="text-gray-400">Discord Moderation Dashboard</p>
        </div>
        
        <div class="mb-8">
          <svg class="w-24 h-24 mx-auto text-indigo-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
          </svg>
        </div>

        <p class="text-gray-300 mb-6">
          Sign in with your Discord account to access the moderation dashboard for servers you manage.
        </p>

        <button 
          onclick="window.auth.login()" 
          class="btn btn-primary w-full py-3 text-lg flex items-center justify-center gap-3"
        >
          <svg class="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
            <path d="M20.317 4.37a19.791 19.791 0 00-4.885-1.515.074.074 0 00-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 00-5.487 0 12.64 12.64 0 00-.617-1.25.077.077 0 00-.079-.037A19.736 19.736 0 003.677 4.37a.07.07 0 00-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 00.031.057 19.9 19.9 0 005.993 3.03.078.078 0 00.084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 00-.041-.106 13.107 13.107 0 01-1.872-.892.077.077 0 01-.008-.128 10.2 10.2 0 00.372-.292.074.074 0 01.077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 01.078.01c.12.098.246.198.373.292a.077.077 0 01-.006.127 12.299 12.299 0 01-1.873.892.077.077 0 00-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 00.084.028 19.839 19.839 0 006.002-3.03.077.077 0 00.032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 00-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/>
          </svg>
          Sign in with Discord
        </button>

        <p class="text-gray-500 text-sm mt-6">
          You'll be redirected to Discord to authorize access
        </p>
      </div>
    </div>
  `;
}

// Initialize the application
async function init() {
  // Make auth available globally for inline handlers
  window.auth = auth;
  window.api = api;

  // Load client config
  await loadClientConfig();

  // Set up router
  setupRoutes();

  // Initialize router
  router.init();
}

/**
 * Set up all application routes
 */
function setupRoutes() {
  // Public routes
  router.register('/', renderLoginPage, { requiresAuth: false });
  router.register('/login', renderLoginPage, { requiresAuth: false });

  // Protected routes
  router.register('/servers', renderServerSelector, { requiresAuth: true });
  router.register('/dashboard', renderDashboard, { requiresAuth: true });
  router.register('/violations', renderViolationsPage, { requiresAuth: true });
  router.register('/rules', renderRulesPage, { requiresAuth: true });
  router.register('/config', renderConfigPage, { requiresAuth: true });
  router.register('/warnings', renderWarningsPage, { requiresAuth: true });
  router.register('/rule-effectiveness', renderRuleEffectivenessPage, { requiresAuth: true });
  router.register('/temporal', renderTemporalAnalyticsPage, { requiresAuth: true });

  // 404 handler
  router.notFound(() => {
    const app = document.getElementById('app');
    app.innerHTML = `
      <div class="flex items-center justify-center min-h-screen">
        <div class="text-center">
          <h1 class="text-6xl font-bold text-gray-600 mb-4">404</h1>
          <p class="text-gray-400 mb-6">Page not found</p>
          <button onclick="window.router.navigate('/servers')" class="btn btn-primary">
            Go to Dashboard
          </button>
        </div>
      </div>
    `;
  });

  // Global navigation guard
  router.beforeEach(async (to, from) => {
    // Clear any auto-refresh intervals when navigating away
    if (autoRefreshInterval) {
      clearInterval(autoRefreshInterval);
      autoRefreshInterval = null;
    }

    // If going to login page and already authenticated, redirect to servers
    if ((to.path === '/' || to.path === '/login') && auth.isLoggedIn()) {
      return '/servers';
    }

    return true;
  });
}

/**
 * Render the server selector page
 */
async function renderServerSelector() {
  const app = document.getElementById('app');

  // Show loading state
  app.innerHTML = `
    <div class="flex items-center justify-center min-h-screen">
      <div class="text-center">
        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-500 mx-auto"></div>
        <p class="mt-4 text-gray-400">Loading servers...</p>
      </div>
    </div>
  `;

  try {
    const response = await api.getServers();
    const servers = response.servers || [];

    if (servers.length === 0) {
      // No servers available
      app.innerHTML = `
        <div class="flex items-center justify-center min-h-screen">
          <div class="card max-w-2xl mx-4 text-center fade-in">
            <svg class="w-16 h-16 mx-auto text-gray-600 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
            </svg>
            <h2 class="text-2xl font-bold text-gray-300 mb-4">No Servers Found</h2>
            <p class="text-gray-400 mb-6">
              You don't have administrator permissions on any servers, or Murdoch hasn't been added to your servers yet.
            </p>
            <a 
              href="https://discord.com/api/oauth2/authorize?client_id=${clientId || 'YOUR_CLIENT_ID'}&permissions=8&scope=bot" 
              target="_blank"
              class="btn btn-primary inline-block"
            >
              Invite Murdoch to Your Server
            </a>
            <button onclick="window.auth.logout()" class="btn btn-secondary ml-4">
              Logout
            </button>
          </div>
        </div>
      `;
      return;
    }

    // Render server list
    const serverCards = servers.map(server => {
      const iconUrl = server.icon
        ? `https://cdn.discordapp.com/icons/${server.id}/${server.icon}.png`
        : 'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="%236366f1"%3E%3Cpath d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"/%3E%3C/svg%3E';

      const botStatus = server.bot_present
        ? '<span class="text-green-400 text-sm">● Bot Active</span>'
        : '<span class="text-yellow-400 text-sm">● Bot Not Present</span>';

      const inviteButton = !server.bot_present
        ? `<a href="https://discord.com/api/oauth2/authorize?client_id=${clientId || 'YOUR_CLIENT_ID'}&permissions=8&scope=bot&guild_id=${server.id}" target="_blank" class="btn btn-secondary btn-sm mt-2 w-full">Invite Bot</a>`
        : '';

      return `
        <div class="card hover:bg-gray-800 transition-colors cursor-pointer" onclick="selectServer('${server.id}', '${server.name}')">
          <div class="flex items-center gap-4">
            <img src="${iconUrl}" alt="${server.name}" class="w-16 h-16 rounded-lg" onerror="this.src='data:image/svg+xml,%3Csvg xmlns=\\'http://www.w3.org/2000/svg\\' viewBox=\\'0 0 24 24\\' fill=\\'%236366f1\\'%3E%3Cpath d=\\'M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10\\'/%3E%3C/svg%3E'">
            <div class="flex-1">
              <h3 class="text-lg font-semibold text-gray-100">${server.name}</h3>
              ${botStatus}
            </div>
            <svg class="w-6 h-6 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
            </svg>
          </div>
          ${inviteButton}
        </div>
      `;
    }).join('');

    app.innerHTML = `
      <div class="min-h-screen bg-gray-900 py-8">
        <div class="max-w-4xl mx-auto px-4">
          <div class="flex justify-between items-center mb-8">
            <div>
              <h1 class="text-3xl font-bold text-gray-100 mb-2">Select a Server</h1>
              <p class="text-gray-400">Choose a server to manage its moderation settings</p>
            </div>
            <button onclick="window.auth.logout()" class="btn btn-secondary">
              Logout
            </button>
          </div>

          <div class="grid gap-4 fade-in">
            ${serverCards}
          </div>
        </div>
      </div>
    `;

    // Check if we have a previously selected server in session storage
    const lastServerId = sessionStorage.getItem('selectedServerId');
    if (lastServerId && servers.find(s => s.id === lastServerId)) {
      const lastServerName = servers.find(s => s.id === lastServerId).name;
      selectServer(lastServerId, lastServerName);
    }

  } catch (error) {
    console.error('Failed to load servers:', error);
    app.innerHTML = `
      <div class="flex items-center justify-center min-h-screen">
        <div class="card max-w-md mx-4 text-center">
          <div class="error-message">
            <p class="font-semibold">Failed to load servers</p>
            <p class="text-sm mt-2">${error.message}</p>
          </div>
          <button onclick="window.location.reload()" class="btn btn-primary mt-4">
            Retry
          </button>
        </div>
      </div>
    `;
  }
}

/**
 * Select a server and navigate to dashboard
 */
function selectServer(serverId, serverName) {
  currentServer = { id: serverId, name: serverName };
  sessionStorage.setItem('selectedServerId', serverId);
  sessionStorage.setItem('selectedServerName', serverName);
  router.navigate('/dashboard');
}

// Make selectServer available globally
window.selectServer = selectServer;

/**
 * Render the main dashboard with charts
 */
async function renderDashboard() {
  const serverId = sessionStorage.getItem('selectedServerId');
  const serverName = sessionStorage.getItem('selectedServerName');

  if (!serverId) {
    router.navigate('/servers');
    return;
  }

  const app = document.getElementById('app');

  // Render layout with loading state
  app.innerHTML = `
    ${renderNavbar(serverName)}
    <div class="min-h-screen bg-gray-900 pt-16">
      <div class="max-w-7xl mx-auto px-4 py-8">
        <div class="flex justify-between items-center mb-6">
          <h1 class="text-3xl font-bold text-gray-100">Dashboard</h1>
          <div class="flex gap-4 items-center">
            <select id="period-selector" class="form-select w-auto">
              <option value="hour">Last Hour</option>
              <option value="day" selected>Last 24 Hours</option>
              <option value="week">Last Week</option>
              <option value="month">Last Month</option>
            </select>
            <button id="refresh-btn" class="btn btn-secondary">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            </button>
          </div>
        </div>

        <div id="dashboard-content">
          <div class="flex items-center justify-center py-12">
            <div class="text-center">
              <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-500 mx-auto"></div>
              <p class="mt-4 text-gray-400">Loading metrics...</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;

  // Set up event listeners
  document.getElementById('period-selector').addEventListener('change', (e) => {
    loadDashboardData(serverId, e.target.value);
  });

  document.getElementById('refresh-btn').addEventListener('click', () => {
    const period = document.getElementById('period-selector').value;
    loadDashboardData(serverId, period);
  });

  // Load initial data
  await loadDashboardData(serverId, 'day');

  // Set up auto-refresh every 60 seconds
  autoRefreshInterval = setInterval(() => {
    const period = document.getElementById('period-selector').value;
    loadDashboardData(serverId, period);
  }, 60000);
}

/**
 * Load dashboard data and render charts
 */
async function loadDashboardData(serverId, period) {
  const content = document.getElementById('dashboard-content');

  try {
    const [metrics, healthMetrics, topOffenders] = await Promise.all([
      api.getServerMetrics(serverId, period),
      api.getHealthMetrics(serverId),
      api.getTopOffenders(serverId, period)
    ]);

    // Render metrics cards and charts
    content.innerHTML = `
      <!-- Metrics Cards -->
      <div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8">
        <div class="card">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-gray-400 text-sm mb-1">Total Messages</p>
              <p class="text-3xl font-bold text-gray-100">${metrics.messages_processed.toLocaleString()}</p>
            </div>
            <div class="bg-indigo-500 bg-opacity-20 p-3 rounded-lg">
              <svg class="w-8 h-8 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
              </svg>
            </div>
          </div>
        </div>

        <div class="card">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-gray-400 text-sm mb-1">Total Violations</p>
              <p class="text-3xl font-bold text-gray-100">${metrics.violations_total.toLocaleString()}</p>
            </div>
            <div class="bg-red-500 bg-opacity-20 p-3 rounded-lg">
              <svg class="w-8 h-8 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>
          </div>
        </div>

        <div class="card">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-gray-400 text-sm mb-1">Avg Response Time</p>
              <p class="text-3xl font-bold text-gray-100">${metrics.avg_response_time_ms}ms</p>
            </div>
            <div class="bg-green-500 bg-opacity-20 p-3 rounded-lg">
              <svg class="w-8 h-8 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
            </div>
          </div>
        </div>
      </div>

      <!-- Health Metrics Widget -->
      <div class="card mb-8">
        <h3 class="text-lg font-semibold text-gray-100 mb-4">Server Health</h3>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <!-- Health Score -->
          <div class="text-center">
            <div class="relative inline-block">
              <svg class="w-32 h-32 transform -rotate-90">
                <circle cx="64" cy="64" r="56" stroke="#374151" stroke-width="8" fill="none" />
                <circle 
                  cx="64" cy="64" r="56" 
                  stroke="${getHealthScoreColor(healthMetrics.health_score)}" 
                  stroke-width="8" 
                  fill="none"
                  stroke-dasharray="${(healthMetrics.health_score / 100) * 351.86} 351.86"
                  stroke-linecap="round"
                />
              </svg>
              <div class="absolute inset-0 flex items-center justify-center">
                <div class="text-center">
                  <div class="text-3xl font-bold" style="color: ${getHealthScoreColor(healthMetrics.health_score)}">${healthMetrics.health_score}</div>
                  <div class="text-xs text-gray-400">Score</div>
                </div>
              </div>
            </div>
            ${healthMetrics.health_score < 70 ? '<div class="mt-2 text-yellow-400 text-sm">⚠️ Needs Attention</div>' : ''}
          </div>

          <!-- Violation Rate -->
          <div>
            <p class="text-gray-400 text-sm mb-2">Violation Rate</p>
            <p class="text-2xl font-bold text-gray-100">${healthMetrics.violation_rate.toFixed(2)}</p>
            <p class="text-xs text-gray-500">per 1000 messages</p>
            ${renderTrendIndicator(healthMetrics.trends.violations_change_pct)}
          </div>

          <!-- Action Distribution -->
          <div class="col-span-2">
            <p class="text-gray-400 text-sm mb-3">Action Distribution</p>
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <span class="text-sm text-gray-300">Warnings</span>
                <div class="flex items-center gap-2">
                  <div class="w-32 bg-gray-700 rounded-full h-2">
                    <div class="bg-yellow-500 h-2 rounded-full" style="width: ${healthMetrics.action_distribution.warnings_pct}%"></div>
                  </div>
                  <span class="text-sm text-gray-400 w-12 text-right">${healthMetrics.action_distribution.warnings_pct.toFixed(1)}%</span>
                </div>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-sm text-gray-300">Timeouts</span>
                <div class="flex items-center gap-2">
                  <div class="w-32 bg-gray-700 rounded-full h-2">
                    <div class="bg-orange-500 h-2 rounded-full" style="width: ${healthMetrics.action_distribution.timeouts_pct}%"></div>
                  </div>
                  <span class="text-sm text-gray-400 w-12 text-right">${healthMetrics.action_distribution.timeouts_pct.toFixed(1)}%</span>
                </div>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-sm text-gray-300">Kicks</span>
                <div class="flex items-center gap-2">
                  <div class="w-32 bg-gray-700 rounded-full h-2">
                    <div class="bg-red-500 h-2 rounded-full" style="width: ${healthMetrics.action_distribution.kicks_pct}%"></div>
                  </div>
                  <span class="text-sm text-gray-400 w-12 text-right">${healthMetrics.action_distribution.kicks_pct.toFixed(1)}%</span>
                </div>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-sm text-gray-300">Bans</span>
                <div class="flex items-center gap-2">
                  <div class="w-32 bg-gray-700 rounded-full h-2">
                    <div class="bg-red-700 h-2 rounded-full" style="width: ${healthMetrics.action_distribution.bans_pct}%"></div>
                  </div>
                  <span class="text-sm text-gray-400 w-12 text-right">${healthMetrics.action_distribution.bans_pct.toFixed(1)}%</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Charts -->
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
        <!-- Messages Over Time -->
        <div class="card">
          <h3 class="text-lg font-semibold text-gray-100 mb-4">Messages Over Time</h3>
          <div class="chart-container">
            <canvas id="messages-chart"></canvas>
          </div>
        </div>

        <!-- Violations by Type -->
        <div class="card">
          <h3 class="text-lg font-semibold text-gray-100 mb-4">Violations by Detection Type</h3>
          <div class="chart-container">
            <canvas id="type-chart"></canvas>
          </div>
        </div>
      </div>

      <!-- Top Offenders Widget -->
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
        <div class="card">
          <div class="flex justify-between items-center mb-4">
            <h3 class="text-lg font-semibold text-gray-100">Top Offenders</h3>
            <a href="#/offenders" class="text-indigo-400 hover:text-indigo-300 text-sm">View All →</a>
          </div>
          <div class="overflow-x-auto">
            <table class="table">
              <thead>
                <tr>
                  <th>User</th>
                  <th>Violations</th>
                  <th>Warning Level</th>
                  <th>Last Violation</th>
                </tr>
              </thead>
              <tbody>
                ${topOffenders.top_users.slice(0, 10).map(user => `
                  <tr class="cursor-pointer hover:bg-gray-700" onclick="window.router.navigateWithQuery('/violations', { user_id: '${user.user_id}' })">
                    <td class="font-medium">${user.username || user.user_id}</td>
                    <td><span class="badge badge-high">${user.violation_count}</span></td>
                    <td>
                      <div class="flex items-center gap-1">
                        ${Array(user.warning_level).fill('⚠️').join('')}
                        ${user.warning_level === 0 ? '<span class="text-gray-500">None</span>' : ''}
                      </div>
                    </td>
                    <td class="text-sm text-gray-400">${new Date(user.last_violation).toLocaleDateString()}</td>
                  </tr>
                `).join('')}
              </tbody>
            </table>
          </div>
          <div class="mt-4 pt-4 border-t border-gray-700">
            <p class="text-sm text-gray-400">
              <span class="font-semibold text-gray-300">${topOffenders.moderated_users_pct.toFixed(1)}%</span> of users have been moderated
            </p>
          </div>
        </div>

        <!-- Violation Distribution Chart -->
        <div class="card">
          <h3 class="text-lg font-semibold text-gray-100 mb-4">Violation Distribution</h3>
          <div class="chart-container">
            <canvas id="distribution-chart"></canvas>
          </div>
        </div>
      </div>

      <!-- Violations by Severity -->
      <div class="card">
        <h3 class="text-lg font-semibold text-gray-100 mb-4">Violations by Severity</h3>
        <div class="chart-container">
          <canvas id="severity-chart"></canvas>
        </div>
      </div>
    `;

    // Render charts
    renderMessagesChart(metrics.time_series);
    renderTypeChart(metrics.violations_by_type);
    renderSeverityChart(metrics.violations_by_severity);
    renderDistributionChart(topOffenders.violation_distribution);

  } catch (error) {
    console.error('Failed to load dashboard data:', error);
    content.innerHTML = `
      <div class="error-message">
        <p class="font-semibold">Failed to load dashboard data</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
  }
}

/**
 * Render messages over time line chart
 */
function renderMessagesChart(timeSeries) {
  const ctx = document.getElementById('messages-chart');
  if (!ctx) return;

  // Handle missing or empty time series data
  if (!timeSeries || !Array.isArray(timeSeries) || timeSeries.length === 0) {
    ctx.parentElement.innerHTML = '<p class="text-gray-500 text-center py-8">No time series data available</p>';
    return;
  }

  new Chart(ctx, {
    type: 'line',
    data: {
      labels: timeSeries.map(point => new Date(point.timestamp).toLocaleTimeString()),
      datasets: [
        {
          label: 'Messages',
          data: timeSeries.map(point => point.messages),
          borderColor: '#6366f1',
          backgroundColor: 'rgba(99, 102, 241, 0.1)',
          tension: 0.4,
        },
        {
          label: 'Violations',
          data: timeSeries.map(point => point.violations),
          borderColor: '#ef4444',
          backgroundColor: 'rgba(239, 68, 68, 0.1)',
          tension: 0.4,
        }
      ]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          labels: { color: '#d1d5db' }
        },
        tooltip: {
          mode: 'index',
          intersect: false,
        }
      },
      scales: {
        x: {
          ticks: { color: '#9ca3af' },
          grid: { color: '#374151' }
        },
        y: {
          ticks: { color: '#9ca3af' },
          grid: { color: '#374151' }
        }
      }
    }
  });
}

/**
 * Render violations by type pie chart
 */
function renderTypeChart(violationsByType) {
  const ctx = document.getElementById('type-chart');
  if (!ctx) return;

  // Handle missing or empty data
  if (!violationsByType || typeof violationsByType !== 'object' || Object.keys(violationsByType).length === 0) {
    ctx.parentElement.innerHTML = '<p class="text-gray-500 text-center py-8">No violation type data available</p>';
    return;
  }

  const labels = Object.keys(violationsByType);
  const data = Object.values(violationsByType);

  new Chart(ctx, {
    type: 'pie',
    data: {
      labels: labels,
      datasets: [{
        data: data,
        backgroundColor: [
          '#6366f1',
          '#8b5cf6',
          '#ec4899',
          '#f59e0b',
        ]
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          position: 'bottom',
          labels: { color: '#d1d5db' }
        }
      }
    }
  });
}

/**
 * Render violations by severity bar chart
 */
function renderSeverityChart(violationsBySeverity) {
  const ctx = document.getElementById('severity-chart');
  if (!ctx) return;

  // Handle missing or empty data
  if (!violationsBySeverity || typeof violationsBySeverity !== 'object' || Object.keys(violationsBySeverity).length === 0) {
    ctx.parentElement.innerHTML = '<p class="text-gray-500 text-center py-8">No severity data available</p>';
    return;
  }

  const severityOrder = ['Low', 'Medium', 'High', 'Critical'];
  const labels = severityOrder.filter(s => violationsBySeverity[s] !== undefined);
  const data = labels.map(s => violationsBySeverity[s]);

  const colors = {
    'Low': '#10b981',
    'Medium': '#f59e0b',
    'High': '#f97316',
    'Critical': '#ef4444'
  };

  new Chart(ctx, {
    type: 'bar',
    data: {
      labels: labels,
      datasets: [{
        label: 'Violations',
        data: data,
        backgroundColor: labels.map(l => colors[l])
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          display: false
        }
      },
      scales: {
        x: {
          ticks: { color: '#9ca3af' },
          grid: { display: false }
        },
        y: {
          ticks: { color: '#9ca3af' },
          grid: { color: '#374151' }
        }
      }
    }
  });
}

/**
 * Render violation distribution chart
 */
function renderDistributionChart(distribution) {
  const ctx = document.getElementById('distribution-chart');
  if (!ctx) return;

  // Handle missing or empty data
  if (!distribution || typeof distribution !== 'object' || Object.keys(distribution).length === 0) {
    ctx.parentElement.innerHTML = '<p class="text-gray-500 text-center py-8">No distribution data available</p>';
    return;
  }

  // Convert distribution object to sorted arrays
  const entries = Object.entries(distribution).map(([count, users]) => ({
    count: parseInt(count),
    users: users
  })).sort((a, b) => a.count - b.count);

  const labels = entries.map(e => `${e.count} violation${e.count > 1 ? 's' : ''}`);
  const data = entries.map(e => e.users);

  new Chart(ctx, {
    type: 'bar',
    data: {
      labels: labels,
      datasets: [{
        label: 'Number of Users',
        data: data,
        backgroundColor: '#6366f1'
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          display: false
        }
      },
      scales: {
        x: {
          ticks: { color: '#9ca3af' },
          grid: { display: false }
        },
        y: {
          ticks: { color: '#9ca3af' },
          grid: { color: '#374151' }
        }
      }
    }
  });
}

/**
 * Render the violations page
 */
async function renderViolationsPage() {
  const serverId = sessionStorage.getItem('selectedServerId');
  const serverName = sessionStorage.getItem('selectedServerName');

  if (!serverId) {
    router.navigate('/servers');
    return;
  }

  const app = document.getElementById('app');

  app.innerHTML = `
    ${renderNavbar(serverName)}
    <div class="min-h-screen bg-gray-900 pt-16">
      <div class="max-w-7xl mx-auto px-4 py-8">
        <div class="flex justify-between items-center mb-6">
          <h1 class="text-3xl font-bold text-gray-100">Violations</h1>
          <button id="export-btn" class="btn btn-primary">
            <svg class="w-5 h-5 inline mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 10v6m0 0l-3-3m3 3l3-3m2 8H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            Export CSV
          </button>
        </div>

        <!-- Filters -->
        <div class="card mb-6">
          <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
            <div class="form-group mb-0">
              <label class="form-label">Severity</label>
              <select id="severity-filter" class="form-select">
                <option value="">All</option>
                <option value="Low">Low</option>
                <option value="Medium">Medium</option>
                <option value="High">High</option>
                <option value="Critical">Critical</option>
              </select>
            </div>
            <div class="form-group mb-0">
              <label class="form-label">Detection Type</label>
              <select id="type-filter" class="form-select">
                <option value="">All</option>
                <option value="Regex">Regex</option>
                <option value="AI">AI</option>
              </select>
            </div>
            <div class="form-group mb-0">
              <label class="form-label">User ID</label>
              <input type="text" id="user-filter" class="form-input" placeholder="Filter by user...">
            </div>
            <div class="form-group mb-0 flex items-end">
              <button id="apply-filters-btn" class="btn btn-primary w-full">Apply Filters</button>
            </div>
          </div>
        </div>

        <div id="violations-content">
          <div class="flex items-center justify-center py-12">
            <div class="text-center">
              <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-500 mx-auto"></div>
              <p class="mt-4 text-gray-400">Loading violations...</p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Violation Detail Modal -->
    <div id="violation-modal" class="hidden fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50" onclick="closeViolationModal(event)">
      <div class="card max-w-2xl w-full mx-4" onclick="event.stopPropagation()">
        <div class="flex justify-between items-start mb-4">
          <h3 class="text-xl font-bold text-gray-100">Violation Details</h3>
          <button onclick="closeViolationModal()" class="text-gray-400 hover:text-gray-200">
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div id="modal-content"></div>
      </div>
    </div>
  `;

  // Set up event listeners
  document.getElementById('apply-filters-btn').addEventListener('click', () => {
    loadViolations(serverId, 1);
  });

  document.getElementById('export-btn').addEventListener('click', () => {
    exportViolations(serverId);
  });

  // Load initial data
  await loadViolations(serverId, 1);
}

/**
 * Load violations with filters and pagination
 */
async function loadViolations(serverId, page = 1) {
  const content = document.getElementById('violations-content');

  const params = {
    page,
    per_page: 20
  };

  const severity = document.getElementById('severity-filter')?.value;
  const type = document.getElementById('type-filter')?.value;
  const userId = document.getElementById('user-filter')?.value;

  if (severity) params.severity = severity;
  if (type) params.detection_type = type;
  if (userId) params.user_id = userId;

  try {
    const response = await api.getViolations(serverId, params);

    if (response.violations.length === 0) {
      content.innerHTML = `
        <div class="card text-center py-12">
          <svg class="w-16 h-16 mx-auto text-gray-600 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <p class="text-gray-400">No violations found</p>
        </div>
      `;
      return;
    }

    const totalPages = Math.ceil(response.total / response.per_page);

    content.innerHTML = `
      <div class="card">
        <div class="overflow-x-auto">
          <table class="table">
            <thead>
              <tr>
                <th>Timestamp</th>
                <th>User</th>
                <th>Reason</th>
                <th>Severity</th>
                <th>Type</th>
                <th>Action</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              ${response.violations.map(v => `
                <tr>
                  <td class="text-sm">${new Date(v.timestamp).toLocaleString()}</td>
                  <td class="font-medium">${v.username || v.user_id}</td>
                  <td class="text-sm max-w-xs truncate">${v.reason}</td>
                  <td><span class="badge badge-${v.severity.toLowerCase()}">${v.severity}</span></td>
                  <td><span class="text-sm text-gray-400">${v.detection_type}</span></td>
                  <td><span class="text-sm text-gray-400">${v.action_taken}</span></td>
                  <td>
                    <button onclick="showViolationDetail('${v.id}')" class="text-indigo-400 hover:text-indigo-300 text-sm">
                      Details →
                    </button>
                  </td>
                </tr>
              `).join('')}
            </tbody>
          </table>
        </div>

        <!-- Pagination -->
        ${totalPages > 1 ? `
          <div class="flex justify-between items-center mt-6 pt-6 border-t border-gray-700">
            <p class="text-sm text-gray-400">
              Showing ${(page - 1) * response.per_page + 1} to ${Math.min(page * response.per_page, response.total)} of ${response.total} violations
            </p>
            <div class="flex gap-2">
              <button 
                onclick="loadViolations('${serverId}', ${page - 1})" 
                class="btn btn-secondary btn-sm" 
                ${page === 1 ? 'disabled' : ''}
              >
                Previous
              </button>
              <span class="px-4 py-2 text-gray-300">Page ${page} of ${totalPages}</span>
              <button 
                onclick="loadViolations('${serverId}', ${page + 1})" 
                class="btn btn-secondary btn-sm"
                ${page === totalPages ? 'disabled' : ''}
              >
                Next
              </button>
            </div>
          </div>
        ` : ''}
      </div>
    `;

  } catch (error) {
    console.error('Failed to load violations:', error);
    content.innerHTML = `
      <div class="error-message">
        <p class="font-semibold">Failed to load violations</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
  }
}

/**
 * Show violation detail modal
 */
function showViolationDetail(violationId) {
  const modal = document.getElementById('violation-modal');
  const modalContent = document.getElementById('modal-content');

  // In a real implementation, we'd fetch full details from the API
  // For now, just show the ID
  modalContent.innerHTML = `
    <div class="space-y-4">
      <div>
        <p class="text-sm text-gray-400 mb-1">Violation ID</p>
        <p class="text-gray-100 font-mono">${violationId}</p>
      </div>
      <div>
        <p class="text-sm text-gray-400 mb-1">Message Content Hash</p>
        <p class="text-gray-100 font-mono text-sm break-all">SHA256:${violationId.substring(0, 32)}...</p>
      </div>
      <p class="text-sm text-gray-500">Full violation details would be displayed here</p>
    </div>
  `;

  modal.classList.remove('hidden');
}

/**
 * Close violation detail modal
 */
function closeViolationModal(event) {
  if (!event || event.target.id === 'violation-modal') {
    document.getElementById('violation-modal').classList.add('hidden');
  }
}

/**
 * Export violations to CSV
 */
async function exportViolations(serverId) {
  const params = {};

  const severity = document.getElementById('severity-filter')?.value;
  const type = document.getElementById('type-filter')?.value;
  const userId = document.getElementById('user-filter')?.value;

  if (severity) params.severity = severity;
  if (type) params.detection_type = type;
  if (userId) params.user_id = userId;

  try {
    await api.exportViolations(serverId, params);
  } catch (error) {
    console.error('Failed to export violations:', error);
    alert('Failed to export violations: ' + error.message);
  }
}

// Make functions available globally
window.loadViolations = loadViolations;
window.showViolationDetail = showViolationDetail;
window.closeViolationModal = closeViolationModal;

/**
 * Render the rules page
 */
async function renderRulesPage() {
  const serverId = sessionStorage.getItem('selectedServerId');
  const serverName = sessionStorage.getItem('selectedServerName');

  if (!serverId) {
    router.navigate('/servers');
    return;
  }

  const app = document.getElementById('app');

  app.innerHTML = `
    ${renderNavbar(serverName)}
    <div class="min-h-screen bg-gray-900 pt-16">
      <div class="max-w-5xl mx-auto px-4 py-8">
        <h1 class="text-3xl font-bold text-gray-100 mb-6">Server Rules</h1>

        <div id="rules-content">
          <div class="flex items-center justify-center py-12">
            <div class="text-center">
              <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-500 mx-auto"></div>
              <p class="mt-4 text-gray-400">Loading rules...</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;

  await loadRules(serverId);
}

/**
 * Load and display rules
 */
async function loadRules(serverId) {
  const content = document.getElementById('rules-content');

  try {
    const response = await api.getRules(serverId);

    content.innerHTML = `
      <div class="card mb-6">
        <div class="flex justify-between items-center mb-4">
          <div>
            <h3 class="text-lg font-semibold text-gray-100">Custom Rules</h3>
            ${response.last_updated ? `
              <p class="text-sm text-gray-400 mt-1">
                Last updated: ${new Date(response.last_updated).toLocaleString()}
                ${response.updated_by ? ` by ${response.updated_by}` : ''}
              </p>
            ` : ''}
          </div>
          <div class="flex gap-2">
            <button id="reset-btn" class="btn btn-secondary">Reset to Default</button>
            <button id="save-btn" class="btn btn-primary">Save Rules</button>
          </div>
        </div>

        <div class="form-group">
          <label class="form-label">Rules (one per line)</label>
          <textarea 
            id="rules-editor" 
            class="form-textarea" 
            rows="15"
            placeholder="Enter custom moderation rules, one per line..."
          >${response.rules || ''}</textarea>
          <p class="text-sm text-gray-500 mt-2">
            Each line represents a rule that will be checked against messages. Use clear, specific language.
          </p>
        </div>

        <div id="rules-message"></div>
      </div>

      <!-- Example Templates -->
      <div class="card">
        <h3 class="text-lg font-semibold text-gray-100 mb-4">Example Templates</h3>
        <p class="text-sm text-gray-400 mb-4">Click to insert a template into the editor</p>
        
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="bg-gray-800 p-4 rounded-lg cursor-pointer hover:bg-gray-700" onclick="insertTemplate('basic')">
            <h4 class="font-semibold text-gray-200 mb-2">Basic Moderation</h4>
            <p class="text-sm text-gray-400">Common rules for general servers</p>
          </div>
          
          <div class="bg-gray-800 p-4 rounded-lg cursor-pointer hover:bg-gray-700" onclick="insertTemplate('strict')">
            <h4 class="font-semibold text-gray-200 mb-2">Strict Moderation</h4>
            <p class="text-sm text-gray-400">Comprehensive rules for family-friendly servers</p>
          </div>
          
          <div class="bg-gray-800 p-4 rounded-lg cursor-pointer hover:bg-gray-700" onclick="insertTemplate('gaming')">
            <h4 class="font-semibold text-gray-200 mb-2">Gaming Community</h4>
            <p class="text-sm text-gray-400">Rules focused on gaming communities</p>
          </div>
          
          <div class="bg-gray-800 p-4 rounded-lg cursor-pointer hover:bg-gray-700" onclick="insertTemplate('professional')">
            <h4 class="font-semibold text-gray-200 mb-2">Professional</h4>
            <p class="text-sm text-gray-400">Rules for professional/business servers</p>
          </div>
        </div>
      </div>
    `;

    // Set up event listeners
    document.getElementById('save-btn').addEventListener('click', () => saveRules(serverId));
    document.getElementById('reset-btn').addEventListener('click', () => resetRules(serverId));

  } catch (error) {
    console.error('Failed to load rules:', error);
    content.innerHTML = `
      <div class="error-message">
        <p class="font-semibold">Failed to load rules</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
  }
}

/**
 * Save rules
 */
async function saveRules(serverId) {
  const editor = document.getElementById('rules-editor');
  const message = document.getElementById('rules-message');
  const saveBtn = document.getElementById('save-btn');

  const rules = editor.value.trim();

  saveBtn.disabled = true;
  saveBtn.textContent = 'Saving...';

  try {
    await api.updateRules(serverId, rules);

    message.innerHTML = `
      <div class="success-message mt-4">
        <p class="font-semibold">Rules saved successfully</p>
      </div>
    `;

    setTimeout(() => {
      message.innerHTML = '';
    }, 3000);

  } catch (error) {
    console.error('Failed to save rules:', error);
    message.innerHTML = `
      <div class="error-message mt-4">
        <p class="font-semibold">Failed to save rules</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
  } finally {
    saveBtn.disabled = false;
    saveBtn.textContent = 'Save Rules';
  }
}

/**
 * Reset rules to default
 */
async function resetRules(serverId) {
  if (!confirm('Are you sure you want to reset rules to default? This will clear all custom rules.')) {
    return;
  }

  const message = document.getElementById('rules-message');
  const resetBtn = document.getElementById('reset-btn');

  resetBtn.disabled = true;
  resetBtn.textContent = 'Resetting...';

  try {
    await api.deleteRules(serverId);

    message.innerHTML = `
      <div class="success-message mt-4">
        <p class="font-semibold">Rules reset to default</p>
      </div>
    `;

    // Reload rules
    setTimeout(() => {
      loadRules(serverId);
    }, 1000);

  } catch (error) {
    console.error('Failed to reset rules:', error);
    message.innerHTML = `
      <div class="error-message mt-4">
        <p class="font-semibold">Failed to reset rules</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
    resetBtn.disabled = false;
    resetBtn.textContent = 'Reset to Default';
  }
}

/**
 * Insert a template into the editor
 */
function insertTemplate(templateName) {
  const editor = document.getElementById('rules-editor');

  const templates = {
    basic: `No harassment or hate speech
No spam or excessive self-promotion
No NSFW content
Be respectful to all members
Follow Discord's Terms of Service`,

    strict: `No profanity or vulgar language
No harassment, bullying, or hate speech
No NSFW content of any kind
No spam, advertising, or self-promotion
No political or religious discussions
Be respectful and kind to all members
No sharing of personal information
Follow Discord's Terms of Service`,

    gaming: `No cheating or exploits discussion
No toxic behavior or flaming
No spam in voice or text channels
No backseat gaming unless requested
Be respectful to teammates
No advertising other servers or games
Follow Discord's Terms of Service`,

    professional: `Maintain professional conduct at all times
No spam or off-topic discussions
No sharing of confidential information
Be respectful in all communications
No solicitation or advertising
Stay on topic in designated channels
Follow Discord's Terms of Service`
  };

  if (templates[templateName]) {
    if (editor.value.trim() && !confirm('This will replace your current rules. Continue?')) {
      return;
    }
    editor.value = templates[templateName];
  }
}

// Make functions available globally
window.insertTemplate = insertTemplate;

/**
 * Render the configuration page
 */
async function renderConfigPage() {
  const serverId = sessionStorage.getItem('selectedServerId');
  const serverName = sessionStorage.getItem('selectedServerName');

  if (!serverId) {
    router.navigate('/servers');
    return;
  }

  const app = document.getElementById('app');

  app.innerHTML = `
    ${renderNavbar(serverName)}
    <div class="min-h-screen bg-gray-900 pt-16">
      <div class="max-w-4xl mx-auto px-4 py-8">
        <h1 class="text-3xl font-bold text-gray-100 mb-6">Bot Configuration</h1>

        <div id="config-content">
          <div class="flex items-center justify-center py-12">
            <div class="text-center">
              <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-500 mx-auto"></div>
              <p class="mt-4 text-gray-400">Loading configuration...</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;

  await loadConfig(serverId);
}

/**
 * Load and display configuration
 */
async function loadConfig(serverId) {
  const content = document.getElementById('config-content');

  try {
    const config = await api.getConfig(serverId);

    content.innerHTML = `
      <div class="card">
        <form id="config-form" class="space-y-6">
          <!-- Severity Threshold -->
          <div class="form-group">
            <label class="form-label flex items-center gap-2">
              Severity Threshold
              <span class="tooltip">
                <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <span class="tooltip-text">Minimum severity level (0.0-1.0) for a message to be flagged. Lower values are more strict.</span>
              </span>
            </label>
            <input 
              type="number" 
              id="severity-threshold" 
              class="form-input" 
              min="0" 
              max="1" 
              step="0.1" 
              value="${config.severity_threshold || 0.5}"
              required
            >
            <p class="text-sm text-gray-500 mt-1">Value between 0.0 (most strict) and 1.0 (least strict)</p>
          </div>

          <!-- Buffer Timeout -->
          <div class="form-group">
            <label class="form-label flex items-center gap-2">
              Buffer Timeout (seconds)
              <span class="tooltip">
                <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <span class="tooltip-text">How long to wait (in seconds) before processing buffered messages. Must be greater than 0.</span>
              </span>
            </label>
            <input 
              type="number" 
              id="buffer-timeout" 
              class="form-input" 
              min="1" 
              value="${config.buffer_timeout || 5}"
              required
            >
            <p class="text-sm text-gray-500 mt-1">Minimum value: 1 second</p>
          </div>

          <!-- Buffer Threshold -->
          <div class="form-group">
            <label class="form-label flex items-center gap-2">
              Buffer Threshold
              <span class="tooltip">
                <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <span class="tooltip-text">Number of messages to buffer before processing. Must be greater than 0.</span>
              </span>
            </label>
            <input 
              type="number" 
              id="buffer-threshold" 
              class="form-input" 
              min="1" 
              value="${config.buffer_threshold || 10}"
              required
            >
            <p class="text-sm text-gray-500 mt-1">Minimum value: 1 message</p>
          </div>

          <!-- Validation Errors -->
          <div id="validation-errors"></div>

          <!-- Success Message -->
          <div id="config-message"></div>

          <!-- Actions -->
          <div class="flex gap-4">
            <button type="submit" class="btn btn-primary flex-1">
              Save Configuration
            </button>
            <button type="button" onclick="loadConfig('${serverId}')" class="btn btn-secondary">
              Reset
            </button>
          </div>
        </form>
      </div>

      <!-- Configuration Info -->
      <div class="card mt-6">
        <h3 class="text-lg font-semibold text-gray-100 mb-4">Configuration Guide</h3>
        <div class="space-y-4 text-sm text-gray-400">
          <div>
            <p class="font-semibold text-gray-300 mb-1">Severity Threshold</p>
            <p>Controls how sensitive the bot is to potential violations. A lower value (e.g., 0.3) will flag more messages, while a higher value (e.g., 0.7) will only flag clear violations.</p>
          </div>
          <div>
            <p class="font-semibold text-gray-300 mb-1">Buffer Timeout</p>
            <p>The maximum time to wait before processing buffered messages. This helps batch process messages for efficiency.</p>
          </div>
          <div>
            <p class="font-semibold text-gray-300 mb-1">Buffer Threshold</p>
            <p>The number of messages to accumulate before processing. Higher values improve efficiency but increase latency.</p>
          </div>
        </div>
      </div>
    `;

    // Set up form submission
    document.getElementById('config-form').addEventListener('submit', (e) => {
      e.preventDefault();
      saveConfig(serverId);
    });

  } catch (error) {
    console.error('Failed to load configuration:', error);
    content.innerHTML = `
      <div class="error-message">
        <p class="font-semibold">Failed to load configuration</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
  }
}

/**
 * Save configuration
 */
async function saveConfig(serverId) {
  const form = document.getElementById('config-form');
  const validationErrors = document.getElementById('validation-errors');
  const message = document.getElementById('config-message');
  const submitBtn = form.querySelector('button[type="submit"]');

  // Clear previous messages
  validationErrors.innerHTML = '';
  message.innerHTML = '';

  // Get form values
  const severityThreshold = parseFloat(document.getElementById('severity-threshold').value);
  const bufferTimeout = parseInt(document.getElementById('buffer-timeout').value);
  const bufferThreshold = parseInt(document.getElementById('buffer-threshold').value);

  // Client-side validation
  const errors = [];

  if (isNaN(severityThreshold) || severityThreshold < 0 || severityThreshold > 1) {
    errors.push('Severity threshold must be between 0.0 and 1.0');
  }

  if (isNaN(bufferTimeout) || bufferTimeout < 1) {
    errors.push('Buffer timeout must be at least 1 second');
  }

  if (isNaN(bufferThreshold) || bufferThreshold < 1) {
    errors.push('Buffer threshold must be at least 1 message');
  }

  if (errors.length > 0) {
    validationErrors.innerHTML = `
      <div class="error-message">
        <p class="font-semibold mb-2">Validation Errors:</p>
        <ul class="list-disc list-inside space-y-1">
          ${errors.map(err => `<li class="text-sm">${err}</li>`).join('')}
        </ul>
      </div>
    `;
    return;
  }

  // Submit to API
  submitBtn.disabled = true;
  submitBtn.textContent = 'Saving...';

  try {
    await api.updateConfig(serverId, {
      severity_threshold: severityThreshold,
      buffer_timeout: bufferTimeout,
      buffer_threshold: bufferThreshold
    });

    message.innerHTML = `
      <div class="success-message">
        <p class="font-semibold">Configuration saved successfully</p>
        <p class="text-sm mt-1">Changes will take effect immediately</p>
      </div>
    `;

    setTimeout(() => {
      message.innerHTML = '';
    }, 5000);

  } catch (error) {
    console.error('Failed to save configuration:', error);

    // Check if error has validation details
    if (error.data && error.data.errors) {
      validationErrors.innerHTML = `
        <div class="error-message">
          <p class="font-semibold mb-2">Validation Errors:</p>
          <ul class="list-disc list-inside space-y-1">
            ${Object.entries(error.data.errors).map(([field, msg]) =>
        `<li class="text-sm">${field}: ${msg}</li>`
      ).join('')}
          </ul>
        </div>
      `;
    } else {
      message.innerHTML = `
        <div class="error-message">
          <p class="font-semibold">Failed to save configuration</p>
          <p class="text-sm mt-2">${error.message}</p>
        </div>
      `;
    }
  } finally {
    submitBtn.disabled = false;
    submitBtn.textContent = 'Save Configuration';
  }
}

/**
 * Render the warnings page
 */
async function renderWarningsPage() {
  const serverId = sessionStorage.getItem('selectedServerId');
  const serverName = sessionStorage.getItem('selectedServerName');

  if (!serverId) {
    router.navigate('/servers');
    return;
  }

  const app = document.getElementById('app');

  app.innerHTML = `
    ${renderNavbar(serverName)}
    <div class="min-h-screen bg-gray-900 pt-16">
      <div class="max-w-7xl mx-auto px-4 py-8">
        <h1 class="text-3xl font-bold text-gray-100 mb-6">User Warnings</h1>

        <!-- Search and Bulk Actions -->
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
          <!-- Search -->
          <div class="card">
            <h3 class="text-lg font-semibold text-gray-100 mb-4">Search Users</h3>
            <div class="flex gap-2">
              <input 
                type="text" 
                id="search-input" 
                class="form-input flex-1" 
                placeholder="Search by username or user ID..."
              >
              <button id="search-btn" class="btn btn-primary">Search</button>
            </div>
          </div>

          <!-- Bulk Clear -->
          <div class="card">
            <h3 class="text-lg font-semibold text-gray-100 mb-4">Bulk Clear Warnings</h3>
            <div class="flex gap-2">
              <input 
                type="date" 
                id="bulk-date" 
                class="form-input flex-1"
                max="${new Date().toISOString().split('T')[0]}"
              >
              <button id="bulk-clear-btn" class="btn btn-danger">Clear Older</button>
            </div>
            <p class="text-sm text-gray-500 mt-2">Clear warnings for users with last violation before this date</p>
          </div>
        </div>

        <div id="warnings-content">
          <div class="flex items-center justify-center py-12">
            <div class="text-center">
              <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-500 mx-auto"></div>
              <p class="mt-4 text-gray-400">Loading warnings...</p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- User Detail Modal -->
    <div id="user-modal" class="hidden fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50" onclick="closeUserModal(event)">
      <div class="card max-w-3xl w-full mx-4 max-h-[80vh] overflow-y-auto" onclick="event.stopPropagation()">
        <div class="flex justify-between items-start mb-4">
          <h3 class="text-xl font-bold text-gray-100">User Warning Details</h3>
          <button onclick="closeUserModal()" class="text-gray-400 hover:text-gray-200">
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div id="user-modal-content"></div>
      </div>
    </div>
  `;

  // Set up event listeners
  document.getElementById('search-btn').addEventListener('click', () => {
    const query = document.getElementById('search-input').value;
    loadWarnings(serverId, query);
  });

  document.getElementById('search-input').addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
      const query = document.getElementById('search-input').value;
      loadWarnings(serverId, query);
    }
  });

  document.getElementById('bulk-clear-btn').addEventListener('click', () => {
    bulkClearWarnings(serverId);
  });

  // Load initial data
  await loadWarnings(serverId, '');
}

/**
 * Load warnings list
 */
async function loadWarnings(serverId, search = '') {
  const content = document.getElementById('warnings-content');

  try {
    const response = await api.getWarnings(serverId, search);

    if (response.users.length === 0) {
      content.innerHTML = `
        <div class="card text-center py-12">
          <svg class="w-16 h-16 mx-auto text-gray-600 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <p class="text-gray-400">${search ? 'No users found matching your search' : 'No users with active warnings'}</p>
        </div>
      `;
      return;
    }

    content.innerHTML = `
      <div class="card">
        <div class="overflow-x-auto">
          <table class="table">
            <thead>
              <tr>
                <th>User</th>
                <th>Warning Level</th>
                <th>Violation Count</th>
                <th>Last Violation</th>
                <th>Kicked</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              ${response.users.map(user => `
                <tr>
                  <td class="font-medium">${user.username || user.user_id}</td>
                  <td>
                    <div class="flex items-center gap-1">
                      ${Array(user.warning_level).fill('⚠️').join('')}
                      <span class="text-sm text-gray-400 ml-2">Level ${user.warning_level}</span>
                    </div>
                  </td>
                  <td><span class="badge badge-high">${user.violation_count}</span></td>
                  <td class="text-sm text-gray-400">${new Date(user.last_violation).toLocaleString()}</td>
                  <td>
                    ${user.kicked
        ? '<span class="text-red-400">Yes</span>'
        : '<span class="text-gray-500">No</span>'
      }
                  </td>
                  <td>
                    <div class="flex gap-2">
                      <button 
                        onclick="showUserDetail('${serverId}', '${user.user_id}')" 
                        class="text-indigo-400 hover:text-indigo-300 text-sm"
                      >
                        View
                      </button>
                      <button 
                        onclick="clearUserWarning('${serverId}', '${user.user_id}')" 
                        class="text-red-400 hover:text-red-300 text-sm"
                      >
                        Clear
                      </button>
                    </div>
                  </td>
                </tr>
              `).join('')}
            </tbody>
          </table>
        </div>
      </div>
    `;

  } catch (error) {
    console.error('Failed to load warnings:', error);
    content.innerHTML = `
      <div class="error-message">
        <p class="font-semibold">Failed to load warnings</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
  }
}

/**
 * Show user detail modal
 */
async function showUserDetail(serverId, userId) {
  const modal = document.getElementById('user-modal');
  const modalContent = document.getElementById('user-modal-content');

  modalContent.innerHTML = `
    <div class="flex items-center justify-center py-8">
      <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-500"></div>
    </div>
  `;

  modal.classList.remove('hidden');

  try {
    const user = await api.getUserWarning(serverId, userId);

    modalContent.innerHTML = `
      <div class="space-y-6">
        <!-- User Info -->
        <div class="grid grid-cols-2 gap-4">
          <div>
            <p class="text-sm text-gray-400 mb-1">User ID</p>
            <p class="text-gray-100 font-mono">${user.user_id}</p>
          </div>
          <div>
            <p class="text-sm text-gray-400 mb-1">Username</p>
            <p class="text-gray-100">${user.username || 'Unknown'}</p>
          </div>
          <div>
            <p class="text-sm text-gray-400 mb-1">Warning Level</p>
            <div class="flex items-center gap-2">
              ${Array(user.warning_level).fill('⚠️').join('')}
              <span class="text-gray-100">Level ${user.warning_level}</span>
            </div>
          </div>
          <div>
            <p class="text-sm text-gray-400 mb-1">Status</p>
            <p class="text-gray-100">
              ${user.kicked
        ? '<span class="text-red-400">Kicked</span>'
        : '<span class="text-green-400">Active</span>'
      }
            </p>
          </div>
        </div>

        <!-- Violation History -->
        <div>
          <h4 class="text-lg font-semibold text-gray-100 mb-3">Recent Violations</h4>
          <div class="space-y-2 max-h-64 overflow-y-auto">
            ${user.violations.map(v => `
              <div class="bg-gray-800 p-3 rounded-lg">
                <div class="flex justify-between items-start mb-2">
                  <span class="badge badge-${v.severity.toLowerCase()}">${v.severity}</span>
                  <span class="text-xs text-gray-500">${new Date(v.timestamp).toLocaleString()}</span>
                </div>
                <p class="text-sm text-gray-300">${v.reason}</p>
                <p class="text-xs text-gray-500 mt-1">Action: ${v.action_taken}</p>
              </div>
            `).join('')}
          </div>
        </div>

        <!-- Actions -->
        <div class="flex gap-4 pt-4 border-t border-gray-700">
          <button 
            onclick="clearUserWarning('${serverId}', '${userId}'); closeUserModal();" 
            class="btn btn-danger flex-1"
          >
            Clear Warnings
          </button>
          <button onclick="closeUserModal()" class="btn btn-secondary flex-1">
            Close
          </button>
        </div>
      </div>
    `;

  } catch (error) {
    console.error('Failed to load user details:', error);
    modalContent.innerHTML = `
      <div class="error-message">
        <p class="font-semibold">Failed to load user details</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
  }
}

/**
 * Clear warning for a specific user
 */
async function clearUserWarning(serverId, userId) {
  if (!confirm('Are you sure you want to clear warnings for this user?')) {
    return;
  }

  try {
    await api.clearUserWarning(serverId, userId);

    // Reload warnings list
    const search = document.getElementById('search-input').value;
    await loadWarnings(serverId, search);

    // Show success message
    const content = document.getElementById('warnings-content');
    const successMsg = document.createElement('div');
    successMsg.className = 'success-message mb-4';
    successMsg.innerHTML = '<p class="font-semibold">Warning cleared successfully</p>';
    content.insertBefore(successMsg, content.firstChild);

    setTimeout(() => successMsg.remove(), 3000);

  } catch (error) {
    console.error('Failed to clear warning:', error);
    alert('Failed to clear warning: ' + error.message);
  }
}

/**
 * Bulk clear warnings
 */
async function bulkClearWarnings(serverId) {
  const dateInput = document.getElementById('bulk-date');
  const date = dateInput.value;

  if (!date) {
    alert('Please select a date');
    return;
  }

  if (!confirm(`Are you sure you want to clear warnings for all users with last violation before ${date}?`)) {
    return;
  }

  const btn = document.getElementById('bulk-clear-btn');
  btn.disabled = true;
  btn.textContent = 'Clearing...';

  try {
    const result = await api.bulkClearWarnings(serverId, date);

    alert(`Successfully cleared warnings for ${result.cleared_count || 0} users`);

    // Reload warnings list
    const search = document.getElementById('search-input').value;
    await loadWarnings(serverId, search);

  } catch (error) {
    console.error('Failed to bulk clear warnings:', error);
    alert('Failed to bulk clear warnings: ' + error.message);
  } finally {
    btn.disabled = false;
    btn.textContent = 'Clear Older';
  }
}

/**
 * Close user detail modal
 */
function closeUserModal(event) {
  if (!event || event.target.id === 'user-modal') {
    document.getElementById('user-modal').classList.add('hidden');
  }
}

// Make functions available globally
window.showUserDetail = showUserDetail;
window.clearUserWarning = clearUserWarning;
window.closeUserModal = closeUserModal;

function renderHealthPage() {
  const app = document.getElementById('app');
  app.innerHTML = '<div class="p-8">Health - To be implemented</div>';
}

function renderOffendersPage() {
  const app = document.getElementById('app');
  app.innerHTML = '<div class="p-8">Offenders - To be implemented</div>';
}

/**
 * Render the rule effectiveness page
 */
async function renderRuleEffectivenessPage() {
  const serverId = sessionStorage.getItem('selectedServerId');
  const serverName = sessionStorage.getItem('selectedServerName');

  if (!serverId) {
    router.navigate('/servers');
    return;
  }

  const app = document.getElementById('app');

  app.innerHTML = `
    ${renderNavbar(serverName)}
    <div class="min-h-screen bg-gray-900 pt-16">
      <div class="max-w-7xl mx-auto px-4 py-8">
        <div class="flex justify-between items-center mb-6">
          <h1 class="text-3xl font-bold text-gray-100">Rule Effectiveness</h1>
          <select id="period-selector" class="form-select w-auto">
            <option value="day">Last 24 Hours</option>
            <option value="week" selected>Last Week</option>
            <option value="month">Last Month</option>
          </select>
        </div>

        <div id="effectiveness-content">
          <div class="flex items-center justify-center py-12">
            <div class="text-center">
              <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-500 mx-auto"></div>
              <p class="mt-4 text-gray-400">Loading rule effectiveness data...</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;

  // Set up event listener
  document.getElementById('period-selector').addEventListener('change', (e) => {
    loadRuleEffectiveness(serverId, e.target.value);
  });

  await loadRuleEffectiveness(serverId, 'week');
}

/**
 * Load rule effectiveness data
 */
async function loadRuleEffectiveness(serverId, period) {
  const content = document.getElementById('effectiveness-content');

  try {
    const data = await api.getRuleEffectiveness(serverId, period);

    if (data.top_rules.length === 0) {
      content.innerHTML = `
        <div class="card text-center py-12">
          <svg class="w-16 h-16 mx-auto text-gray-600 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <p class="text-gray-400">No rule violations found for this period</p>
          <p class="text-sm text-gray-500 mt-2">Either no custom rules are configured or no violations have occurred</p>
        </div>
      `;
      return;
    }

    content.innerHTML = `
      <!-- Top Rules Chart -->
      <div class="card mb-6">
        <h3 class="text-lg font-semibold text-gray-100 mb-4">Top 5 Most Triggered Rules</h3>
        <div class="chart-container">
          <canvas id="top-rules-chart"></canvas>
        </div>
      </div>

      <!-- Rule Details -->
      <div class="grid grid-cols-1 gap-6">
        ${data.top_rules.map((rule, index) => `
          <div class="card">
            <div class="flex justify-between items-start mb-4">
              <div class="flex-1">
                <div class="flex items-center gap-3 mb-2">
                  <span class="text-2xl font-bold text-indigo-400">#${index + 1}</span>
                  <h3 class="text-lg font-semibold text-gray-100">${rule.rule_name}</h3>
                </div>
                <p class="text-sm text-gray-400">
                  ${rule.violation_count} violation${rule.violation_count !== 1 ? 's' : ''} 
                  (${((rule.violation_count / data.total_rule_violations) * 100).toFixed(1)}% of total)
                </p>
              </div>
            </div>

            <!-- Severity Distribution -->
            <div>
              <p class="text-sm text-gray-400 mb-3">Severity Distribution</p>
              <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                ${Object.entries(rule.severity_distribution).map(([severity, count]) => `
                  <div class="bg-gray-800 p-3 rounded-lg">
                    <p class="text-xs text-gray-400 mb-1">${severity}</p>
                    <p class="text-xl font-bold text-gray-100">${count}</p>
                    <p class="text-xs text-gray-500">${((count / rule.violation_count) * 100).toFixed(0)}%</p>
                  </div>
                `).join('')}
              </div>
            </div>
          </div>
        `).join('')}
      </div>

      ${data.top_rules.length === 0 ? `
        <div class="card text-center py-8 mt-6">
          <p class="text-gray-400">No rules with zero violations to display</p>
        </div>
      ` : ''}
    `;

    // Render chart
    renderTopRulesChart(data.top_rules);

  } catch (error) {
    console.error('Failed to load rule effectiveness:', error);
    content.innerHTML = `
      <div class="error-message">
        <p class="font-semibold">Failed to load rule effectiveness data</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
  }
}

/**
 * Render top rules chart
 */
function renderTopRulesChart(topRules) {
  const ctx = document.getElementById('top-rules-chart');
  if (!ctx) return;

  const labels = topRules.map(r => r.rule_name.length > 30 ? r.rule_name.substring(0, 30) + '...' : r.rule_name);
  const data = topRules.map(r => r.violation_count);

  new Chart(ctx, {
    type: 'bar',
    data: {
      labels: labels,
      datasets: [{
        label: 'Violations',
        data: data,
        backgroundColor: '#6366f1'
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      indexAxis: 'y',
      plugins: {
        legend: {
          display: false
        }
      },
      scales: {
        x: {
          ticks: { color: '#9ca3af' },
          grid: { color: '#374151' }
        },
        y: {
          ticks: { color: '#9ca3af' },
          grid: { display: false }
        }
      }
    }
  });
}

/**
 * Render the temporal analytics page
 */
async function renderTemporalAnalyticsPage() {
  const serverId = sessionStorage.getItem('selectedServerId');
  const serverName = sessionStorage.getItem('selectedServerName');

  if (!serverId) {
    router.navigate('/servers');
    return;
  }

  const app = document.getElementById('app');

  app.innerHTML = `
    ${renderNavbar(serverName)}
    <div class="min-h-screen bg-gray-900 pt-16">
      <div class="max-w-7xl mx-auto px-4 py-8">
        <div class="flex justify-between items-center mb-6">
          <h1 class="text-3xl font-bold text-gray-100">Temporal Analytics</h1>
          <select id="period-selector" class="form-select w-auto">
            <option value="day">Last 24 Hours</option>
            <option value="week" selected>Last Week</option>
            <option value="month">Last Month</option>
          </select>
        </div>

        <div id="temporal-content">
          <div class="flex items-center justify-center py-12">
            <div class="text-center">
              <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-500 mx-auto"></div>
              <p class="mt-4 text-gray-400">Loading temporal analytics...</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;

  // Set up event listener
  document.getElementById('period-selector').addEventListener('change', (e) => {
    loadTemporalAnalytics(serverId, e.target.value);
  });

  await loadTemporalAnalytics(serverId, 'week');
}

/**
 * Load temporal analytics data
 */
async function loadTemporalAnalytics(serverId, period) {
  const content = document.getElementById('temporal-content');

  try {
    const data = await api.getTemporalAnalytics(serverId, period);

    content.innerHTML = `
      <!-- Average Violations -->
      <div class="card mb-6">
        <div class="text-center">
          <p class="text-gray-400 text-sm mb-2">Average Violations Per Hour</p>
          <p class="text-5xl font-bold text-indigo-400">${data.avg_violations_per_hour.toFixed(1)}</p>
        </div>
      </div>

      <!-- Heatmap -->
      <div class="card mb-6">
        <h3 class="text-lg font-semibold text-gray-100 mb-4">Violation Heatmap</h3>
        <p class="text-sm text-gray-400 mb-4">Violations by day of week and hour of day</p>
        <div id="heatmap-container" class="overflow-x-auto">
          ${renderHeatmap(data.heatmap, data.peak_times)}
        </div>
      </div>

      <!-- Peak Times -->
      <div class="card mb-6">
        <h3 class="text-lg font-semibold text-gray-100 mb-4">Peak Violation Times</h3>
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          ${data.peak_times.slice(0, 3).map(peak => `
            <div class="bg-gray-800 p-4 rounded-lg">
              <p class="text-sm text-gray-400 mb-2">
                ${['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'][peak.day_of_week]}
                at ${peak.hour}:00
              </p>
              <p class="text-2xl font-bold text-red-400">${peak.violation_count}</p>
              <p class="text-xs text-gray-500">violations</p>
            </div>
          `).join('')}
        </div>
      </div>

      <!-- Major Events -->
      ${data.major_events.length > 0 ? `
        <div class="card">
          <h3 class="text-lg font-semibold text-gray-100 mb-4">Major Moderation Events</h3>
          <p class="text-sm text-gray-400 mb-4">Events with 10+ violations within 5 minutes</p>
          <div class="space-y-3">
            ${data.major_events.map(event => `
              <div class="bg-gray-800 p-4 rounded-lg flex items-center justify-between">
                <div>
                  <p class="font-semibold text-gray-100">${event.event_type}</p>
                  <p class="text-sm text-gray-400">${event.description}</p>
                  <p class="text-xs text-gray-500 mt-1">${new Date(event.timestamp).toLocaleString()}</p>
                </div>
                <div class="text-right">
                  <p class="text-2xl font-bold text-red-400">${event.violation_count}</p>
                  <p class="text-xs text-gray-500">violations</p>
                </div>
              </div>
            `).join('')}
          </div>
        </div>
      ` : `
        <div class="card text-center py-8">
          <p class="text-gray-400">No major moderation events detected</p>
          <p class="text-sm text-gray-500 mt-2">Major events are defined as 10+ violations within 5 minutes</p>
        </div>
      `}
    `;

  } catch (error) {
    console.error('Failed to load temporal analytics:', error);
    content.innerHTML = `
      <div class="error-message">
        <p class="font-semibold">Failed to load temporal analytics</p>
        <p class="text-sm mt-2">${error.message}</p>
      </div>
    `;
  }
}

/**
 * Render heatmap visualization
 */
function renderHeatmap(heatmapData, peakTimes) {
  const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const hours = Array.from({ length: 24 }, (_, i) => i);

  // Create a 2D array for the heatmap
  const grid = Array(7).fill(null).map(() => Array(24).fill(0));

  heatmapData.forEach(cell => {
    grid[cell.day_of_week][cell.hour] = cell.violation_count;
  });

  // Find max value for color scaling
  const maxValue = Math.max(...heatmapData.map(c => c.violation_count), 1);

  // Check if a cell is a peak time
  const isPeak = (day, hour) => {
    return peakTimes.some(p => p.day_of_week === day && p.hour === hour);
  };

  // Generate color based on value
  const getColor = (value) => {
    if (value === 0) return '#1f2937';
    const intensity = value / maxValue;
    if (intensity < 0.25) return '#374151';
    if (intensity < 0.5) return '#4b5563';
    if (intensity < 0.75) return '#f59e0b';
    return '#ef4444';
  };

  return `
    <div class="inline-block">
      <table class="border-collapse">
        <thead>
          <tr>
            <th class="p-2"></th>
            ${hours.map(h => `<th class="p-1 text-xs text-gray-500">${h}</th>`).join('')}
          </tr>
        </thead>
        <tbody>
          ${days.map((day, dayIndex) => `
            <tr>
              <td class="p-2 text-sm text-gray-400 font-medium">${day}</td>
              ${hours.map(hour => {
    const value = grid[dayIndex][hour];
    const color = getColor(value);
    const peak = isPeak(dayIndex, hour);
    return `
                  <td class="p-1">
                    <div 
                      class="w-8 h-8 rounded ${peak ? 'ring-2 ring-yellow-400' : ''} tooltip cursor-pointer"
                      style="background-color: ${color}"
                      title="${day} ${hour}:00 - ${value} violations"
                    >
                      ${peak ? '<span class="text-xs">⭐</span>' : ''}
                    </div>
                  </td>
                `;
  }).join('')}
            </tr>
          `).join('')}
        </tbody>
      </table>
      <div class="mt-4 flex items-center gap-4 text-sm text-gray-400">
        <span>Less</span>
        <div class="flex gap-1">
          <div class="w-6 h-6 rounded" style="background-color: #1f2937"></div>
          <div class="w-6 h-6 rounded" style="background-color: #374151"></div>
          <div class="w-6 h-6 rounded" style="background-color: #4b5563"></div>
          <div class="w-6 h-6 rounded" style="background-color: #f59e0b"></div>
          <div class="w-6 h-6 rounded" style="background-color: #ef4444"></div>
        </div>
        <span>More</span>
        <span class="ml-4">⭐ = Peak Time</span>
      </div>
    </div>
  `;
}

// Make router available globally
window.router = router;

// Start the app when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}

export { currentServer, autoRefreshInterval };
