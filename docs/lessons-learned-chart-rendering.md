# Lessons Learned: Chart Rendering Bugs

**Date:** 2026-02-10
**Epic:** 5 Retrospective
**Story:** 2.1 - Logarithmic SSG Chart Rendering

## Summary

Story 2.1 was marked "done" in Epic 2 but had THREE critical bugs that prevented the chart from rendering. These bugs went undetected through Epics 3, 4, and 5 until discovered during Epic 5 retrospective.

## The Three Bugs

### Bug 1: Zero Dimensions in WasmRenderer

**Problem:**
```rust
let renderer = WasmRenderer::new(0, 0);  // WRONG - zero dimensions
```

**Symptom:** Chart canvas has no size, nothing can render

**Fix:**
```rust
let renderer = WasmRenderer::new(1200, 600);  // Explicit dimensions
```

**Lesson:** Always initialize renderers with actual dimensions, never 0,0. Verify the initialization parameters match the container size.

**Prevention:** Add a unit test that verifies renderer dimensions are non-zero.

---

### Bug 2: Missing JavaScript Dependency

**Problem:** The `charming` library uses ECharts under the hood, but the ECharts JavaScript library was never added to `index.html`.

**Symptom:** Chart fails silently because `echarts` object doesn't exist in browser

**Fix:** Added to `frontend/index.html`:
```html
<script src="https://cdn.jsdelivr.net/npm/echarts@5.4.3/dist/echarts.min.js"></script>
```

**Lesson:** When using a Rust library that wraps JavaScript (like charming wrapping ECharts), verify ALL JavaScript dependencies are loaded in the HTML.

**Prevention:**
- Document JavaScript dependencies in component documentation
- Add browser console check: `typeof echarts !== 'undefined'`
- E2E test that verifies required globals exist

---

### Bug 3: DOM Timing Issue

**Problem:** The `Effect` tried to render the chart before the DOM element was mounted:
```rust
renderer.render(&cid, &chart).ok();  // Runs immediately
```

**Symptom:** Error: `no element with id 'ssg-chart-aapl' found`

**Fix:** Defer rendering until next browser frame:
```rust
let window = web_sys::window().expect("no global window");
let render_callback = Closure::once(Box::new(move || {
    let renderer = WasmRenderer::new(1200, 600);
    renderer.render(&cid, &chart).ok();
}) as Box<dyn FnOnce()>);

window.request_animation_frame(render_callback.as_ref().unchecked_ref()).ok();
render_callback.forget();
```

**Lesson:** In reactive frameworks like Leptos, Effects run immediately but the view may not be mounted yet. Use `requestAnimationFrame` to defer DOM manipulation until after the render cycle completes.

**Prevention:**
- Always defer canvas/chart rendering with `requestAnimationFrame`
- Never assume DOM elements exist synchronously in Effects
- Log errors instead of using `.ok()` to silence them

---

## Root Cause Analysis

### Why weren't these caught earlier?

1. **No E2E Tests:** No automated test actually opened a browser and verified the chart rendered
2. **No Visual Verification:** "Done" was based on code existence, not working feature
3. **Silent Error Handling:** `.ok()` discarded the render error, hiding the DOM timing issue
4. **No Manual Testing:** Nobody actually ran the application and looked at the chart
5. **No Definition of Done:** No checklist requiring visual verification

### Cost

- **Time:** Hours of debugging during Epic 5 retrospective
- **User Experience:** Core feature (SSG chart) was non-functional for 4 epics
- **Technical Debt:** Three bugs accumulated rather than being caught early
- **Trust:** Uncertainty about what other "done" stories might not work

## Checklist for Future Chart Components

Before marking a chart component "done":

- [ ] Chart canvas initializes with non-zero dimensions
- [ ] All required JavaScript libraries loaded in HTML
- [ ] Rendering deferred with `requestAnimationFrame` or similar
- [ ] Error handling logs errors instead of silencing them
- [ ] Visual verification: Open browser, see the chart
- [ ] Browser console: Zero errors
- [ ] Network tab: All dependencies load successfully
- [ ] Chart responds to data changes
- [ ] Chart responds to window resize (if applicable)

## References

- Affected Files:
  - `frontend/src/components/ssg_chart.rs`
  - `frontend/index.html`
- Epic 5 Retrospective: 2026-02-10
- Related Action Items: E2E Testing, Definition of Done
