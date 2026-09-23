## Unreleased

* Added a **Formula** operation to the Math panel: free-text expressions over
  traces (`{name}` syntax, so names with spaces work), the time variable `t`,
  and constants `pi`/`e`. Supports `+ - * / ^`, parentheses, implicit
  multiplication, and the functions `sqrt`, `root(x,n)`, `exp`, `sin`, `cos`,
  `tan`, `asin`, `acos`, `atan`, `atan2(y,x)`, `sinh`, `cosh`, `tanh`, `asinh`,
  `acosh`, `atanh`, `abs`, `ln`, `log10`, `log2`, `log(x)` (base-10) and
  `log(x,b)` (arbitrary base). The editor provides colored insert-chips for
  every trace plus `t`/`pi`/`e` and a function menu, all inserting at the text
  cursor, and renders a live LaTeX-style preview where parenthesized divisions
  appear as stacked fractions with a horizontal line. Formula math traces are
  stateless, evaluate on the union of the referenced traces' timestamps
  (linear interpolation), and skip non-finite results as gaps. Pure `f(t)`
  formulas like `sin(2*pi*t)` work without any input trace.
* `MathTrace::input_trace_names` now returns owned `Vec<TraceRef>` instead of
  `Vec<&TraceRef>` (needed to report formula-referenced traces).
* Added Markers to the Measurement panel: multiple named, colored single-point
  markers with optional horizontal/vertical lines and selectable dot shape,
  catch-trace snapping, per-marker visibility, drawn on all scopes, plus a
  Clear All action, `MARKER_*` events, and a `take_marker_change()` /
  `set_markers()` API for external synchronization across panes/tabs.

## v0.3.0

* Added threshold events
* Support Y axis with units
* Reworked traces menu
* Custom colors support
* Added possibility to control & readout position & size of plot window
* Rework API with unified config as opposed to multiple `run_...` functions
* Various UI improvements
* See git log for details
* Support embedded windows

## v0.2.0

* Added Math functionality
* See git log for details
