# Sparse node identity

Golden Engine owns project codecs and immutable project captures. Node types declare whether
their identity remains independently addressable when all their declared contents are default.

`Node::project_preserve_identity()` defaults to false. Override it for a node type whose UUID
must survive saving and loading even while nothing references it. The UUID stays in the normal
project record; the policy adds no persisted field or authored value. Ordinary unreferenced
defaults remain eligible for omission.

Live sparse encoding and immutable captures apply the same policy. Capture records freeze the
policy beside the node's authored metadata, so background encoding does not access live nodes.
Node enums forward the public hook to their concrete node. Implementations should treat this
as a stable property of the node type, not a changing runtime setting.

The engine's `app/tests/sparse_identity.rs` checks both codecs, preservation through loading and
continued omission of an ordinary default sibling. Copy/import retains its existing explicit
identity-remapping semantics.
