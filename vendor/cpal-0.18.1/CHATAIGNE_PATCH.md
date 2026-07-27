# Chataigne CPAL patch

This directory vendors CPAL 0.18.1 under its upstream Apache-2.0 license.

Chataigne adds one narrow Windows ASIO fix:

- `HostTrait::device_by_id` loads only the driver named by the requested
  `DeviceId`.
- The default CPAL implementation enumerates devices until it finds a match.
  ASIO enumeration loads drivers in registry order, which can initialize an
  unrelated exclusive driver and prevent the selected driver from opening.

The public CPAL API is unchanged. Remove this patch after the equivalent exact
ASIO lookup is available in the upstream dependency.
