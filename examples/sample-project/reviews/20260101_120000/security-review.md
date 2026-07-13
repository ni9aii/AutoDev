# Security Review Report

### [CRITICAL] Hardcoded credentials in example config
File: `examples/app.conf`
Line: 7
Description: A plaintext API key is committed in the example config. Even though
it is an example, users copy-paste it verbatim. Load secrets from the environment.

### [IMPORTANT] Unbounded request buffer in `net::accept`
File: `src/net.rs`
Line: 130
Description: The read loop appends to a Vec without a size cap. A slowloris-style
client can exhaust memory. Cap at a configurable MAX_REQUEST_BYTES.
