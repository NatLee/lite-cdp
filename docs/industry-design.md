# Journey Observatory

The original tracking service separates customer summaries, identity, URL lists, labels and UUID event history. Recorded actions and timestamps are the basis for this redesign: an instrument for reading an observed visit, followed by the existing audience and daily-volume analysis.

The overview loads the six most recently seen people from the selected site's existing people API. Selecting a person loads their latest 50 events. The route groups only the recorded `session_id`, sorts each group by timestamp then event ID, and separates missing IDs into individual records. Session keys and missing-record keys use separate namespaces. Changing the person remounts its event loader so another visitor's route is never reused.

The three lanes show pageviews, interactions and identify events. Connected points mean recorded event sequence within that session, not inferred identity, conversion or elapsed duration. The graphic explicitly states that points are equally spaced and the loaded portion may be incomplete. Native buttons provide keyboard focus and selection; the focus panel shows the selected action, timestamp, path, referrer and exact event properties. The same instrument is used on the person page with its existing type filter and load-earlier controls.

The current-state cohort atlas remains below the journey and daily chart. Cohort percentages retain the whole-audience denominator; daily distinct visitors are never summed into a period-wide unique count. Installation is presented as a three-step observation setup, with the existing tracker, identify and custom-event examples and actual last-event receipt status.

No API, backend, database, permission or dependency changes were needed. The ink/cobalt/cyan identity uses native CSS and SVG plus the existing self-hosted Noto Sans TC. A focused feature screenshot leads the README; the complete overview is available in its expandable gallery.

## Validation

- Frontend: 21 tests, including session boundaries, chronology, missing IDs, keyboard event focus, selected-person fetches and exact detail links; production build.
- Rust: 25 unit and 18 integration tests; fmt and clippy with warnings denied.
- Existing smoke script passed against a new temporary database.
- Actual Chrome: all cohort/list/CSV counts, date slider keyboard controls, recorded session/node counts and visitor changes at desktop/mobile in both themes.
- Actual Chrome: person event-type filter and restoration at 390, 768, 1024 and 1440 pixels; every README route checked for horizontal overflow.
- Every UI image uses a fresh browser document with loaded fonts and settled paint. The editable cover was redrawn as a clearly labelled schematic route rather than fictional customer metrics.

Docker image rebuilding and external integrations are outside this visual iteration's validation.
