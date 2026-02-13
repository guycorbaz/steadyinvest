# Post-Epic 5 Refinements & Issues

**Created:** 2026-02-10
**Status:** Backlog for Next Sprint
**Priority:** Medium (MVP is functional, these are polish items)

---

## Context

After Epic 5 retrospective and bug fixes, the application is now fully functional as an MVP. All core features work and are accessible. The following items are refinements and polish work for the next development cycle.

---

## 🎨 Cosmetic & UX Improvements

### 1. Graphical Refinements Needed

**Category:** UX Polish
**Priority:** Medium
**Description:** Various graphical elements need cosmetic adjustments for professional appearance.

**Details:**
- Chart aesthetics and visual polish
- Layout refinements throughout the application
- Color scheme consistency
- Typography adjustments
- Spacing and alignment improvements
- Responsive design fine-tuning

**User Feedback:** "MVP needs some cosmetics adjustments, mainly for the graphical part"

**Recommendation:**
- Conduct UX review session
- Create specific stories for visual improvements
- Reference UX Design Specification for intended aesthetic
- Consider user testing feedback

---

## 🐛 Functional Issues to Investigate

### 2. Projected CAGR and EPS Sliders - Possible Inversion

**Category:** Bug Investigation
**Priority:** High
**Story Reference:** Story 3.1 (Kinetic Trendline Projection)
**Component:** `frontend/src/components/ssg_chart.rs`

**Reported Issue:**
"There are some tests to do with projected CAGR and EPS sliders that seems to be inverted."

**Symptoms:**
- Moving Sales CAGR slider may affect EPS projection (suspected)
- Moving EPS CAGR slider may affect Sales projection (suspected)
- Slider labels and effects may not match

**Investigation Needed:**
1. Test slider behavior systematically:
   - Move Sales CAGR slider → verify Sales projection changes (not EPS)
   - Move EPS CAGR slider → verify EPS projection changes (not Sales)
2. Check signal bindings in `ssg_chart.rs`:
   - `sales_projection_cagr` signal
   - `eps_projection_cagr` signal
3. Verify JavaScript bridge callbacks:
   - `rust_update_sales_cagr()`
   - `rust_update_eps_cagr()`
4. Check chart rendering order

**Potential Root Causes:**
- Signal wiring crossed in component
- Chart series order doesn't match expectations
- JavaScript callback functions calling wrong Rust functions
- Labels correct but underlying calculations swapped

**Test Plan:**
1. Search for a stock (e.g., AAPL)
2. Note initial Sales and EPS projections
3. Move ONLY Sales CAGR slider → verify ONLY Sales projection changes
4. Reset
5. Move ONLY EPS CAGR slider → verify ONLY EPS projection changes
6. Document actual vs expected behavior

**Files to Review:**
- `frontend/src/components/ssg_chart.rs` (lines 26-41: signal callbacks)
- `frontend/public/chart_bridge.js` (lines 35-42, 52-59: drag callbacks)

**Acceptance Criteria for Fix:**
- Sales CAGR slider controls ONLY Sales projection
- EPS CAGR slider controls ONLY EPS projection
- Visual feedback matches slider labels
- Calculated target prices reflect correct projections

---

## 📋 Backlog Items

### Additional Items for Consideration

**Code Documentation & Technical Debt:**
- [ ] **Comprehensive Rust documentation pass** (High Priority)
  - Add doc comments (`///`) to all public functions and methods
  - Document all structs, enums, and type definitions
  - Explain complex algorithms and tricky code sections
  - Add module-level documentation (`//!`)
  - Include usage examples for non-trivial functions
  - Document panic conditions and error handling
  - **Rationale:** Follow Rust best practices for maintainability and onboarding
  - **Scope:** Backend (`backend/src/**`), Frontend (`frontend/src/**`), Logic crate (`crates/steady-invest-logic/**`)

**Testing:**
- [ ] E2E test suite (Action Item 3 from retrospective)
- [ ] Cross-browser compatibility testing
- [ ] Mobile responsive testing

**Features:**
- [ ] Data export functionality improvements
- [ ] Additional chart customization options
- [ ] Performance optimizations

**Documentation:**
- [ ] User guide/help documentation
- [ ] Keyboard shortcuts guide
- [ ] Data source attribution

---

## 🎯 Sprint Planning Recommendations

### Suggested Epic Structure

**Epic 6: MVP Refinement & Polish**
- Story 6.1: Investigate and fix CAGR/EPS slider behavior
- Story 6.2: Visual/graphical refinements (cosmetic improvements)
- Story 6.3: UX consistency pass
- Story 6.4: Responsive design improvements
- Story 6.5: E2E test suite implementation
- Story 6.6: Comprehensive Rust documentation pass (functions, structs, modules)

**Estimated Effort:** 2-3 sprints

---

## 📊 Current MVP Status

**Working Features:**
- ✅ Ticker search and data retrieval
- ✅ SSG chart visualization (logarithmic)
- ✅ Quality dashboard (ROE, Profit on Sales)
- ✅ Valuation calculations
- ✅ Projection sliders (functionality present, may need correction)
- ✅ System Monitor (admin)
- ✅ Audit Log (admin)
- ✅ Navigation system
- ✅ Footer with latency monitoring

**Ready for Refinement:**
- 🎨 Visual polish
- 🔍 Slider behavior verification
- 📱 Responsive design enhancements
- 🧪 Comprehensive testing

---

## 💡 Notes from Epic 5 Retrospective

The Epic 5 retrospective was highly productive:
- Fixed 3 critical chart rendering bugs
- Implemented missing navigation system
- Established robust processes and documentation
- Transformed non-functional application into working MVP

**Key Lesson:** Visual verification and user testing are essential. Guy's feedback about sliders and cosmetics came from actually using the application - exactly the kind of verification that should happen before marking stories "done."

**Process Reminder:** Use the Definition of Done checklist, including functional testing of interactive elements like sliders.

---

## 🚀 Ready to Ship

**Current State:** Functional MVP ready for user feedback and iterative improvement

**Next Steps:**
1. Prioritize slider investigation (potential functional issue)
2. Create detailed stories for cosmetic improvements
3. Implement E2E tests to catch issues like slider inversion
4. Continue with Definition of Done for all new work

---

**Document Owner:** Bob (Scrum Master)
**Review Date:** Before next sprint planning
**Status:** Ready for sprint planning discussion
