/**
 * WebSocket Client for Murdoch Dashboard
 * Handles real-time updates via WebSocket connection
 */

class WebSocketClient {
  constructor() {
    this.ws = null;
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = Infinity;
    this.reconnectDelay = 1000; // Start at 1 second
    this.maxReconnectDelay = 60000; // Max 60 seconds
    this.reconnectTimer = null;
    this.isConnecting = false;
    this.isIntentionallyClosed = false;
    this.subscribedGuilds = new Set();
    this.eventHandlers = new Map();
    this.connectionStateCallbacks = new Set();
    this.pingInterval = null;
    this.pongTimeout = null;
  }

  /**
   * Register a callback for connection state changes
   * @param {Function} callback - Called with (state) where state is 'connected', 'connecting', 'disconnected', 'reconnecting'
   * @returns {Function} Unsubscribe function
   */
  onConnectionStateChange(callback) {
    this.connectionStateCallbacks.add(callback);
    return () => this.connectionStateCallbacks.delete(callback);
  }

  /**
   * Notify all connection state callbacks
   * @param {string} state - Connection state
   */
  notifyConnectionState(state) {
    this.connectionStateCallbacks.forEach(cb => {
      try {
        cb(state);
      } catch (error) {
        console.error('Error in connection state callback:', error);
      }
    });
  }

  /**
   * Connect to the WebSocket server
   * @param {string} guildId - Guild ID to subscribe to
   */
  connect(guildId) {
    if (this.isConnecting || (this.ws && this.ws.readyState === WebSocket.OPEN)) {
      // Already connected or connecting
      if (guildId && !this.subscribedGuilds.has(guildId)) {
        this.subscribe(guildId);
      }
      return;
    }

    this.isConnecting = true;
    this.isIntentionallyClosed = false;
    this.notifyConnectionState('connecting');

    try {
      // Construct WebSocket URL
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const host = window.location.host;
      const wsUrl = `${protocol}//${host}/ws`;

      console.log('Connecting to WebSocket:', wsUrl);
      this.ws = new WebSocket(wsUrl);

      this.ws.onopen = () => {
        console.log('WebSocket connected');
        this.isConnecting = false;
        this.reconnectAttempts = 0;
        this.reconnectDelay = 1000;
        this.notifyConnectionState('connected');

        // Subscribe to guild if provided
        if (guildId) {
          this.subscribe(guildId);
        }

        // Start ping/pong keepalive
        this.startPingInterval();
      };

      this.ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          this.handleMessage(message);
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error, event.data);
        }
      };

      this.ws.onerror = (error) => {
        console.error('WebSocket error:', error);
      };

      this.ws.onclose = (event) => {
        console.log('WebSocket closed:', event.code, event.reason);
        this.isConnecting = false;
        this.stopPingInterval();

        // Clear subscriptions
        this.subscribedGuilds.clear();

        if (this.isIntentionallyClosed) {
          this.notifyConnectionState('disconnected');
          return;
        }

        // Handle authentication failure (code 4001)
        if (event.code === 4001) {
          console.error('WebSocket authentication failed');
          this.notifyConnectionState('disconnected');
          // Redirect to login
          window.location.href = '/api/auth/login';
          return;
        }

        // Attempt reconnection
        this.notifyConnectionState('reconnecting');
        this.scheduleReconnect(guildId);
      };

    } catch (error) {
      console.error('Failed to create WebSocket connection:', error);
      this.isConnecting = false;
      this.notifyConnectionState('disconnected');
      this.scheduleReconnect(guildId);
    }
  }

  /**
   * Schedule a reconnection attempt with exponential backoff
   * @param {string} guildId - Guild ID to reconnect to
   */
  scheduleReconnect(guildId) {
    if (this.isIntentionallyClosed) {
      return;
    }

    // Clear any existing reconnect timer
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
    }

    // Calculate delay with exponential backoff
    const delay = Math.min(
      this.reconnectDelay * Math.pow(2, this.reconnectAttempts),
      this.maxReconnectDelay
    );

    console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts + 1})`);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectAttempts++;
      this.connect(guildId);
    }, delay);
  }

  /**
   * Start ping interval to keep connection alive
   */
  startPingInterval() {
    // Send ping every 30 seconds
    this.pingInterval = setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.send({ type: 'ping' });

        // Set timeout for pong response
        this.pongTimeout = setTimeout(() => {
          console.warn('Pong timeout - closing connection');
          this.ws.close();
        }, 30000); // 30 second timeout
      }
    }, 30000);
  }

  /**
   * Stop ping interval
   */
  stopPingInterval() {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
    if (this.pongTimeout) {
      clearTimeout(this.pongTimeout);
      this.pongTimeout = null;
    }
  }

  /**
   * Subscribe to events for a specific guild
   * @param {string} guildId - Guild ID to subscribe to
   */
  subscribe(guildId) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.warn('Cannot subscribe - WebSocket not connected');
      return;
    }

    if (this.subscribedGuilds.has(guildId)) {
      return;
    }

    console.log('Subscribing to guild:', guildId);
    this.send({
      type: 'subscribe',
      guild_id: guildId
    });

    this.subscribedGuilds.add(guildId);
  }

  /**
   * Unsubscribe from events for a specific guild
   * @param {string} guildId - Guild ID to unsubscribe from
   */
  unsubscribe(guildId) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return;
    }

    if (!this.subscribedGuilds.has(guildId)) {
      return;
    }

    console.log('Unsubscribing from guild:', guildId);
    this.send({
      type: 'unsubscribe',
      guild_id: guildId
    });

    this.subscribedGuilds.delete(guildId);
  }

  /**
   * Send a message to the WebSocket server
   * @param {Object} message - Message to send
   */
  send(message) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.warn('Cannot send message - WebSocket not connected');
      return;
    }

    try {
      this.ws.send(JSON.stringify(message));
    } catch (error) {
      console.error('Failed to send WebSocket message:', error);
    }
  }

  /**
   * Handle incoming WebSocket message
   * @param {Object} message - Parsed message object
   */
  handleMessage(message) {
    // Handle pong response (server sends lowercase 'pong')
    if (message.type === 'pong' || message.type === 'Pong') {
      if (this.pongTimeout) {
        clearTimeout(this.pongTimeout);
        this.pongTimeout = null;
      }
      return;
    }

    // Dispatch to registered event handlers
    const eventType = message.type || this.inferEventType(message);
    const handlers = this.eventHandlers.get(eventType);

    if (handlers && handlers.size > 0) {
      handlers.forEach(handler => {
        try {
          handler(message);
        } catch (error) {
          console.error(`Error in ${eventType} handler:`, error);
        }
      });
    } else {
      console.log('Received WebSocket message:', eventType, message);
    }
  }

  /**
   * Infer event type from message structure
   * @param {Object} message - Message object
   * @returns {string} Event type
   */
  inferEventType(message) {
    if (message.Violation) return 'Violation';
    if (message.MetricsUpdate) return 'MetricsUpdate';
    if (message.ConfigUpdate) return 'ConfigUpdate';
    if (message.HealthUpdate) return 'HealthUpdate';
    return 'Unknown';
  }

  /**
   * Register an event handler
   * @param {string} eventType - Type of event to handle
   * @param {Function} handler - Handler function
   * @returns {Function} Unsubscribe function
   */
  on(eventType, handler) {
    if (!this.eventHandlers.has(eventType)) {
      this.eventHandlers.set(eventType, new Set());
    }

    this.eventHandlers.get(eventType).add(handler);

    // Return unsubscribe function
    return () => {
      const handlers = this.eventHandlers.get(eventType);
      if (handlers) {
        handlers.delete(handler);
      }
    };
  }

  /**
   * Remove an event handler
   * @param {string} eventType - Type of event
   * @param {Function} handler - Handler function to remove
   */
  off(eventType, handler) {
    const handlers = this.eventHandlers.get(eventType);
    if (handlers) {
      handlers.delete(handler);
    }
  }

  /**
   * Close the WebSocket connection
   */
  disconnect() {
    this.isIntentionallyClosed = true;

    // Clear reconnect timer
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    // Stop ping interval
    this.stopPingInterval();

    // Close WebSocket
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    // Clear subscriptions
    this.subscribedGuilds.clear();

    this.notifyConnectionState('disconnected');
  }

  /**
   * Get current connection state
   * @returns {string} Connection state
   */
  getConnectionState() {
    if (!this.ws) {
      return 'disconnected';
    }

    switch (this.ws.readyState) {
      case WebSocket.CONNECTING:
        return 'connecting';
      case WebSocket.OPEN:
        return 'connected';
      case WebSocket.CLOSING:
      case WebSocket.CLOSED:
        return this.reconnectTimer ? 'reconnecting' : 'disconnected';
      default:
        return 'disconnected';
    }
  }

  /**
   * Check if WebSocket is connected
   * @returns {boolean} True if connected
   */
  isConnected() {
    return this.ws && this.ws.readyState === WebSocket.OPEN;
  }
}

// Create and export singleton instance
const wsClient = new WebSocketClient();

export { wsClient, WebSocketClient };
