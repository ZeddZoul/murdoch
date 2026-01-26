/**
 * Authentication Handler for Murdoch Dashboard
 * Manages user sessions, login redirects, and logout
 */

import { api } from './api.js';

class AuthManager {
  constructor() {
    this.currentUser = null;
    this.isAuthenticated = false;
    this.isChecking = false;
    this.authCallbacks = new Set();
  }

  /**
   * Register a callback to be notified of auth state changes
   */
  onAuthChange(callback) {
    this.authCallbacks.add(callback);
    return () => this.authCallbacks.delete(callback);
  }

  /**
   * Notify all auth callbacks
   */
  notifyAuthChange() {
    this.authCallbacks.forEach(cb => cb(this.isAuthenticated, this.currentUser));
  }

  /**
   * Check if user is authenticated by fetching current user
   * This is called on app load to verify session
   */
  async checkAuth() {
    if (this.isChecking) {
      return this.isAuthenticated;
    }

    this.isChecking = true;

    try {
      const user = await api.getCurrentUser();
      this.currentUser = user;
      this.isAuthenticated = true;
      this.notifyAuthChange();
      return true;
    } catch (error) {
      // If we get a 401, the session is invalid
      if (error.status === 401) {
        this.currentUser = null;
        this.isAuthenticated = false;
        this.notifyAuthChange();
        return false;
      }
      // For other errors, we can't determine auth state
      console.error('Auth check failed:', error);
      throw error;
    } finally {
      this.isChecking = false;
    }
  }

  /**
   * Redirect to Discord OAuth login
   */
  login() {
    window.location.href = '/api/auth/login';
  }

  /**
   * Logout the current user
   */
  async logout() {
    try {
      await api.logout();
    } catch (error) {
      console.error('Logout failed:', error);
    } finally {
      // Clear local state regardless of API call result
      this.currentUser = null;
      this.isAuthenticated = false;
      this.notifyAuthChange();

      // Redirect to login
      this.login();
    }
  }

  /**
   * Require authentication for a route
   * Returns true if authenticated, false if redirected to login
   */
  async requireAuth() {
    if (this.isAuthenticated) {
      return true;
    }

    // Check if we have a valid session
    try {
      const authenticated = await this.checkAuth();
      if (authenticated) {
        return true;
      }
    } catch (error) {
      console.error('Auth check failed:', error);
    }

    // Not authenticated, redirect to login
    this.login();
    return false;
  }

  /**
   * Get the current user
   */
  getUser() {
    return this.currentUser;
  }

  /**
   * Check if user is authenticated (synchronous)
   */
  isLoggedIn() {
    return this.isAuthenticated;
  }
}

// Create and export singleton instance
const auth = new AuthManager();

export { auth };
