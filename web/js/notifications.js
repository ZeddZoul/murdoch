/**
 * Notification Center for Murdoch Dashboard
 * Manages in-app notifications with read/unread functionality
 */

import { api } from './api.js';

class NotificationCenter {
  constructor() {
    this.notifications = [];
    this.unreadCount = 0;
    this.isOpen = false;
    this.limit = 50;
  }

  /**
   * Initialize the notification center
   */
  async init() {
    // Create notification bell icon in navbar
    this.createBellIcon();

    // Load initial notifications
    const serverId = sessionStorage.getItem('selectedServerId');
    if (serverId) {
      await this.loadNotifications(serverId);
    }
  }

  /**
   * Create notification bell icon in navbar
   */
  createBellIcon() {
    const navbar = document.querySelector('nav .flex.items-center.gap-4');
    if (!navbar) {
      console.warn('Navbar not found, cannot add notification bell');
      return;
    }

    const bellContainer = document.createElement('div');
    bellContainer.className = 'relative';
    bellContainer.innerHTML = `
      <button id="notification-bell" class="relative p-2 text-gray-400 hover:text-gray-200 transition-colors">
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
        </svg>
        <span id="notification-badge" class="hidden absolute top-0 right-0 inline-flex items-center justify-center px-2 py-1 text-xs font-bold leading-none text-white bg-red-500 rounded-full"></span>
      </button>
    `;

    // Insert before theme toggle
    const themeToggle = navbar.querySelector('#theme-toggle');
    if (themeToggle) {
      navbar.insertBefore(bellContainer, themeToggle);
    } else {
      navbar.appendChild(bellContainer);
    }

    // Add click handler
    const bell = document.getElementById('notification-bell');
    bell.addEventListener('click', () => this.togglePanel());
  }

  /**
   * Toggle notification panel
   */
  togglePanel() {
    if (this.isOpen) {
      this.closePanel();
    } else {
      this.openPanel();
    }
  }

  /**
   * Open notification panel
   */
  openPanel() {
    this.isOpen = true;

    // Create panel if it doesn't exist
    let panel = document.getElementById('notification-panel');
    if (!panel) {
      panel = document.createElement('div');
      panel.id = 'notification-panel';
      panel.className = 'fixed top-16 right-4 w-96 max-h-[600px] bg-gray-800 border border-gray-700 rounded-lg shadow-xl z-50 flex flex-col';
      document.body.appendChild(panel);
    }

    // Render panel content
    this.renderPanel();

    // Add click outside handler
    setTimeout(() => {
      document.addEventListener('click', this.handleClickOutside);
    }, 0);
  }

  /**
   * Close notification panel
   */
  closePanel() {
    this.isOpen = false;
    const panel = document.getElementById('notification-panel');
    if (panel) {
      panel.remove();
    }
    document.removeEventListener('click', this.handleClickOutside);
  }

  /**
   * Handle click outside panel
   */
  handleClickOutside = (e) => {
    const panel = document.getElementById('notification-panel');
    const bell = document.getElementById('notification-bell');

    if (panel && !panel.contains(e.target) && !bell.contains(e.target)) {
      this.closePanel();
    }
  }

  /**
   * Render notification panel
   */
  renderPanel() {
    const panel = document.getElementById('notification-panel');
    if (!panel) return;

    const hasNotifications = this.notifications.length > 0;

    panel.innerHTML = `
      <div class="p-4 border-b border-gray-700 flex items-center justify-between">
        <h3 class="text-lg font-semibold text-gray-100">Notifications</h3>
        <div class="flex items-center gap-2">
          <button id="notification-preferences-btn" class="text-sm text-gray-400 hover:text-gray-200" title="Notification Preferences">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
          </button>
          ${this.unreadCount > 0 ? `
            <button id="mark-all-read" class="text-sm text-blue-400 hover:text-blue-300">
              Mark all read
            </button>
          ` : ''}
        </div>
      </div>
      <div class="flex-1 overflow-y-auto">
        ${hasNotifications ? this.renderNotifications() : this.renderEmptyState()}
      </div>
    `;

    // Add event listeners
    const prefsBtn = document.getElementById('notification-preferences-btn');
    prefsBtn?.addEventListener('click', () => this.showPreferencesModal());

    if (this.unreadCount > 0) {
      const markAllBtn = document.getElementById('mark-all-read');
      markAllBtn?.addEventListener('click', () => this.markAllAsRead());
    }

    // Add click handlers for individual notifications
    this.notifications.forEach((notification, index) => {
      const notifEl = document.getElementById(`notification-${index}`);
      if (notifEl) {
        notifEl.addEventListener('click', () => this.handleNotificationClick(notification));
      }
    });
  }

  /**
   * Render notifications list
   */
  renderNotifications() {
    return this.notifications.map((notification, index) => {
      const priorityColors = {
        'low': 'text-gray-400',
        'medium': 'text-blue-400',
        'high': 'text-yellow-400',
        'critical': 'text-red-400'
      };

      const priorityColor = priorityColors[notification.priority] || 'text-gray-400';
      const isUnread = !notification.read;

      return `
        <div id="notification-${index}" class="p-4 border-b border-gray-700 hover:bg-gray-750 cursor-pointer transition-colors ${isUnread ? 'bg-gray-750' : ''}">
          <div class="flex items-start gap-3">
            <div class="flex-shrink-0">
              <div class="${priorityColor}">
                <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
                  <circle cx="10" cy="10" r="8" />
                </svg>
              </div>
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-start justify-between gap-2">
                <p class="font-semibold text-gray-100 ${isUnread ? 'font-bold' : ''}">${this.escapeHtml(notification.title)}</p>
                ${isUnread ? '<span class="flex-shrink-0 w-2 h-2 bg-blue-500 rounded-full"></span>' : ''}
              </div>
              <p class="text-sm text-gray-400 mt-1">${this.escapeHtml(notification.message)}</p>
              <p class="text-xs text-gray-500 mt-2">${this.formatTimestamp(notification.created_at)}</p>
            </div>
          </div>
        </div>
      `;
    }).join('');
  }

  /**
   * Render empty state
   */
  renderEmptyState() {
    return `
      <div class="flex flex-col items-center justify-center h-64 text-gray-400">
        <svg class="w-16 h-16 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
        </svg>
        <p class="text-lg font-medium">No notifications</p>
        <p class="text-sm mt-1">You're all caught up!</p>
      </div>
    `;
  }

  /**
   * Load notifications from API
   */
  async loadNotifications(serverId) {
    try {
      const response = await api.get(`/api/servers/${serverId}/notifications?limit=${this.limit}`);
      this.notifications = response.notifications || [];
      this.updateUnreadCount();
      this.updateBadge();

      // Refresh panel if open
      if (this.isOpen) {
        this.renderPanel();
      }
    } catch (error) {
      console.error('Failed to load notifications:', error);
    }
  }

  /**
   * Handle notification click
   */
  async handleNotificationClick(notification) {
    // Mark as read if unread
    if (!notification.read) {
      await this.markAsRead(notification.id);
    }

    // Close panel
    this.closePanel();

    // Navigate to link if available
    if (notification.link) {
      window.location.hash = notification.link;
    }
  }

  /**
   * Mark notification as read
   */
  async markAsRead(notificationId) {
    try {
      const serverId = sessionStorage.getItem('selectedServerId');
      await api.post(`/api/servers/${serverId}/notifications/${notificationId}/read`);

      // Update local state
      const notification = this.notifications.find(n => n.id === notificationId);
      if (notification) {
        notification.read = true;
        this.updateUnreadCount();
        this.updateBadge();
        this.renderPanel();
      }
    } catch (error) {
      console.error('Failed to mark notification as read:', error);
    }
  }

  /**
   * Mark notification as unread
   */
  async markAsUnread(notificationId) {
    try {
      const serverId = sessionStorage.getItem('selectedServerId');
      await api.post(`/api/servers/${serverId}/notifications/${notificationId}/unread`);

      // Update local state
      const notification = this.notifications.find(n => n.id === notificationId);
      if (notification) {
        notification.read = false;
        this.updateUnreadCount();
        this.updateBadge();
        this.renderPanel();
      }
    } catch (error) {
      console.error('Failed to mark notification as unread:', error);
    }
  }

  /**
   * Mark all notifications as read
   */
  async markAllAsRead() {
    const unreadNotifications = this.notifications.filter(n => !n.read);

    for (const notification of unreadNotifications) {
      await this.markAsRead(notification.id);
    }
  }

  /**
   * Add a new notification (called from WebSocket)
   */
  addNotification(notification) {
    // Add to beginning of list
    this.notifications.unshift(notification);

    // Keep only last 50
    if (this.notifications.length > this.limit) {
      this.notifications = this.notifications.slice(0, this.limit);
    }

    this.updateUnreadCount();
    this.updateBadge();

    // Refresh panel if open
    if (this.isOpen) {
      this.renderPanel();
    }
  }

  /**
   * Update unread count
   */
  updateUnreadCount() {
    this.unreadCount = this.notifications.filter(n => !n.read).length;
  }

  /**
   * Update notification badge
   */
  updateBadge() {
    const badge = document.getElementById('notification-badge');
    if (!badge) return;

    if (this.unreadCount > 0) {
      badge.textContent = this.unreadCount > 99 ? '99+' : this.unreadCount;
      badge.classList.remove('hidden');
    } else {
      badge.classList.add('hidden');
    }
  }

  /**
   * Format timestamp for display
   */
  formatTimestamp(timestamp) {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now - date;
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) {
      return 'Just now';
    } else if (diffMins < 60) {
      return `${diffMins} minute${diffMins > 1 ? 's' : ''} ago`;
    } else if (diffHours < 24) {
      return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
    } else if (diffDays < 7) {
      return `${diffDays} day${diffDays > 1 ? 's' : ''} ago`;
    } else {
      return date.toLocaleDateString();
    }
  }

  /**
   * Escape HTML to prevent XSS
   */
  escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  /**
   * Refresh notifications for current server
   */
  async refresh() {
    const serverId = sessionStorage.getItem('selectedServerId');
    if (serverId) {
      await this.loadNotifications(serverId);
    }
  }

  /**
   * Show notification preferences modal
   */
  async showPreferencesModal() {
    const serverId = sessionStorage.getItem('selectedServerId');
    if (!serverId) return;

    // Close notification panel
    this.closePanel();

    // Load current preferences
    let preferences;
    try {
      preferences = await api.get(`/api/servers/${serverId}/notification-preferences`);
    } catch (error) {
      console.error('Failed to load preferences:', error);
      window.showToast('Error', 'Failed to load notification preferences', 'error');
      return;
    }

    // Create modal
    const modal = document.createElement('div');
    modal.id = 'notification-preferences-modal';
    modal.className = 'fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50';
    modal.innerHTML = `
      <div class="bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div class="p-6 border-b border-gray-700">
          <h2 class="text-2xl font-bold text-gray-100">Notification Preferences</h2>
        </div>
        <div class="p-6 space-y-6">
          <!-- Discord Webhook -->
          <div>
            <label class="block text-sm font-medium text-gray-300 mb-2">Discord Webhook URL</label>
            <input type="url" id="webhook-url" value="${preferences.discord_webhook_url || ''}" 
              class="w-full px-4 py-2 bg-gray-700 border border-gray-600 rounded-lg text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="https://discord.com/api/webhooks/...">
            <p class="text-xs text-gray-400 mt-1">Receive notifications via Discord webhook</p>
          </div>

          <!-- Notification Threshold -->
          <div>
            <label class="block text-sm font-medium text-gray-300 mb-2">Notification Threshold</label>
            <select id="notification-threshold" class="w-full px-4 py-2 bg-gray-700 border border-gray-600 rounded-lg text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500">
              <option value="low" ${preferences.notification_threshold === 'low' ? 'selected' : ''}>Low - All notifications</option>
              <option value="medium" ${preferences.notification_threshold === 'medium' ? 'selected' : ''}>Medium - Important notifications</option>
              <option value="high" ${preferences.notification_threshold === 'high' ? 'selected' : ''}>High - Critical notifications only</option>
              <option value="critical" ${preferences.notification_threshold === 'critical' ? 'selected' : ''}>Critical - Emergency notifications only</option>
            </select>
            <p class="text-xs text-gray-400 mt-1">Minimum priority level for notifications</p>
          </div>

          <!-- Enabled Events -->
          <div>
            <label class="block text-sm font-medium text-gray-300 mb-2">Enabled Events</label>
            <div class="space-y-2">
              <label class="flex items-center">
                <input type="checkbox" id="event-health-score" ${preferences.enabled_events.includes('health_score_drop') ? 'checked' : ''}
                  class="w-4 h-4 text-blue-600 bg-gray-700 border-gray-600 rounded focus:ring-blue-500">
                <span class="ml-2 text-gray-300">Health Score Drops Below 50</span>
              </label>
              <label class="flex items-center">
                <input type="checkbox" id="event-mass-violations" ${preferences.enabled_events.includes('mass_violations') ? 'checked' : ''}
                  class="w-4 h-4 text-blue-600 bg-gray-700 border-gray-600 rounded focus:ring-blue-500">
                <span class="ml-2 text-gray-300">Mass Violations (10+ in 60 seconds)</span>
              </label>
              <label class="flex items-center">
                <input type="checkbox" id="event-bot-offline" ${preferences.enabled_events.includes('bot_offline') ? 'checked' : ''}
                  class="w-4 h-4 text-blue-600 bg-gray-700 border-gray-600 rounded focus:ring-blue-500">
                <span class="ml-2 text-gray-300">Bot Offline Detection</span>
              </label>
              <label class="flex items-center">
                <input type="checkbox" id="event-new-violation" ${preferences.enabled_events.includes('new_violation') ? 'checked' : ''}
                  class="w-4 h-4 text-blue-600 bg-gray-700 border-gray-600 rounded focus:ring-blue-500">
                <span class="ml-2 text-gray-300">New Violations</span>
              </label>
              <label class="flex items-center">
                <input type="checkbox" id="event-config-update" ${preferences.enabled_events.includes('config_update') ? 'checked' : ''}
                  class="w-4 h-4 text-blue-600 bg-gray-700 border-gray-600 rounded focus:ring-blue-500">
                <span class="ml-2 text-gray-300">Configuration Updates</span>
              </label>
            </div>
          </div>

          <!-- Mute Notifications -->
          <div>
            <label class="block text-sm font-medium text-gray-300 mb-2">Mute Notifications</label>
            <select id="mute-duration" class="w-full px-4 py-2 bg-gray-700 border border-gray-600 rounded-lg text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500">
              <option value="0">Not muted</option>
              <option value="1">Mute for 1 hour</option>
              <option value="4">Mute for 4 hours</option>
              <option value="8">Mute for 8 hours</option>
              <option value="24">Mute for 24 hours</option>
            </select>
            <p class="text-xs text-gray-400 mt-1">Temporarily disable all notifications</p>
          </div>
        </div>
        <div class="p-6 border-t border-gray-700 flex justify-end gap-3">
          <button id="cancel-preferences" class="px-4 py-2 text-gray-300 hover:text-gray-100">Cancel</button>
          <button id="save-preferences" class="btn btn-primary">Save Preferences</button>
        </div>
      </div>
    `;

    document.body.appendChild(modal);

    // Add event listeners
    document.getElementById('cancel-preferences').addEventListener('click', () => modal.remove());
    document.getElementById('save-preferences').addEventListener('click', async () => {
      await this.savePreferences(serverId);
      modal.remove();
    });

    // Click outside to close
    modal.addEventListener('click', (e) => {
      if (e.target === modal) {
        modal.remove();
      }
    });
  }

  /**
   * Save notification preferences
   */
  async savePreferences(serverId) {
    const webhookUrl = document.getElementById('webhook-url').value.trim();
    const threshold = document.getElementById('notification-threshold').value;
    const muteDuration = parseInt(document.getElementById('mute-duration').value);

    // Collect enabled events
    const enabledEvents = [];
    if (document.getElementById('event-health-score').checked) {
      enabledEvents.push('health_score_drop');
    }
    if (document.getElementById('event-mass-violations').checked) {
      enabledEvents.push('mass_violations');
    }
    if (document.getElementById('event-bot-offline').checked) {
      enabledEvents.push('bot_offline');
    }
    if (document.getElementById('event-new-violation').checked) {
      enabledEvents.push('new_violation');
    }
    if (document.getElementById('event-config-update').checked) {
      enabledEvents.push('config_update');
    }

    // Calculate muted_until
    let mutedUntil = null;
    if (muteDuration > 0) {
      const now = new Date();
      now.setHours(now.getHours() + muteDuration);
      mutedUntil = now.toISOString();
    }

    const preferences = {
      guild_id: parseInt(serverId),
      discord_webhook_url: webhookUrl || null,
      notification_threshold: threshold,
      enabled_events: enabledEvents,
      muted_until: mutedUntil
    };

    try {
      await api.put(`/api/servers/${serverId}/notification-preferences`, preferences);
      window.showToast('Success', 'Notification preferences saved', 'success');
    } catch (error) {
      console.error('Failed to save preferences:', error);
      window.showToast('Error', 'Failed to save notification preferences', 'error');
    }
  }
}

// Create and export singleton instance
const notificationCenter = new NotificationCenter();

export { notificationCenter, NotificationCenter };

