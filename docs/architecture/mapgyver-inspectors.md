# MapGyver Inspector ownership

Golden's standard Inspector owns ordinary typed Parameter editing and node metadata, including
enable switches, mixed selection and history. MapGyver registers child filters for Layer and
Layer Group Parameters; it does not replace their Inspector component. Column and Cell use the
default Inspector. Media/child-media adapters keep standard Parameters and their existing
channel, asset and contextual operations. Selected Matrix definitions come from bounded
backend reads independently of the virtualized grid's visible rows.

A Surface has one authored enable state: `NodeMeta.enabled`. Surface construction and
`ConfigureSurface` write this metadata; `SurfaceDefinition.enabled` projects it for existing
typed operation/scripting callers. Rendering projects `effective_enabled`, including ancestors,
and invalidates captured work when that state changes. There is no Surface Enabled Parameter
or opaque persisted boolean. Configuration groups Parameter and metadata edits in one history
transaction; generic metadata edits, copy, scripts and sparse project reload use the same state.

Matrix runtime feedback belongs to its grid, separate from authored settings. Requested and
submitted Column/Cell markers remain visible; one compact status cue exposes requested,
installed, submitted, freshness, availability and error details on demand. These are renderer
observations, not display or receiver confirmation. The Inspector contains no Matrix runtime
prose or duplicate property form.
