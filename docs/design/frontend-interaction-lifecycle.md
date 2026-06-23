# Frontend Interaction Lifecycle

Short-lived UI interactions often wait for `nextTick`, dynamic imports, idle work, or service calls before touching state again. Those delayed continuations must prove they still belong to the current component and the current interaction.

Use `withComponentEpoch()` for this boundary:

- `nextEpoch()` starts a new async interaction and returns the token that the continuation must capture.
- `invalidate()` cancels older continuations when the user cancels, switches target, collapses a panel, or starts a newer interaction.
- `assertAlive(epoch)` replaces local `disposed && seq === currentSeq` checks. It returns false after unmount or after a newer epoch invalidates the captured token.

Keep component code declarative. Put reusable interaction state in small composables, such as `useInlineRename` for rename drafts and `useFocusOnActivation` for delayed focus. Component tests should assert visible behavior; composable tests should cover cancellation, reset, and stale-continuation rules.
