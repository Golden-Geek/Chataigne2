# MapGyver object colors

Golden's typed `NodeMeta.presentation.color` is the single authored color override.
MapGyver enables its normal color-edit permission on Screens, Surfaces, media (including
References) and nested media Folders. Golden's existing manager rows, Inspector header,
Set Color picker and Reset Color action edit that metadata through `PatchMeta`.
Reset removes only the override, restoring the backend default or ordinary theme appearance.
History, independent copies and project persistence use the existing Golden mechanisms.

The public Golden script methods `setColor(r, g, b, a)` and `resetColor()` use the same
metadata/history path. Set accepts four finite numeric components; reset accepts no arguments.
Both require color-edit permission and preserve other presentation metadata. They are generic
node methods, available through ordinary node proxies rather than MapGyver-only script APIs.

MapGyver reads colors by UUID from the existing indexed graph projection. It adds no document
scan, extra metadata request, preview lease or source renderer. Surface and related-source
contours, Composition child-media outlines and the Screen frame use the authored accent.
Selection, hover and invalid-geometry cues retain their distinct shared Golden appearance.
DOM/native Surface outlines share `viewportContourAppearance`; Golden's optional viewport
frame accent keeps the DOM fallback consistent with the native Screen frame.

Colors affect editor presentation only. They do not change channel definitions, media pixels,
mapping geometry, enable state or runtime status. Matrix item context-menu changes belong to
the following M15/13 gate; this feature does not accept that separate requirement.
