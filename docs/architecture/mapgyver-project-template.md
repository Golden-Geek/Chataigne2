# MapGyver fresh project template

MapGyver owns its document template through Golden's `ProjectLifecycle::initialize_new_project`.
The shared desktop and headless hosts use that hook for startup and New. Loading a saved project
uses the codec/candidate lifecycle instead and never seeds missing Screens.

The Screens manager, initial user Screen and its user Surface are one detached `NodeTree`.
Golden attaches and stabilizes the complete candidate before publication and clears bootstrap
history. The initial objects remain ordinary typed, selectable, removable document children.
No frontend effect repairs the hierarchy or counts Screens.

Golden's detached-candidate augmentation delivers generated-child/cache events to declaration
owners above Parameters folders before publication. That walk is bounded by the generated
owner-relative declaration path; ordinary nodes retain direct-parent routing. Live subscriptions
and app inbox callbacks stay excluded, preserving the detached validation/activation boundary.

The initial Screen shares the ordinary creation factory's no-display fallback: enabled,
Raw anti-aliasing, `MapGyver Screen` shared-texture destination at 1920×1080, with one unassigned
Surface. Project construction precedes native display discovery, so this deterministic default
also works headlessly. Later display catalogs publish choices without changing that authored
destination. Ordinary subsequent creation can choose an available secondary monitor and reserves
existing Screen labels/sender names through the same backend policy.

An intentionally empty loaded project stays empty. The app lifecycle tests cover exact
zero/one/multiple Screen persistence and initial history; the owned `initial-screen.mjs` fixture
covers ordinary desktop/headless startup, New/Load, native selection and output replacement.
