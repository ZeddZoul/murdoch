/**
 * Client-Side Router for Murdoch Dashboard
 * Hash-based routing with authentication guards
 */

import { auth } from './auth.js';

class Router {
  constructor() {
    this.routes = new Map();
    this.currentRoute = null;
    this.defaultRoute = '/';
    this.notFoundHandler = null;
    this.beforeEachGuards = [];
    this.afterEachHooks = [];
  }

  /**
   * Register a route with optional authentication guard
   */
  register(path, handler, options = {}) {
    this.routes.set(path, {
      handler,
      requiresAuth: options.requiresAuth || false,
      meta: options.meta || {},
    });
    return this;
  }

  /**
   * Register a global navigation guard that runs before each route
   */
  beforeEach(guard) {
    this.beforeEachGuards.push(guard);
    return this;
  }

  /**
   * Register a hook that runs after each route
   */
  afterEach(hook) {
    this.afterEachHooks.push(hook);
    return this;
  }

  /**
   * Set the 404 handler
   */
  notFound(handler) {
    this.notFoundHandler = handler;
    return this;
  }

  /**
   * Navigate to a route
   */
  navigate(path, replace = false) {
    if (replace) {
      window.location.replace(`#${path}`);
    } else {
      window.location.hash = path;
    }
  }

  /**
   * Get current route path from hash
   */
  getCurrentPath() {
    const hash = window.location.hash.slice(1);
    return hash || this.defaultRoute;
  }

  /**
   * Parse route parameters from path
   * Supports patterns like /servers/:id/metrics
   */
  matchRoute(path) {
    // Try exact match first
    if (this.routes.has(path)) {
      return { route: this.routes.get(path), params: {} };
    }

    // Try pattern matching
    for (const [pattern, route] of this.routes.entries()) {
      const params = this.extractParams(pattern, path);
      if (params !== null) {
        return { route, params };
      }
    }

    return null;
  }

  /**
   * Extract parameters from a path pattern
   * Pattern: /servers/:id/metrics
   * Path: /servers/123/metrics
   * Returns: { id: '123' }
   */
  extractParams(pattern, path) {
    const patternParts = pattern.split('/').filter(Boolean);
    const pathParts = path.split('/').filter(Boolean);

    if (patternParts.length !== pathParts.length) {
      return null;
    }

    const params = {};
    for (let i = 0; i < patternParts.length; i++) {
      const patternPart = patternParts[i];
      const pathPart = pathParts[i];

      if (patternPart.startsWith(':')) {
        const paramName = patternPart.slice(1);
        params[paramName] = pathPart;
      } else if (patternPart !== pathPart) {
        return null;
      }
    }

    return params;
  }

  /**
   * Handle route change
   */
  async handleRoute() {
    const path = this.getCurrentPath();
    const match = this.matchRoute(path);

    if (!match) {
      if (this.notFoundHandler) {
        this.notFoundHandler();
      }
      return;
    }

    const { route, params } = match;
    const to = { path, params, meta: route.meta };
    const from = this.currentRoute;

    // Run beforeEach guards
    for (const guard of this.beforeEachGuards) {
      const result = await guard(to, from);
      if (result === false) {
        // Guard blocked navigation
        return;
      }
      if (typeof result === 'string') {
        // Guard redirected to another route
        this.navigate(result, true);
        return;
      }
    }

    // Check authentication requirement
    if (route.requiresAuth) {
      const canProceed = await auth.requireAuth();
      if (!canProceed) {
        return;
      }
    }

    // Execute route handler
    try {
      await route.handler(params, to);
      this.currentRoute = to;

      // Run afterEach hooks
      for (const hook of this.afterEachHooks) {
        hook(to, from);
      }
    } catch (error) {
      console.error('Route handler error:', error);
      if (this.notFoundHandler) {
        this.notFoundHandler();
      }
    }
  }

  /**
   * Initialize the router
   */
  init() {
    // Handle initial route
    this.handleRoute();

    // Listen for hash changes
    window.addEventListener('hashchange', () => {
      this.handleRoute();
    });

    // Handle browser back/forward
    window.addEventListener('popstate', () => {
      this.handleRoute();
    });
  }

  /**
   * Get query parameters from current URL
   */
  getQueryParams() {
    const hash = window.location.hash;
    const queryStart = hash.indexOf('?');
    if (queryStart === -1) {
      return {};
    }

    const queryString = hash.slice(queryStart + 1);
    const params = new URLSearchParams(queryString);
    const result = {};
    for (const [key, value] of params.entries()) {
      result[key] = value;
    }
    return result;
  }

  /**
   * Navigate with query parameters
   */
  navigateWithQuery(path, queryParams = {}) {
    const queryString = new URLSearchParams(queryParams).toString();
    const fullPath = queryString ? `${path}?${queryString}` : path;
    this.navigate(fullPath);
  }

  /**
   * Go back in history
   */
  back() {
    window.history.back();
  }

  /**
   * Go forward in history
   */
  forward() {
    window.history.forward();
  }
}

// Create and export singleton instance
const router = new Router();

export { router };
