# Deployment Verification Checklist

**Created:** 2026-02-10
**Purpose:** Verify features actually work in deployed environment before marking "done"
**Use:** Run this checklist for EVERY story before marking it complete

## Why This Exists

**Epic 5 Retrospective Finding:** Stories marked "done" based on code existence, not working features. Chart had 3 critical bugs, admin features were unreachable - all went undetected because nobody actually verified deployment.

## Pre-Deployment Checks

### Build Verification

- [ ] `docker compose build` completes without errors
- [ ] No build warnings that indicate missing dependencies
- [ ] All Rust compilation succeeds
- [ ] Frontend WASM files generated (check `frontend/dist/`)
- [ ] Backend binary created

### Configuration Verification

- [ ] `.env` file exists with correct values
- [ ] Database connection string is correct
- [ ] All required environment variables are set
- [ ] No hardcoded credentials or secrets in code

## Deployment Checks

### Application Startup

- [ ] `docker compose up` starts all services
- [ ] Backend starts without errors
- [ ] Frontend starts without errors
- [ ] Database connection successful
- [ ] No error logs in `docker compose logs`

### Browser Access

- [ ] Application loads at expected URL (http://localhost:8080)
- [ ] Page renders without blank screens
- [ ] No JavaScript errors in browser console (F12 → Console)
- [ ] All CSS/styles loading correctly

## Feature Verification

### Navigation Check

- [ ] **New feature is accessible via navigation/UI**
- [ ] Can reach the feature without typing URLs manually
- [ ] Navigation links work correctly
- [ ] Breadcrumbs or back buttons work (if applicable)

### Functional Verification

- [ ] **Primary feature functionality works as expected**
- [ ] All buttons/controls respond to clicks
- [ ] Forms submit and validate correctly
- [ ] Data displays correctly
- [ ] No error messages on normal usage

### Data Verification

- [ ] Feature reads from database correctly
- [ ] Feature writes to database correctly
- [ ] Data persists after page refresh
- [ ] No data corruption or loss

### Visual Verification

- [ ] **Feature looks correct visually**
- [ ] Charts/graphs render (not blank canvases)
- [ ] Tables display data
- [ ] Images/icons load
- [ ] Layout is not broken
- [ ] Responsive on different screen sizes (if applicable)

## Integration Checks

### Cross-Feature Integration

- [ ] New feature doesn't break existing features
- [ ] Existing features still accessible
- [ ] Existing data still loads
- [ ] No regressions in other areas

### API Integration (if applicable)

- [ ] API endpoints respond correctly
- [ ] Status codes are appropriate (200, 201, 400, etc.)
- [ ] Response data matches expected schema
- [ ] Error handling works correctly

### External Dependencies (if applicable)

- [ ] External APIs accessible
- [ ] CDN resources loading (JavaScript libraries, fonts, etc.)
- [ ] Third-party services responding
- [ ] Rate limits respected

## Network/Performance Checks

### Resource Loading

- [ ] All JavaScript files load (Network tab)
- [ ] All CSS files load
- [ ] WASM files load
- [ ] No 404 errors in Network tab
- [ ] No failed requests

### Performance

- [ ] Page loads in acceptable time (< 3 seconds)
- [ ] Feature responds quickly to interactions
- [ ] No infinite loading spinners
- [ ] No memory leaks (check over time)

## Security Checks

### Access Control (if applicable)

- [ ] Unauthorized users cannot access protected features
- [ ] Admin features require proper permissions
- [ ] Authentication works correctly
- [ ] Session management working

### Data Protection

- [ ] No sensitive data in browser console
- [ ] No secrets in client-side code
- [ ] HTTPS enabled (if applicable)
- [ ] Input validation working

## Browser Compatibility (for frontend features)

Test in at least 2 browsers:

- [ ] Chrome/Chromium
- [ ] Firefox

Verify:
- [ ] Feature works in both
- [ ] No browser-specific errors
- [ ] Visual consistency

## Database State

### Data Integrity

- [ ] Database migrations applied
- [ ] Required tables/columns exist
- [ ] Sample data loads correctly
- [ ] No orphaned records
- [ ] Constraints working (foreign keys, etc.)

## Rollback Verification

### Can We Roll Back?

- [ ] Know how to revert database migrations if needed
- [ ] Previous version still deployable
- [ ] No irreversible data changes

## Story-Specific Checks

Add checks specific to the story's acceptance criteria:

**Example for Chart Feature:**
- [ ] Chart canvas visible (not blank)
- [ ] Chart displays data
- [ ] Legend shows correct labels
- [ ] Tooltips work on hover
- [ ] Chart responds to data changes

**Example for Admin Feature:**
- [ ] Admin panel accessible
- [ ] Data grid displays correctly
- [ ] Export functionality works
- [ ] Filters apply correctly

## Final Sign-Off

Before marking story "done":

- [ ] **I have personally verified this feature works in deployment**
- [ ] **Another person has tested it and confirmed it works**
- [ ] All checklist items above are checked
- [ ] No known bugs or incomplete functionality
- [ ] Product Owner (or proxy) has accepted the feature

## Red Flags - DO NOT Mark Done If:

- ❌ "It works on my machine" but not in Docker
- ❌ Code exists but feature isn't accessible to users
- ❌ Tests pass but visual verification fails
- ❌ Console shows errors
- ❌ Feature only partially works
- ❌ Navigation to feature is missing
- ❌ Chart/visualization is blank
- ❌ Data doesn't load or display

## Quick Smoke Test

Minimum verification for every story:

1. Build and deploy
2. Open browser to the feature
3. Use the main functionality
4. Check console for errors
5. Verify it looks correct

**Time estimate:** 5-10 minutes
**Worth it?** YES - prevents hours of debugging later

## Lessons from Epic 5

**Story 2.1 (Chart):** Marked done, but:
- Chart was blank (3 bugs)
- Nobody opened a browser to check
- Took hours to debug later

**Epic 5 (Admin Features):** Marked done, but:
- Features unreachable (no navigation)
- Nobody tried to access them
- Completely unusable

**Cost of skipping this checklist:** Hours of debugging, frustrated users, broken features in production

## Updates

This checklist should be updated:
- When new types of features are added
- After retrospectives identify gaps
- As team learns new verification needs
