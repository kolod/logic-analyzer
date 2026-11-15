# Verification log: 007 decoder framework and annotations

Manual checks follow `quickstart.md`. Automated results are in CI; this file records what a person
looked at.

## Phase 3 (User Story 1): decode a signal and see what it says

| Date | Platform | Step | Result |
|------|----------|------|--------|
| 2026-11-15 | Windows 11 (headless render, `iced_test`) | quickstart 8.1 to 8.3: capture on the simulated device, add the sample decoder, choose channels, wait for the pass, render at two zoom levels (`LA_SNAPSHOT_DIR`) | The pictures show the **Decoders** menu and panel, the lane "Clocked bits" with ranges, bit dots and a red incomplete-byte span, the edge arrows on the clock channel and the dots on the data channel. Looked at by the implementer from the written PNG. |
| pending | Windows 11, running application | the same steps by hand, plus click to select, double click to go to, F8 / Shift+F8 | **Not done yet**: needs a person at the window. |
| pending | Debian | the same | **Not done yet**: Debian results are asked of the user, not claimed. |

Known gaps against the task text (to close in the polish phase): no hover readout of an annotation's
text (the selected annotation is described in the panel instead), and no separate `decoders.rs`
module in `la-app` (the panel is in `app.rs` next to the other panels).
