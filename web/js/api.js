/**
 * API Client for Murdoch Dashboard
 * Handles all HTTP requests to the backend API with authentication and error handling
 */

class ApiError extends Error {
  constructor(message, status, data) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.data = data;
  }
}

class ApiClient {
  constructor(baseUrl = '') {
    this.baseUrl = baseUrl;
    this.loadingCallbacks = new Set();
  }

  /**
   * Register a callback to be notified of loading state changes
   */
  onLoadingChange(callback) {
    this.loadingCallbacks.add(callback);
    return () => this.loadingCallbacks.delete(callback);
  }

  /**
   * Notify all loading callbacks
   */
  notifyLoading(isLoading) {
    this.loadingCallbacks.forEach(cb => cb(isLoading));
  }

  /**
   * Core fetch wrapper with error handling and auth
   */
  async request(endpoint, options = {}) {
    const url = `${this.baseUrl}${endpoint}`;
    const config = {
      credentials: 'include',
      headers: {
        'Content-Type': 'application/json',
        ...options.headers,
      },
      ...options,
    };

    this.notifyLoading(true);

    try {
      const response = await fetch(url, config);

      if (response.status === 401) {
        window.location.href = '/api/auth/login';
        throw new ApiError('Unauthorized', 401, null);
      }

      if (!response.ok) {
        let errorData;
        try {
          errorData = await response.json();
        } catch {
          errorData = { error: response.statusText };
        }
        throw new ApiError(
          errorData.error || 'Request failed',
          response.status,
          errorData
        );
      }

      if (response.status === 204) {
        return null;
      }

      const contentType = response.headers.get('content-type');
      if (contentType && contentType.includes('application/json')) {
        return await response.json();
      }

      return await response.text();
    } finally {
      this.notifyLoading(false);
    }
  }

  /**
   * GET request
   */
  async get(endpoint, params = {}) {
    // Filter out empty/null/undefined values
    const filteredParams = Object.fromEntries(
      Object.entries(params).filter(([_, v]) => v !== '' && v !== null && v !== undefined)
    );
    const queryString = new URLSearchParams(filteredParams).toString();
    const url = queryString ? `${endpoint}?${queryString}` : endpoint;
    return this.request(url, { method: 'GET' });
  }

  /**
   * POST request
   */
  async post(endpoint, data = null) {
    return this.request(endpoint, {
      method: 'POST',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  /**
   * PUT request
   */
  async put(endpoint, data) {
    return this.request(endpoint, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  /**
   * DELETE request
   */
  async delete(endpoint) {
    return this.request(endpoint, { method: 'DELETE' });
  }

  // Authentication endpoints
  async getCurrentUser() {
    return this.get('/api/auth/me');
  }

  async logout() {
    return this.post('/api/auth/logout');
  }

  // Server endpoints
  async getServers() {
    return this.get('/api/servers');
  }

  async getServerMetrics(serverId, period = 'day') {
    return this.get(`/api/servers/${serverId}/metrics`, { period });
  }

  // Violations endpoints
  async getViolations(serverId, params = {}) {
    return this.get(`/api/servers/${serverId}/violations`, params);
  }

  async exportViolations(serverId, params = {}) {
    const queryString = new URLSearchParams(params).toString();
    const url = `/api/servers/${serverId}/violations/export${queryString ? '?' + queryString : ''}`;

    this.notifyLoading(true);
    try {
      const response = await fetch(url, {
        credentials: 'include',
      });

      if (!response.ok) {
        throw new ApiError('Export failed', response.status, null);
      }

      const blob = await response.blob();
      const downloadUrl = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = downloadUrl;
      a.download = `violations-${serverId}-${Date.now()}.csv`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      window.URL.revokeObjectURL(downloadUrl);
    } finally {
      this.notifyLoading(false);
    }
  }

  // Configuration endpoints
  async getConfig(serverId) {
    return this.get(`/api/servers/${serverId}/config`);
  }

  async updateConfig(serverId, config) {
    return this.put(`/api/servers/${serverId}/config`, config);
  }

  // Rules endpoints
  async getRules(serverId) {
    return this.get(`/api/servers/${serverId}/rules`);
  }

  async updateRules(serverId, rules) {
    return this.put(`/api/servers/${serverId}/rules`, { rules });
  }

  async deleteRules(serverId) {
    return this.delete(`/api/servers/${serverId}/rules`);
  }

  // Warnings endpoints
  async getWarnings(serverId, search = '') {
    return this.get(`/api/servers/${serverId}/warnings`, search ? { search } : {});
  }

  async getUserWarning(serverId, userId) {
    return this.get(`/api/servers/${serverId}/warnings/${userId}`);
  }

  async clearUserWarning(serverId, userId) {
    return this.delete(`/api/servers/${serverId}/warnings/${userId}`);
  }

  async bulkClearWarnings(serverId, olderThan) {
    return this.post(`/api/servers/${serverId}/warnings/bulk-clear`, { older_than: olderThan });
  }

  // Health metrics endpoints
  async getHealthMetrics(serverId) {
    return this.get(`/api/servers/${serverId}/health`);
  }

  // Top offenders endpoints
  async getTopOffenders(serverId, period = 'week') {
    return this.get(`/api/servers/${serverId}/top-offenders`, { period });
  }

  // Rule effectiveness endpoints
  async getRuleEffectiveness(serverId, period = 'week') {
    return this.get(`/api/servers/${serverId}/rule-effectiveness`, { period });
  }

  // Temporal analytics endpoints
  async getTemporalAnalytics(serverId, period = 'week') {
    return this.get(`/api/servers/${serverId}/temporal-analytics`, { period });
  }
}

// Create and export singleton instance
const api = new ApiClient();

export { api, ApiError };
