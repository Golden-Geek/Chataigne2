bugs :

- create surface, set control mode to animation for its opacity parameter : crash (saying something about negative value). got the same for another opacity somewhere, and for video speed
  This may very well be larger than those cases, and as a general manner, any form of external control should not cause bugs or crash when going outside the actual range of the parameter. it should just crop the incoming value to the authorized range.
- Matrix :

  - UI :
    - layer / column / cell / group selection : i want this as normal selection / highlight mechanism, so no need to outline the cross selected layer/cell, and only the selected element should be highlighted (selection color in golden style). No need to set an additional outline layer/column on the label
    - All of those items should be able to have custom color (golden meta). And reflect this color in the matrix view
    - Small rounded corners on panels, cells, etc. , no or very subtle border (when not selected), make it modern elegant ! I also want to test a subtle drop shadow on active cells (with transition of course)
  - Flow :
    - I want a preset system so that users can reorder / disable / enable elements of the matrix (columns, layers) and set a preset for it, then recall it. cells would stay unchanged for now.
- General UI in editors : as stated in AGENTS rule, i want smooth experience. this means even mouse-hover highlight and selection, when hovering/selecting screen in screen warper panel, but same for surface, composition, matrix, etc. make sure that a short transition is there to avoid direct jumps.
- On this shared selection/highlight system for 2d editors (screen, surface, composition), make the outline and handles more subtle, elegant design
- When opening the app or creating new session, automatically show the auto-created screen in screen warping, and auto-created surface in surface editor
- Surface Editor : I want a toggle option in navbar (icon) to see all surfaces using the same source in the surface editor (from any screen), not just the selected one. This makes it easier to calibrate them in relation to each other. And I when using a testpattern, i want to see it there, even if no source is assigned to this surface.
- Editor snapping :

  - Screen Warping : add a snap (magnet icon) toggle-tool in the navigation/tool bar to allow auto-snapping. This means that when moving a handle of a screen close to another handle of another screen (not the same), it gets snapped and shows a small icon there to show that they're linked. this means that moving one moves the other. you can use the golden-parameter binding system for that. more than 2 can be linked together. clicking on the link-icon aside a handle-group removes all links.
  - Surface Editor / Composition-Matrix Editor : add a snap (magnet icon) toggle-tool that makes auto aligning on close geometry while moving: for instance, a rectangle is put, moving or scaling another rectangle makes it snap to aligned X or aligned Y when getting close to the axis of an existing other element. This is "standard" behaviour for software like photoshop, illustrator, or any 2d editor. I hope i'm clear.
- Architecture : Gradient should definitely be in golden ! it should be available as a node in itself, like an automation, or when using animation control mode on a color parameter.
- Shared Texture : don't auto select internal generated textures (from screen outputs), when a shared texture is created, auto select the first outside texture or none. Mark internal textures "(internal)".
