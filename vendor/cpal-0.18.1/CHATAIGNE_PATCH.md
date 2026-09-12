# Chataigne CPAL patch

This directory vendors CPAL 0.18.1 under its upstream Apache-2.0 license.

Chataigne adds one narrow Windows ASIO fix:

- `HostTrait::device_by_id` loads only the driver named by the requested
  `DeviceId`.
- The default CPAL implementation enumerates devices until it finds a match.
  ASIO enumeration loads drivers in registry order, which can initialize an
  unrelated exclusive driver and prevent the selected driver from opening.

The patch was introduced by Chataigne commit
`b91bb32a0ad19f6035428b0001be84ef1f31b8d2`. Upstream CPAL's ASIO host still
uses the default iterator-based lookup at the time this note was last reviewed.

The public CPAL API is unchanged. `golden_audio` is distributed from a reviewed
Chataigne Git revision, which includes this vendored path dependency. A detached
registry publication is deliberately unsupported while the patch is required,
because Cargo packages cannot publish with a repository-relative path dependency.
`tools/qualification/external_audio_consumer.py` compiles an independent Git
consumer and verifies that it resolved this exact vendored implementation.

Remove the vendor directory and return the workspace dependency to the upstream
release only after upstream exposes an equivalent exact ASIO lookup and the
external-consumer qualification passes without the source-marker assertion.
