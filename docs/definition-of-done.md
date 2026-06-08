# Definition of Done

**Last Updated:** 2026-02-10
**Status:** Active

## Purpose

This document defines the explicit criteria that must be met before a story can be marked as "done". This ensures consistent quality and prevents incomplete work from being considered finished.

## Story-Level Definition of Done

A story is considered "done" when ALL of the following criteria are met:

### 1. Code Quality
- [ ] Code written and implements all acceptance criteria
- [ ] Code follows project conventions and style guidelines
- [ ] No commented-out code or debug statements in production code
- [ ] Error handling implemented appropriately
- [ ] Code reviewed and approved by at least one other developer

### 2. Testing
- [ ] Unit tests written and passing (100% pass rate)
- [ ] Integration tests written for API endpoints (where applicable)
- [ ] All existing tests still passing (no regressions)
- [ ] Test coverage meets project standards
- [ ] Manual testing completed for UI components

### 3. Documentation
- [ ] Code comments added for complex logic
- [ ] README updated if new setup steps required
- [ ] Architecture doc updated if architectural decisions changed
- [ ] API documentation updated (if API changes made)

### 4. Deployment Verification
- [ ] Feature builds successfully in Docker
- [ ] Application starts without errors
- [ ] **Feature accessible via navigation/user journey**
- [ ] **Feature visually verified in deployed environment**
- [ ] No console errors in browser (for frontend features)
- [ ] No error logs in backend (for backend features)

### 5. Integration
- [ ] Feature integrated with existing codebase
- [ ] Navigation updated to access new feature (if applicable)
- [ ] Dependencies added to appropriate config files
- [ ] Environment variables documented in .env.example

### 6. Acceptance Criteria
- [ ] All acceptance criteria from story explicitly verified
- [ ] Product Owner or proxy has reviewed the feature
- [ ] Any deviations from original requirements documented and approved

## Epic-Level Definition of Done

An epic is considered "done" when:

- [ ] All stories in the epic meet the story-level definition of done
- [ ] Epic-level acceptance criteria met
- [ ] End-to-end user journey tested and working
- [ ] Performance requirements verified
- [ ] Security requirements verified (if applicable)
- [ ] Epic retrospective completed

## When to Mark Something "Done"

**ONLY mark a story as "done" when:**
1. You can demonstrate the working feature to someone else
2. A user could actually use the feature in the deployed environment
3. All checklist items above are checked off

**DO NOT mark a story as "done" if:**
- "It works on my machine" but not in deployment
- Code exists but isn't accessible to users
- Tests pass but feature doesn't work visually
- There are known bugs or incomplete functionality

## Consequences of Incomplete "Done"

Epic 5 retrospective revealed:
- Story 2.1 marked "done" with 3 critical bugs preventing chart from rendering
- Epic 5 features marked "done" but unreachable due to missing navigation
- **Cost:** Hours of debugging, user frustration, technical debt

## Review and Updates

This definition should be reviewed and updated:
- After each epic retrospective
- When new project requirements emerge
- When team composition changes
