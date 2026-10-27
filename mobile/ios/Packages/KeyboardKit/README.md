# KeyboardKit 8.9.6 (vendored, patched)

Upstream: https://github.com/KeyboardKit/KeyboardKit (tag 8.9.6, MIT license).

Patch: `Sources/KeyboardKit/Host/KeyboardInputViewController+Host.swift`
— `hostBundleIdValueFor16` crashed the keyboard extension on iOS 27, where the
private `xpc_connection_copy_bundle_id` returns NULL and
`NSString(utf8String:)` throws. The patch returns `nil` for a missing symbol
or a NULL result instead of crashing.
