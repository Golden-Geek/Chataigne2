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

Golden's `NodeColorMenu` and `nodeColorMenuItems` share the same picker and metadata target
between the generic node menu and Matrix's contextual menus. Each palette visit groups its
changes into one Undo step, using the shared latest-edit gesture to bound requests. Closing
flushes the last color; replacing the document invalidates the target. Right-click color
actions affect only their context object and preserve the document's multiselection.

Matrix supplies a sparse Cell target through its typed `SetCellColor` operation. The backend
validates the address and creates a missing Cell with its color in one edit; Reset never
creates a missing Cell. Existing Cells preserve their other presentation metadata. Returned
Cell identity keeps the palette bound to its object when that row leaves the virtual viewport.

Colors affect editor presentation only. They do not change channel definitions, media pixels,
mapping geometry, enable state or runtime status.
