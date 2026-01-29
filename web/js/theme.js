/**
 * Theme Management Module
 * Handles theme switching between dark and light modes
 */

const THEME_KEY = 'murdoch-theme';
const THEMES = {
  DARK: 'dark',
  LIGHT: 'light'
};

class ThemeManager {
  constructor() {
    this.currentTheme = this.loadTheme();
    this.init();
  }

  /**
   * Initialize theme system
   */
  init() {
    this.applyTheme(this.currentTheme);
    this.setupEventListeners();
  }

  /**
   * Load theme from localStorage or detect system preference
   * @returns {string} Theme name ('dark' or 'light')
   */
  loadTheme() {
    const savedTheme = localStorage.getItem(THEME_KEY);

    if (savedTheme && (savedTheme === THEMES.DARK || savedTheme === THEMES.LIGHT)) {
      return savedTheme;
    }

    return this.detectSystemTheme();
  }

  /**
   * Detect system theme preference using prefers-color-scheme
   * @returns {string} Theme name ('dark' or 'light')
   */
  detectSystemTheme() {
    if (window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches) {
      return THEMES.LIGHT;
    }
    return THEMES.DARK;
  }

  /**
   * Apply theme to the document
   * @param {string} theme - Theme name ('dark' or 'light')
   */
  applyTheme(theme) {
    document.documentElement.setAttribute('data-theme', theme);
    this.currentTheme = theme;
    this.updateThemeIcons();
    this.updateChartColors();
  }

  /**
   * Toggle between dark and light themes
   */
  toggleTheme() {
    const newTheme = this.currentTheme === THEMES.DARK ? THEMES.LIGHT : THEMES.DARK;
    this.setTheme(newTheme);
  }

  /**
   * Set specific theme
   * @param {string} theme - Theme name ('dark' or 'light')
   */
  setTheme(theme) {
    if (theme !== THEMES.DARK && theme !== THEMES.LIGHT) {
      console.error('Invalid theme:', theme);
      return;
    }

    this.applyTheme(theme);
    localStorage.setItem(THEME_KEY, theme);
  }

  /**
   * Get current theme
   * @returns {string} Current theme name
   */
  getTheme() {
    return this.currentTheme;
  }

  /**
   * Check if current theme is dark
   * @returns {boolean} True if dark theme is active
   */
  isDark() {
    return this.currentTheme === THEMES.DARK;
  }

  /**
   * Update theme toggle button icons
   */
  updateThemeIcons() {
    const sunIcon = document.querySelector('.theme-icon-sun');
    const moonIcon = document.querySelector('.theme-icon-moon');

    if (!sunIcon || !moonIcon) return;

    if (this.currentTheme === THEMES.DARK) {
      sunIcon.classList.add('hidden');
      moonIcon.classList.remove('hidden');
    } else {
      sunIcon.classList.remove('hidden');
      moonIcon.classList.add('hidden');
    }
  }

  /**
   * Update Chart.js colors based on current theme
   */
  updateChartColors() {
    if (typeof Chart === 'undefined') return;

    const colors = this.getChartColors();

    Chart.defaults.color = colors.text;
    Chart.defaults.borderColor = colors.grid;
    Chart.defaults.backgroundColor = colors.background;
  }

  /**
   * Get chart colors for current theme
   * @returns {object} Color configuration for charts
   */
  getChartColors() {
    if (this.currentTheme === THEMES.DARK) {
      return {
        text: '#d1d5db',
        grid: '#374151',
        background: 'rgba(31, 41, 55, 0.8)',
        primary: '#6366f1',
        success: '#10b981',
        warning: '#f59e0b',
        danger: '#ef4444',
        info: '#3b82f6'
      };
    } else {
      return {
        text: '#374151',
        grid: '#e5e7eb',
        background: 'rgba(255, 255, 255, 0.8)',
        primary: '#4f46e5',
        success: '#059669',
        warning: '#d97706',
        danger: '#dc2626',
        info: '#2563eb'
      };
    }
  }

  /**
   * Get color for health score based on theme
   * @param {number} score - Health score (0-100)
   * @returns {string} Color hex code
   */
  getHealthScoreColor(score) {
    const colors = this.getChartColors();

    if (score >= 90) return colors.success;
    if (score >= 70) return colors.warning;
    if (score >= 50) return '#f97316'; // orange
    return colors.danger;
  }

  /**
   * Setup event listeners for theme toggle
   */
  setupEventListeners() {
    document.addEventListener('click', (e) => {
      if (e.target.closest('#theme-toggle')) {
        this.toggleTheme();
      }
    });

    if (window.matchMedia) {
      window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
        if (!localStorage.getItem(THEME_KEY)) {
          this.applyTheme(e.matches ? THEMES.DARK : THEMES.LIGHT);
        }
      });
    }
  }
}

const themeManager = new ThemeManager();

export { themeManager, THEMES };
