# nitrate-mpv

Loads libmpv at runtime (no build-time linking, no headers) and wraps the parts of its C API that [Nitrate](https://github.com/rennerdo30/nitrate) uses:
- the client: options, async commands, observed properties and events;
- the render API with the D3D11 backend.

Part of the Nitrate media player and published for it; the API follows Nitrate's needs and may change between versions. MIT licensed. libmpv itself is a separate LGPL library you provide at runtime.
