# Code Review Report

### [CRITICAL] SQL injection in db.rs query builder
File: `src/db.rs`
Line: 42
Description: User-supplied `name` is concatenated directly into the SQL string
instead of being passed as a bound parameter. An attacker can inject arbitrary
SQL via the query parameter.

### [IMPORTANT] Missing input validation on `parse_config`
File: `src/config.rs`
Line: 88
Description: `parse_config` trusts the `timeout` field without range checks.
A negative value silently disables the watchdog.

### [MINOR] Trailing whitespace in `src/net.rs`
File: `src/net.rs`
Description: Several lines end with trailing spaces; breaks strict lint gates
in CI.
