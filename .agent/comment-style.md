# Project rules

## Comments: compress without gutting the WHY

A compressed comment should be readable at a glance and still carry the one
thing the code alone can't say: a non-obvious invariant, an ordering
constraint, or the reason a workaround exists. Everything else — restated
behavior, framework mechanics visible in the code, changelog notes — gets cut.

### How to compress
- Terse and scannable. State the fact, skip the throat-clearing.
- Minimize relying on specific code objects — identifiers, literal paths,
  error strings — as the mechanism that carries the comment's meaning.
  Backticks are just the tell: a comment wrapping five things in backticks is
  usually a comment that explains itself by pointing at five things, and
  every one of them can drift out of sync with the comment independently.
  State the general rule; don't make the reader reconstruct it by
  cross-referencing a list of named things.
  - The test: delete the specific names/examples. Is the sentence still a
    complete, correct statement of the rule? If yes, they were color, and
    keeping a couple for scannability is fine. If the rule only exists by
    implication from the list, that's the anti-pattern — state the rule.
  - Fine (general rule stated; examples are illustration, not the
    explanation — deleting `(/about, /robots.txt, a typo)` still leaves a
    correct sentence): `// most of what lands here (/about, /robots.txt, a
    typo) was never a real share link`
  - Not fine (no rule without the list — deleting the names leaves nothing):
    `// handles /about, /robots.txt, and typo'd links`
  - A single necessary pointer is always fine — name the one thing the
    comment can't be understood without (a route pattern, a sibling
    function).
- A multi-part explanation becomes a short bulleted list — one clause per
  line — not one paragraph strung together with em-dash clauses.
- Each bullet or line should be much shorter than a terminal width. If a
  line wraps, split it or cut it further.
- Prefer one line over three, and three short lines over one long paragraph.

### What never needs a comment
- Anything already obvious from the variable, function, or type name. If
  the name says what the line does, restating it is noise.
  - e.g. `// increment the counter` above `counter += 1;` — delete, don't
    compress.
- What changed and why relative to a previous version ("switched from X to
  Y because Z used to..."). That belongs in the commit message. Code
  comments describe the current state, not its history — a changelog
  embedded in a comment goes stale the moment someone touches the line
  again without updating the note.

### What must survive compression
- The one-sentence reason a line exists, when removing it would silently
  reintroduce a bug.
- Ordering/timing constraints between two pieces of code that aren't
  colocated (e.g. "must run before X", "the framework calls this before
  render").
- Why a workaround exists, when the obvious/naive version wouldn't work.

### Examples

Chatty module doc (127 words, 7 backtick terms):
```
//! Confirms before router-initiated navigation (a `Link` click, programmatic
//! nav) leaves a page with unsaved input - the gap `unsaved_guard`'s
//! `beforeunload` can't cover, since the router never unloads the document
//! for these. The browser's back/forward buttons are a third path neither of
//! those sees - a same-document `popstate` for which the router never calls
//! `on_update` - and are guarded by `unsaved_guard`'s popstate listener.
//!
//! Wired in as the router's `on_update` callback, which - per
//! `dioxus_router` - runs *after* the history entry has already changed but
//! *before* components/hooks re-render for it. That ordering is what makes
//! blocking possible at all: returning a route from here makes the router
//! silently replace the history entry with it (a second, un-observed change)
//! before anything re-renders, so a declined navigation never becomes
//! visible - not even as a flash of the page being left.
```
compresses to (44 words, 1 backtick term) — same invariant, no chatter:
```
//! Blocks Link/programmatic navigation away from unsaved input.
//! Tab close and back/forward are unsaved_guard's job instead.
//!
//! Runs as on_update, which fires after the history entry changes
//! but before re-render - so reverting here swaps history again
//! before anything paints. No flash of the abandoned page.
```

Gutting a comment silently reintroduces the bug it was guarding against —
this is what compression must not do:
```
// The input just became the saved poll; without this the
// push below would still see the dirty flag and confirm
// leaving it.
mark_clean();
```
Compress instead of deleting:
```
// Without this, the push below still sees the dirty flag.
mark_clean();
```
