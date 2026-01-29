# Lighthouse Mobile Audit Guide

This document provides instructions for running a Lighthouse mobile audit on the Murdoch Dashboard to ensure it meets the requirement of achieving a score greater than 90.

## Prerequisites

1. The application must be running locally or deployed
2. Chrome browser or Chrome DevTools installed
3. Lighthouse CLI (optional, for automated testing)

## Running Lighthouse Audit via Chrome DevTools

### Step 1: Start the Application

```bash
# Start the Rust backend
cargo shuttle run

# Or if deployed, navigate to your deployment URL
```

### Step 2: Open Chrome DevTools

1. Open Chrome browser
2. Navigate to your dashboard URL (e.g., `http://localhost:8000`)
3. Open DevTools (F12 or Right-click → Inspect)
4. Click on the "Lighthouse" tab

### Step 3: Configure Lighthouse

1. Select "Mobile" device type
2. Check the following categories:
   - Performance
   - Accessibility
   - Best Practices
   - SEO
3. Click "Analyze page load"

### Step 4: Review Results

Lighthouse will generate a report with scores for each category. The mobile performance score should be greater than 90.

## Running Lighthouse via CLI

### Installation

```bash
npm install -g lighthouse
```

### Run Audit

```bash
# Basic mobile audit
lighthouse http://localhost:8000 --preset=desktop --view

# Mobile audit with specific categories
lighthouse http://localhost:8000 \
  --only-categories=performance,accessibility,best-practices,seo \
  --form-factor=mobile \
  --screenEmulation.mobile=true \
  --output=html \
  --output-path=./lighthouse-report.html \
  --view
```

### Automated Testing

```bash
# Run audit and save JSON output
lighthouse http://localhost:8000 \
  --only-categories=performance \
  --form-factor=mobile \
  --output=json \
  --output-path=./lighthouse-mobile.json

# Check if score meets threshold
node -e "const report = require('./lighthouse-mobile.json'); const score = report.categories.performance.score * 100; console.log('Mobile Performance Score:', score); process.exit(score >= 90 ? 0 : 1);"
```

## Mobile Optimizations Implemented

The following optimizations have been implemented to achieve a score > 90:

### 1. Responsive Design (Task 18.1)
- Media queries for screens < 768px
- Flexible grid layouts
- Optimized font sizes and spacing
- Horizontal scrolling for tables

### 2. Touch-Friendly Controls (Task 18.2)
- Minimum 44px touch targets for all interactive elements
- Increased button sizes and padding
- Touch-optimized form inputs (16px font to prevent zoom)
- Proper tap highlight colors

### 3. Optimized Charts (Task 18.3)
- Smaller chart heights on mobile (250px vs 300px)
- Reduced font sizes in legends and labels
- Fewer tick marks on axes
- Simplified tooltips
- Smaller point radii with larger hit areas

### 4. Pull-to-Refresh (Task 18.4)
- Native-like pull-to-refresh gesture
- Visual feedback during pull
- Smooth animations

### 5. Performance Optimizations
- Preconnect to external resources
- Deferred script loading
- Optimized CSS with minimal reflows
- Efficient chart rendering
- Request deduplication
- Caching layer

### 6. Accessibility
- Semantic HTML with proper ARIA roles
- Proper heading hierarchy
- Color contrast meeting WCAG 2.1 AA standards
- Keyboard navigation support
- Screen reader friendly

### 7. Best Practices
- HTTPS enforcement (in production)
- Proper viewport meta tag
- Theme color meta tag
- No console errors
- Proper error handling

## Expected Scores

With all optimizations implemented, the dashboard should achieve:

- **Performance**: > 90
- **Accessibility**: > 90
- **Best Practices**: > 90
- **SEO**: > 90

## Troubleshooting

### Low Performance Score

If the performance score is below 90:

1. **Check Network Conditions**: Run audit with "No throttling" to isolate network issues
2. **Reduce JavaScript**: Ensure scripts are deferred and minified
3. **Optimize Images**: Use WebP format and proper sizing
4. **Enable Caching**: Verify cache headers are set correctly
5. **Reduce Bundle Size**: Check for unused dependencies

### Low Accessibility Score

If the accessibility score is below 90:

1. **Check Color Contrast**: Use DevTools to verify WCAG compliance
2. **Add ARIA Labels**: Ensure all interactive elements have proper labels
3. **Keyboard Navigation**: Test all functionality with keyboard only
4. **Form Labels**: Verify all form inputs have associated labels

### Low Best Practices Score

If the best practices score is below 90:

1. **HTTPS**: Ensure the site is served over HTTPS in production
2. **Console Errors**: Fix any JavaScript errors
3. **Deprecated APIs**: Update any deprecated browser APIs
4. **Security Headers**: Add proper security headers

## Continuous Monitoring

To maintain high scores:

1. Run Lighthouse audits before each release
2. Set up CI/CD pipeline with Lighthouse CI
3. Monitor real user metrics with tools like Web Vitals
4. Regularly test on actual mobile devices

## Resources

- [Lighthouse Documentation](https://developers.google.com/web/tools/lighthouse)
- [Web Vitals](https://web.dev/vitals/)
- [Mobile Performance Best Practices](https://web.dev/mobile/)
- [WCAG 2.1 Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
