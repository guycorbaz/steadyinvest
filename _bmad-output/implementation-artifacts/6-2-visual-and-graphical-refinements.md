# Story 6.2: Visual and Graphical Refinements

Status: ready-for-dev

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Value Hunter,
I want the application's visual design to match the "Institutional HUD" aesthetic throughout,
So that the interface feels professional and polished.

## Acceptance Criteria

1. **Given** the UX Design Specification defines the "Institutional HUD" aesthetic
2. **When** reviewing all screens and components
   - **Then** chart aesthetics must be visually polished and consistent
   - **And** layout spacing and alignment must be refined across all components
   - **And** color scheme must be consistent with the deep black background (#0F0F12)
   - **And** typography must use correct fonts (JetBrains Mono for data cells)
   - **And** all interactive elements must have appropriate hover and active states

## Problem Context

### Current State

After completing Epics 1-5, the naic application is functionally complete with all core features working:
- ✅ Ticker search and data retrieval
- ✅ Logarithmic SSG chart rendering
- ✅ Quality dashboard (ROE, Profit on Sales)
- ✅ Kinetic trendline projections
- ✅ Manual data overrides
- ✅ Thesis locking and report export
- ✅ API health monitoring

However, the visual design needs a comprehensive refinement pass to ensure it matches the "Institutional HUD" aesthetic defined in the UX Design Specification.

### Design Vision

The UX Design Specification (source: `_bmad-output/planning-artifacts/ux-design-specification.md`) defines a "Professional Zen" aesthetic that combines:

1. **Bloomberg Terminal Authority**: High-density, institutional-grade visual design
2. **Todoist Zen**: Minimalist, focused interface that stays out of the way
3. **Jira Structure**: Organized high-density metadata with clear hierarchy

**Key Design Principles:**
- **Analyst First**: Every design decision prioritizes clarity of trends over decorative flair
- **Stay Out of the Way**: Design for speed; every pixel must earn its right to exist
- **Precision as Comfort**: Sharp lines, consistent grids, high-quality typography
- **Tactile Truth**: Interactive elements feel responsive and direct

### Scope of Refinements

This story focuses on **visual polish and consistency**, NOT new functionality. All refinements should:
- Enhance existing components without changing behavior
- Improve visual consistency across the application
- Ensure adherence to the UX Design Specification
- Fix visual bugs and inconsistencies
- Polish interactive states (hover, active, focus)

## Tasks / Subtasks

### Task 1: Chart Visual Refinements (AC: #2 - Chart aesthetics)
- [ ] Review SSG chart visual design against UX spec
  - [ ] Verify color scheme matches Institutional HUD (background: #0F0F12, surfaces: #16161D)
  - [ ] Check trendline colors (Sales: #1DB954/Emerald, EPS: #3498DB/Electric Blue, Price: #F1C40F)
  - [ ] Ensure chart grid lines are subtle and non-distracting
  - [ ] Verify logarithmic scale labels are clearly visible
- [ ] Polish drag handles for kinetic projections
  - [ ] Ensure handles are visually prominent (8px radius, proper colors)
  - [ ] Add subtle glow/shadow on hover
  - [ ] Verify cursor changes appropriately (grab/grabbing)
- [ ] Refine chart legend and CAGR display
  - [ ] Check font consistency (Inter for labels, JetBrains Mono for numbers)
  - [ ] Ensure proper contrast ratios (7:1 minimum)
  - [ ] Verify spacing and alignment

### Task 2: Layout Spacing and Alignment (AC: #2 - Layout spacing)
- [ ] Audit all components for consistent spacing
  - [ ] Apply 4px precision grid throughout
  - [ ] Check padding and margins on all panels
  - [ ] Verify component alignment (grids, buttons, inputs)
- [ ] Review Quality Dashboard layout
  - [ ] Ensure 10-year data fits without scrolling on standard laptop (13-15")
  - [ ] Check monospace grid alignment with JetBrains Mono
  - [ ] Verify year-over-year trend indicators are clear
- [ ] Polish Valuation Panel layout
  - [ ] Ensure target price calculations are clearly visible
  - [ ] Check spacing between P/E inputs and result displays
  - [ ] Verify layout adapts to different value ranges

### Task 3: Color Scheme Consistency (AC: #2 - Color scheme)
- [ ] Audit all components against color system
  - [ ] Background: #0F0F12 (Deep Black) everywhere
  - [ ] Surfaces: #16161D (Subtle Grey) for panels
  - [ ] Primary Accent: #3B82F6 (Electric Blue) for interactive elements
  - [ ] Growth/Success: #10B981 (Emerald) for positive states
  - [ ] Danger/Warning: #EF4444 (Crimson) for alerts
- [ ] Check text colors
  - [ ] Primary text: #E0E0E0 (high contrast on black)
  - [ ] Secondary text: #B0B0B0 (labels and metadata)
  - [ ] Muted text: #71717A (zinc-400) for non-critical info
- [ ] Verify accent usage
  - [ ] Green (#1DB954) for Sales projection - verify consistency
  - [ ] Blue (#3498DB) for EPS projection - verify consistency
  - [ ] Yellow (#F1C40F) for Price - verify consistency

### Task 4: Typography Consistency (AC: #2 - Typography)
- [ ] Audit font usage across all components
  - [ ] UI/Structural elements: Inter font family
  - [ ] Financial data cells: JetBrains Mono (monospace)
  - [ ] Verify font weights (normal, medium, bold) are consistent
- [ ] Check data grid typography
  - [ ] Quality Dashboard: JetBrains Mono for all numbers
  - [ ] Valuation Panel: JetBrains Mono for financial calculations
  - [ ] Chart labels: JetBrains Mono for CAGR percentages
- [ ] Verify type scale
  - [ ] Compact, high-density scale favoring data over whitespace
  - [ ] Ensure readability at standard laptop DPI
  - [ ] Check that numerical alignment is perfect in monospace grids

### Task 5: Interactive State Polish (AC: #2 - Interactive elements)
- [ ] Audit all buttons and clickable elements
  - [ ] Hover states: subtle background change or border highlight
  - [ ] Active states: clear pressed effect
  - [ ] Focus states: visible keyboard focus indicators
  - [ ] Disabled states: reduced opacity with clear visual distinction
- [ ] Review form inputs (sliders, text fields, dropdowns)
  - [ ] Hover: border color change or glow
  - [ ] Focus: prominent focus ring matching accent color
  - [ ] Active: clear interaction feedback
  - [ ] Validation states: success (green) and error (red) indicators
- [ ] Polish navigation elements (Command Strip if present)
  - [ ] Hover: highlight current section
  - [ ] Active: clear indication of current page
  - [ ] Transitions: smooth, not jarring

### Task 6: Visual Consistency Audit (AC: #2 - Consistency across components)
- [ ] Create visual inventory of all components
  - [ ] List all UI elements: buttons, inputs, panels, modals, etc.
  - [ ] Document current visual treatment
  - [ ] Identify inconsistencies
- [ ] Standardize component patterns
  - [ ] Border radius: consistent across similar components (likely 0-2px for sharp institutional look)
  - [ ] Border width: consistent (likely 1px)
  - [ ] Shadow usage: consistent depth and color
  - [ ] Transition timing: consistent (likely fast, ~150-200ms)
- [ ] Review and fix visual bugs
  - [ ] Check for layout shifts or jumps
  - [ ] Verify no text clipping or overflow
  - [ ] Ensure proper z-index layering
  - [ ] Fix any alignment issues

### Task 7: High-DPI and Accessibility Verification (AC: #2 - Professional polish)
- [ ] Test on high-DPI displays
  - [ ] Verify 4px grid renders sharply
  - [ ] Check that borders and lines are crisp
  - [ ] Ensure charts render without pixelation
- [ ] Verify accessibility standards
  - [ ] Contrast ratios: 7:1 minimum (WCAG AAA for financial data)
  - [ ] Focus indicators: clearly visible
  - [ ] Keyboard navigation: works for all interactive elements
  - [ ] Screen reader: proper ARIA labels where needed

## Dev Notes

### Visual Design System Reference

**Source:** `_bmad-output/planning-artifacts/ux-design-specification.md`

**Color System (Institutional HUD):**
```css
--background: #0F0F12;      /* Professional Deep Black */
--surface: #16161D;         /* Subtle Grey Layers */
--primary: #3B82F6;         /* Electric Blue - Interactive Core */
--success: #10B981;         /* Emerald - Growth/Success */
--danger: #EF4444;          /* Crimson - Integrity/Danger */
--text-primary: #E0E0E0;    /* High contrast text */
--text-secondary: #B0B0B0;  /* Labels and metadata */
--text-muted: #71717A;      /* Non-critical info */

/* Chart-specific colors */
--sales-color: #1DB954;     /* Sales projection (Green) */
--eps-color: #3498DB;       /* EPS projection (Blue) */
--price-color: #F1C40F;     /* Price high (Yellow) */
```

**Typography System:**
```css
/* UI/Structural */
font-family: 'Inter', sans-serif;

/* Financial Data */
font-family: 'JetBrains Mono', monospace;

/* Type Scale: Compact and high-density */
--text-xs: 0.75rem;    /* 12px */
--text-sm: 0.875rem;   /* 14px */
--text-base: 1rem;     /* 16px */
--text-lg: 1.125rem;   /* 18px */
--text-xl: 1.25rem;    /* 20px */
```

**Spacing System (4px Precision Grid):**
```css
--spacing-1: 0.25rem;  /* 4px */
--spacing-2: 0.5rem;   /* 8px */
--spacing-3: 0.75rem;  /* 12px */
--spacing-4: 1rem;     /* 16px */
--spacing-5: 1.25rem;  /* 20px */
--spacing-6: 1.5rem;   /* 24px */
--spacing-8: 2rem;     /* 32px */
```

### Key Files to Review and Refine

1. **Global Styles:**
   - `frontend/public/styles.scss` - Main stylesheet
   - `frontend/index.html` - Font loading and global CSS

2. **Chart Components:**
   - `frontend/src/components/ssg_chart.rs` - SSG chart with projections
   - `frontend/public/chart_bridge.js` - ECharts configuration

3. **Dashboard Components:**
   - `frontend/src/components/quality_dashboard.rs` - ROE/Profit on Sales grid
   - `frontend/src/components/valuation_panel.rs` - P/E calculations and targets

4. **HUD Components:**
   - `frontend/src/components/analyst_hud.rs` - Main analyst workspace
   - `frontend/src/components/snapshot_hud.rs` - Thesis locking interface

5. **Other Components:**
   - `frontend/src/components/search_bar.rs` - Initial ticker search
   - `frontend/src/components/override_modal.rs` - Manual data override UI
   - `frontend/src/components/lock_thesis_modal.rs` - Thesis locking modal

### Design Principles to Follow

1. **Sharp, Not Rounded**: Prefer sharp edges (border-radius: 0-2px) over rounded corners for institutional feel
2. **High Contrast**: Maintain 7:1 contrast ratio for all text on background
3. **Monospace Alignment**: Use JetBrains Mono for ALL numerical data to ensure perfect column alignment
4. **Minimal Shadows**: Use subtle shadows only where depth is needed (panels, modals)
5. **Fast Transitions**: Keep animations snappy (150-200ms) for responsive feel
6. **Grid-Based Spacing**: All spacing must be multiples of 4px
7. **Consistent Accents**: Use color system consistently across all components

### Previous Work Context (Story 6.1)

**Files Modified in 6.1:**
- `frontend/src/components/ssg_chart.rs` - Fixed CAGR projection inversion bug
- `frontend/public/chart_bridge.js` - Added drag handle debugging

**Key Learnings:**
- Leptos reactive signals work well for real-time chart updates
- ECharts charming library integration is stable
- Console logging was added but not essential for production
- Code follows Rust best practices with doc comments

**Visual Elements Already Implemented:**
- Chart uses correct colors (Sales: green, EPS: blue, Price: yellow)
- Sliders have accent colors (green/blue)
- Basic layout structure exists
- Dark theme background is in place

### Testing Requirements

**Visual Testing Checklist:**
1. Deploy with `docker compose up`
2. Test on multiple screen sizes (13", 15", 17" laptops)
3. Verify all components on different tickers (AAPL, NESN.SW, SMI stocks)
4. Check hover states on all interactive elements
5. Verify font rendering on high-DPI display
6. Test keyboard navigation and focus states
7. Use browser DevTools to verify CSS consistency
8. Take screenshots for before/after comparison

**Browser Testing:**
- Chrome/Chromium (primary)
- Firefox (secondary)
- Safari (if available)

### Non-Functional Requirements

From PRD and Architecture:
- **NFR1**: SPA initial load under 2 seconds (don't add heavy CSS that slows this)
- **Performance**: Keep WASM bundle lean, avoid heavy CSS libraries
- **Accessibility**: WCAG AA minimum, AAA for financial data (7:1 contrast)

### Architecture Constraints

**Tech Stack:**
- Frontend: Leptos 0.6+ (Rust/WASM)
- Styling: Vanilla CSS (styles.scss) + inline Leptos styling
- Charting: charming library (ECharts wrapper)
- No external CSS frameworks (custom design system)

**File Organization:**
```
frontend/
├── public/
│   ├── styles.scss         # Main stylesheet (compile to CSS)
│   └── chart_bridge.js     # ECharts configuration
├── src/
│   ├── components/         # All UI components
│   │   ├── ssg_chart.rs
│   │   ├── quality_dashboard.rs
│   │   ├── valuation_panel.rs
│   │   └── ...
│   └── pages/
│       └── home.rs         # Main page layout
└── index.html             # Font loading and global styles
```

### Code Style and Patterns

**Leptos Inline Styling Pattern:**
```rust
view! {
    <div style="
        background-color: #0F0F12;
        padding: 20px;
        border-radius: 8px;
        border: 1px solid #333;
    ">
        // Component content
    </div>
}
```

**CSS Class Pattern (in styles.scss):**
```scss
.analyst-hud {
  background-color: #0F0F12;
  padding: var(--spacing-4);
  border-radius: 2px; // Sharp institutional look
  border: 1px solid #16161D;
}

.data-cell {
  font-family: 'JetBrains Mono', monospace;
  color: #E0E0E0;
  font-size: 0.875rem; // 14px
}
```

**Leptos Class Pattern:**
```rust
view! {
    <div class="analyst-hud">
        <span class="data-cell">{move || format!("{:.2}%", value.get())}</span>
    </div>
}
```

### Common Patterns from Existing Code

**From ssg_chart.rs (lines 211-249):**
```rust
<div class="ssg-chart-wrapper" style="
    background-color: #0F0F12;
    padding: 20px;
    border-radius: 8px;
    margin-bottom: 30px;
    border: 1px solid #333;
">
```

**Current slider styling (lines 215-238):**
- Uses Tailwind-like utility classes: `flex`, `justify-between`, `items-center`, `gap-4`
- Text colors: `text-zinc-400`, `text-green-500`, `text-blue-500`
- Accent colors: `accent-green-500`, `accent-blue-500`

**Recommendation:** Standardize on either:
1. Pure inline styles (more explicit, works everywhere)
2. CSS classes in styles.scss (more maintainable, reusable)
3. Combination: structural layout in SCSS, component-specific in inline

### Definition of Done

Per project standards:

**Code Quality:**
- [ ] All visual refinements maintain existing functionality
- [ ] No regressions in chart rendering or interactivity
- [ ] Code follows Rust/CSS conventions

**Visual Quality:**
- [ ] All components match UX Design Specification
- [ ] Color scheme is consistent throughout
- [ ] Typography uses correct fonts everywhere
- [ ] Interactive states are polished and consistent
- [ ] Layout spacing follows 4px grid

**Testing:**
- [ ] Visual verification in deployed Docker environment
- [ ] Tested on multiple screen sizes
- [ ] Tested with multiple tickers
- [ ] All interactive elements have proper hover/focus states
- [ ] No console errors

**Documentation:**
- [ ] Code comments added for non-obvious CSS
- [ ] Story file updated with completion notes
- [ ] Screenshots or visual documentation if significant changes

**Accessibility:**
- [ ] Contrast ratios verified (7:1 minimum)
- [ ] Keyboard navigation works
- [ ] Focus indicators are visible

### References

- [Source: _bmad-output/planning-artifacts/ux-design-specification.md - Visual Design Foundation]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md - Design System Foundation]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md - Core User Experience]
- [Source: _bmad-output/planning-artifacts/epics.md - Epic 6, Story 6.2]
- [Source: _bmad-output/implementation-artifacts/6-1-investigate-and-fix-cagr-eps-slider-behavior.md - Previous story context]

## Dev Agent Record

### Agent Model Used

Claude Sonnet 4.5 (claude-sonnet-4-5-20250929)

### Debug Log References

_To be added during implementation_

### Completion Notes List

_To be added by dev agent during implementation_

### File List

_To be added by dev agent during implementation_
