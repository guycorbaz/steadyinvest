# Living Documentation Process

**Created:** 2026-02-10
**Owner:** Scrum Master + Team
**Status:** Active

## Purpose

Prevent documentation drift by establishing a process where documentation stays current with implementation. Born from Epic 5 retrospective where Architecture.md still referenced PostgreSQL despite migrating to MariaDB in Epic 1.

## Core Principle

**Documentation is code.** When implementation changes, documentation MUST change too.

## When to Update Documentation

### Immediate Updates (Same PR/Story)

Update documentation IN THE SAME COMMIT/PR when you:

1. **Change database technology** (Postgres → MariaDB, etc.)
2. **Add/remove major dependencies** (libraries, services, APIs)
3. **Change authentication/authorization approach**
4. **Modify API contracts** (endpoints, request/response formats)
5. **Change deployment architecture** (containerization, hosting, etc.)
6. **Add new environment variables** (update .env.example)
7. **Change build process** (new tools, commands, steps)

### Document Updates That Can't Wait

**Within 24 hours** of implementation:

- Architecture decision records
- Setup instructions (README)
- API documentation
- Configuration changes
- Security-related changes

## What Documents Need Updating

### 1. Architecture Document (`architecture.md`)
**Update when:**
- Database technology changes
- Major library/framework changes
- Deployment strategy changes
- Security model changes
- New services/components added

**Review:** Every epic retrospective

### 2. README (`README.md`)
**Update when:**
- Setup steps change
- New prerequisites added
- Build commands change
- Environment variables added/changed

**Review:** Every story that changes setup

### 3. API Documentation
**Update when:**
- New endpoints added
- Request/response schemas change
- Authentication requirements change
- Error codes change

**Review:** Every API-related story

### 4. `.env.example`
**Update when:**
- New environment variables required
- Database connection format changes
- External service configuration changes

**Review:** Every configuration change

### 5. Definition of Done (`definition-of-done.md`)
**Update when:**
- New quality standards agreed
- New tools/processes adopted
- Retrospectives reveal gaps

**Review:** Every epic retrospective

## Process for Updates

### During Development

1. **Make the change** in code
2. **Update relevant docs** in the same branch
3. **Include doc changes** in the same PR/commit
4. **Code review checks docs** were updated

### Code Review Checklist

Reviewers must verify:
- [ ] If implementation changed, docs updated?
- [ ] If new dependency added, documented?
- [ ] If config changed, .env.example updated?
- [ ] If API changed, API docs updated?
- [ ] If architecture changed, architecture.md updated?

### Epic Retrospective Audit

At each epic retrospective:

1. **Documentation Drift Check:**
   - Open Architecture.md
   - Compare to actual implementation
   - Identify any drift
   - Fix immediately or create action item

2. **README Verification:**
   - Can a new developer follow setup instructions?
   - Are prerequisites current?
   - Test setup on clean environment

3. **API Documentation Check:**
   - Run actual API calls
   - Verify documentation matches reality
   - Update any discrepancies

## Documentation Owner

**Primary:** Scrum Master ensures process followed

**Secondary:** Every developer is responsible for updating docs when they change code

**Reviewer:** Code reviewers verify docs updated before approval

## Tools and Automation

### Helpful Checks

1. **Git Pre-commit Hook** (future):
   - If certain files changed, prompt for doc review
   - Example: If `backend/src/models/` changes, remind about API docs

2. **CI/CD Check** (future):
   - Fail builds if .env.example missing variables from config files
   - Check for TODO/FIXME in documentation

3. **Documentation Tests** (future):
   - Verify code examples in docs actually compile/run
   - Test setup instructions in fresh container

## Consequences of Drift

**From Epic 5 Retrospective:**

- Architecture doc said PostgreSQL
- System actually used MariaDB
- Team didn't know current architecture
- Debugging was harder
- Onboarding would give wrong information

**Cost:** Confusion, wasted time, loss of trust in documentation

## Quick Reference

| Code Change | Docs to Update |
|-------------|---------------|
| Database switch | Architecture.md, .env.example, README.md |
| New API endpoint | API docs, Architecture.md |
| New env variable | .env.example, README.md |
| Deployment change | Architecture.md, README.md |
| New dependency | README.md, Architecture.md |
| Build process change | README.md |
| Security model change | Architecture.md, Security.md |

## Review Schedule

- **Every PR:** Reviewer checks docs
- **Every Story:** Developer updates affected docs
- **Every Epic:** Retrospective audits all major docs
- **Quarterly:** Full documentation review and cleanup

## Success Metrics

- Zero "documentation drift" findings in retrospectives
- New developers can set up project without asking questions
- Architecture doc matches reality 100%
- API docs match actual API behavior
