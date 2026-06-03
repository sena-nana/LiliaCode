# Chat Composer IAB

## Intent

Chat Composer is redesigned as a focused input workspace: the main editor keeps inline context references, pending agent actions temporarily take over the input, and controls are grouped into a two-level toolbar.

## Behavior Model

- Normal state shows one rich input surface. Text, inline file references, pasted images, pasted paths, and long pasted text all flow through the same context model.
- Pending state takes over the input surface. AskUser, tool consent, and plan approval continue to use the composer entry area for answers, rejection notes, or modification requests.
- Running state with an empty composer turns the send action into interrupt. Running state with content still queues a new message.

## Layout

- Stage: pending panel or rich input, followed by the context search panel when active.
- Toolbar: image previews first, then attachment, permission, plan, and send/interrupt controls.
- The visual language stays within existing `chat-composer`, `composer-inline`, `chat-chip`, and `chat-attachment-chip` classes.

## Confirmation Notes

- Public props and emits on `ChatComposer` stay unchanged.
- No contracts changes are required.
- `prefers-reduced-motion` behavior is not added or altered.
