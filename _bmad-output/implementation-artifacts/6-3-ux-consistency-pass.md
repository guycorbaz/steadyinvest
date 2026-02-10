# Story 6.3: UX Consistency Pass

Status: ready-for-dev

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Value Hunter,
I want a consistent user experience across all features and workflows,
So that the application feels cohesive and intuitive.

## Acceptance Criteria

1. **Given** the application has multiple features and panels
2. **When** navigating between different sections
   - **Then** navigation patterns must be consistent (Command Strip behavior)
   - **And** data entry patterns must follow the same interaction model
   - **And** error states and validation messages must use consistent styling and tone
   - **And** keyboard shortcuts must work consistently across all screens
   - **And** loading states and feedback must follow the same visual language

## Problem Context

### Current State

After Story 6.2, the application has:
- ✅ Comprehensive design system with CSS variables
- ✅ Consistent visual styling (colors, typography, spacing)
- ✅ Mobile/tablet responsive layouts
- ✅ Polished interactive states (hover, focus, active)

However, **user experience consistency** across features needs attention:
- Navigation patterns may vary between features
- Data entry interactions may not follow the same model
- Error handling and validation messaging may be inconsistent
- Keyboard shortcuts may not work uniformly across screens
- Loading states may use different visual feedback patterns

### UX Consistency Definition

**Source:** `_bmad-output/planning-artifacts/ux-design-specification.md` - Core User Experience

UX consistency means:
1. **Navigation Consistency**: Command Strip behavior uniform across all screens
2. **Interaction Consistency**: Data entry, selection, and manipulation follow same patterns
3. **Feedback Consistency**: Error messages, validation, loading states use same visual language
4. **Keyboard Consistency**: Shortcuts work predictably across all contexts
5. **State Consistency**: Disabled, loading, error, success states look and behave the same

### Scope

This story focuses on **behavior and interaction consistency**, NOT visual styling (that was Story 6.2). Key areas:

1. **Navigation**: Command Strip, page transitions, breadcrumbs
2. **Data Entry**: Input fields, sliders, dropdowns, validation
3. **Error Handling**: Error messages, validation feedback, recovery flows
4. **Keyboard Navigation**: Tab order, shortcuts, focus management
5. **Loading States**: Spinners, skeletons, progress indicators

## Tasks / Subtasks

### Task 1: Navigation Consistency Audit (AC: #2 - Navigation patterns)
- [ ] Audit Command Strip behavior across all pages
  - [ ] Verify active state highlighting works on all pages
  - [ ] Check that navigation items are consistently accessible
  - [ ] Ensure Command Strip collapses consistently on mobile
  - [ ] Verify hover states work uniformly
- [ ] Audit page transitions
  - [ ] Check that transition animations are consistent
  - [ ] Verify loading states between page changes
  - [ ] Ensure back/forward navigation works consistently
- [ ] Document navigation patterns
  - [ ] Create navigation pattern documentation
  - [ ] Identify and fix inconsistencies

### Task 2: Data Entry Consistency Audit (AC: #2 - Data entry patterns)
- [ ] Audit input field interactions
  - [ ] Text inputs: placeholder text, focus behavior, validation timing
  - [ ] Number inputs: increment controls, validation, formatting
  - [ ] Sliders: drag behavior, value display, keyboard input
  - [ ] Dropdowns: selection behavior, keyboard navigation
- [ ] Audit validation patterns
  - [ ] Verify validation triggers consistently (on blur, on change, on submit)
  - [ ] Check that validation messages appear in consistent locations
  - [ ] Ensure validation styling is uniform (error borders, icons, colors)
- [ ] Audit form submission patterns
  - [ ] Check submit button states (enabled, disabled, loading)
  - [ ] Verify form submission feedback is consistent
  - [ ] Ensure success/error handling follows same pattern

### Task 3: Error State Consistency (AC: #2 - Error states and validation messages)
- [ ] Audit error message patterns
  - [ ] Inventory all error message locations and styling
  - [ ] Check error message tone and language consistency
  - [ ] Verify error icons and visual indicators are consistent
- [ ] Audit validation feedback
  - [ ] Check inline validation (field-level errors)
  - [ ] Check form-level validation (summary errors)
  - [ ] Verify validation error recovery flows
- [ ] Create error handling guidelines
  - [ ] Document error message templates
  - [ ] Define error state visual patterns
  - [ ] Specify error recovery workflows

### Task 4: Keyboard Navigation Consistency (AC: #2 - Keyboard shortcuts)
- [ ] Audit keyboard shortcuts
  - [ ] Create inventory of all keyboard shortcuts
  - [ ] Test shortcuts work on all applicable screens
  - [ ] Check for shortcut conflicts or inconsistencies
- [ ] Audit tab order and focus management
  - [ ] Verify logical tab order on all pages
  - [ ] Check focus indicators are visible and consistent
  - [ ] Ensure focus traps work in modals
  - [ ] Verify Escape key consistently closes modals/dialogs
- [ ] Document keyboard interactions
  - [ ] Create keyboard shortcut reference
  - [ ] Add keyboard hints to UI where appropriate

### Task 5: Loading State Consistency (AC: #2 - Loading states and feedback)
- [ ] Audit loading indicators
  - [ ] Check initial page load indicator
  - [ ] Verify data fetching indicators (chart, dashboard)
  - [ ] Check button loading states (spinners)
  - [ ] Verify skeleton screens or placeholders
- [ ] Audit progress feedback
  - [ ] Check that long operations show progress
  - [ ] Verify timeout handling is consistent
  - [ ] Ensure loading failure states are handled
- [ ] Standardize loading patterns
  - [ ] Define loading spinner component usage
  - [ ] Document loading state best practices
  - [ ] Implement consistent timeout/retry logic

### Task 6: Component-Specific Consistency

Fix any component-specific inconsistencies discovered:

- [ ] **Search Bar**: Verify behavior, validation, keyboard handling
- [ ] **SSG Chart**: Sliders, drag handles, keyboard shortcuts
- [ ] **Quality Dashboard**: Table interactions, sorting, keyboard navigation
- [ ] **Valuation Panel**: Input fields, sliders, calculation feedback
- [ ] **Modals** (Override, Lock Thesis): Open/close, focus trap, keyboard handling
- [ ] **System Monitor**: Navigation, data refresh, error display
- [ ] **Audit Log**: Filtering, pagination, keyboard shortcuts

## Dev Notes

### Key Files for UX Consistency Review

**Frontend Components (Primary Focus):**
- `frontend/src/pages/home.rs` - Main page layout and navigation
- `frontend/src/components/search_bar.rs` - Search interaction patterns
- `frontend/src/components/ssg_chart.rs` - Chart interactions, sliders, keyboard
- `frontend/src/components/quality_dashboard.rs` - Table interactions
- `frontend/src/components/valuation_panel.rs` - Input fields, sliders
- `frontend/src/components/analyst_hud.rs` - Main workspace navigation
- `frontend/src/components/override_modal.rs` - Modal interaction patterns
- `frontend/src/components/lock_thesis_modal.rs` - Modal keyboard handling
- `frontend/src/pages/system_monitor.rs` - System admin navigation
- `frontend/src/pages/audit_log.rs` - Data table interactions

**Global Styles:**
- `frontend/public/styles.scss` - Component interaction styles

**JavaScript Bridge:**
- `frontend/public/chart_bridge.js` - Chart keyboard and mouse interactions

### Previous Story Learnings (Story 6.2)

**What Worked Well:**
1. **CSS Variables**: Design system with semantic variables made global changes easy
2. **Mobile-First Responsive**: @media queries prevented desktop-only assumptions
3. **ECharts emphasis API**: Using proper APIs (not custom hacks) for hover effects
4. **Typography via CSS Classes**: Removing inline styles reduced bundle size

**Key Patterns Established:**
1. **Design System**: Colors, spacing, typography all use CSS variables
2. **Responsive Strategy**: Mobile (<768px), Tablet (769-1024px), Desktop (>1024px)
3. **Focus States**: 2px outline with 2px offset for keyboard accessibility
4. **Cursor States**: grab (idle), grabbing (during drag) for tactile feedback
5. **Transitions**: Fast (150ms) for snappy feel

**Files Modified in Story 6.2:**
- `frontend/public/styles.scss`: 450+ lines, complete design system
- `frontend/src/components/ssg_chart.rs`: Responsive chart width
- `frontend/src/components/quality_dashboard.rs`: Typography via CSS classes
- `frontend/src/components/valuation_panel.rs`: Design system integration
- `frontend/public/chart_bridge.js`: ECharts emphasis API + cursor states

### UX Consistency Patterns to Follow

**From UX Design Specification:**

**Navigation:**
- Command Strip: Persistent, vertical sidebar with active state highlighting
- Transitions: Smooth, not jarring (150-200ms)
- Mobile: Command Strip collapses to 60px width

**Data Entry:**
- Input fields: Focus ring with primary color (#3B82F6)
- Validation: Inline feedback on blur, form-level on submit
- Error states: Red border (#EF4444) + error message below field
- Success states: Green indicator (#10B981) for confirmed actions

**Keyboard Navigation:**
- Tab order: Logical, left-to-right, top-to-bottom
- Focus indicators: 2px outline, 2px offset, primary color
- Escape key: Closes modals, cancels operations
- Enter key: Submits forms, confirms actions
- Arrow keys: Navigate lists, adjust sliders

**Loading States:**
- Initial load: Pulse animation with primary color
- Data fetching: Skeleton screens or subtle spinner
- Button loading: Spinner replaces button text, button disabled
- Timeout: 30s default, show error message with retry option

### Architecture Compliance

**From `architecture.md`:**

**Tech Stack:**
- Frontend: Leptos 0.6+ (Rust/WASM)
- Styling: Vanilla CSS (styles.scss) - NO CSS frameworks
- Charting: charming library (ECharts wrapper)
- State Management: Leptos RwSignal for reactive state

**Code Organization:**
```
frontend/
├── src/
│   ├── app.rs                 # Main app component
│   ├── pages/
│   │   ├── mod.rs
│   │   ├── home.rs           # Main analysis page
│   │   ├── system_monitor.rs # System health page
│   │   └── audit_log.rs      # Audit log page
│   └── components/
│       ├── mod.rs
│       ├── search_bar.rs     # Ticker search
│       ├── ssg_chart.rs      # SSG chart with sliders
│       ├── quality_dashboard.rs # ROE/Profit table
│       ├── valuation_panel.rs   # P/E valuation
│       ├── analyst_hud.rs       # Main HUD wrapper
│       ├── override_modal.rs    # Data override modal
│       └── lock_thesis_modal.rs # Thesis lock modal
└── public/
    ├── styles.scss           # Main stylesheet
    └── chart_bridge.js       # ECharts bridge
```

**Leptos Patterns:**
- Use `RwSignal` for mutable state
- Use `Signal` for read-only derived state
- Use `view!` macro for JSX-like templates
- Use `on:event` for event handlers
- Use `prop:attribute` for reactive attributes

### Testing Requirements

**Manual Testing Checklist:**

1. **Navigation Testing:**
   - [ ] Click through all Command Strip items
   - [ ] Verify active state highlights correctly
   - [ ] Test mobile Command Strip collapse
   - [ ] Check page transitions are smooth

2. **Data Entry Testing:**
   - [ ] Tab through all input fields on each page
   - [ ] Test validation on: search, override modal, P/E inputs
   - [ ] Verify error messages appear consistently
   - [ ] Check slider interactions (mouse + keyboard)

3. **Keyboard Navigation Testing:**
   - [ ] Tab order logical on all pages
   - [ ] Escape closes all modals
   - [ ] Enter submits all forms
   - [ ] Arrow keys work on sliders
   - [ ] Focus indicators visible everywhere

4. **Loading State Testing:**
   - [ ] Initial app load shows spinner
   - [ ] Chart loading shows skeleton/spinner
   - [ ] Button loading states work correctly
   - [ ] Network timeout shows error + retry

5. **Error Handling Testing:**
   - [ ] Invalid ticker search shows error
   - [ ] Network failure shows retry option
   - [ ] Validation errors are clear and actionable
   - [ ] Error recovery flows work smoothly

**Browser Testing:**
- Chrome/Chromium (primary)
- Firefox (secondary)
- Mobile browser (responsive testing)

### Non-Functional Requirements

**From PRD:**
- **NFR1**: SPA initial load under 2 seconds
- **Accessibility**: WCAG AA minimum, keyboard parity for analysis workflow

**UX Performance:**
- Interactions should feel instant (<100ms perceived response)
- Loading states should appear if operation takes >200ms
- Animations should be smooth (60fps)

### Common UX Consistency Issues to Avoid

1. **Inconsistent Validation**:
   - ❌ Some fields validate on change, others on blur
   - ✅ Consistent timing: validate on blur, show errors immediately

2. **Inconsistent Error Messages**:
   - ❌ "Invalid input", "Error", "Ticker not found", "Failed"
   - ✅ Consistent tone: "Please enter a valid ticker symbol (e.g., AAPL)"

3. **Inconsistent Keyboard Shortcuts**:
   - ❌ Escape works in some modals but not others
   - ✅ Escape consistently closes all modals and cancels operations

4. **Inconsistent Loading States**:
   - ❌ Some buttons show spinner, others just disable
   - ✅ All async actions show consistent loading indicator

5. **Inconsistent Focus Management**:
   - ❌ Focus lost after modal close, tab order illogical
   - ✅ Focus returns to trigger element, tab order follows visual layout

### Definition of Done

**Code Quality:**
- [ ] All navigation patterns work consistently across pages
- [ ] All data entry interactions follow same model
- [ ] All error states use consistent styling and messaging
- [ ] All keyboard shortcuts work on all applicable screens
- [ ] All loading states use consistent visual feedback

**Testing:**
- [ ] Manual testing checklist completed
- [ ] Keyboard navigation tested on all pages
- [ ] Error handling tested for all forms
- [ ] Loading states verified for all async operations
- [ ] Browser testing completed (Chrome, Firefox)

**Documentation:**
- [ ] UX consistency guidelines documented
- [ ] Keyboard shortcuts reference created
- [ ] Error message templates defined
- [ ] Component interaction patterns documented

**Regression Prevention:**
- [ ] No regressions in existing functionality
- [ ] All previous stories' features still work
- [ ] Visual design from Story 6.2 maintained

### References

- [Source: _bmad-output/planning-artifacts/ux-design-specification.md - Core User Experience]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md - Effortless Interactions]
- [Source: _bmad-output/planning-artifacts/architecture.md - Frontend Stack]
- [Source: _bmad-output/implementation-artifacts/6-2-visual-and-graphical-refinements.md - Previous story context]

## Dev Agent Record

### Agent Model Used

Claude Sonnet 4.5 (claude-sonnet-4-5-20250929)

### Debug Log References

_To be added during implementation_

### Completion Notes List

_To be added by dev agent during implementation_

### File List

_To be added by dev agent during implementation_
