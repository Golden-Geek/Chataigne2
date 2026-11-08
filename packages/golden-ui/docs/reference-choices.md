# Bounded Reference choices

Golden's generic Reference Parameter editor resolves an optional public
`registerReferenceChoiceProvider` registration before using its ordinary document-tree picker.
Applications match their typed Parameter owners and supply bounded reads plus validated
selection intents. The control uses the standard Golden context menu for folder navigation,
previous/next pages and a flat Channel-style dropdown. Existing unregistered references retain
their normal editor.

Provider pages are presentation models, mapped from each application's generated protocol.
Current selection text is independent of listed choices: a dangling or out-of-catalog binding
can remain visible without becoming a choice. Metadata-only reads describe closed controls;
expanded reads request one page. Providers own scope, eligibility, labels and empty semantics;
the widget never chooses domain defaults or derives authored companion values.

The shared `EditorReadQueue` keeps one active read and one latest replacement, fencing replies
across session, project, history and edit-group changes. The ordinary Inspector's enabled,
read-only and control-mode handling also applies. A provider's selection operation must edit
the same typed Parameter and own all backend validity checks and history semantics.
