# Security Policy

## Reporting a vulnerability

Please report security vulnerabilities privately rather than opening a
public GitHub issue.

Open a [private security advisory](https://github.com/ThomasJenkinson/cjson-rs/security/advisories/new)
on this repository. We will acknowledge receipt within 7 days and aim to
provide a fix or mitigation within 30 days of disclosure.

## Supported versions

This project is pre-1.0. Only the latest released version is supported.

## Scope

In scope:

- Memory safety violations in the `cjson-rs-ffi` crate or anything it
  exposes via the `cJSON_*` C ABI.
- Parser bugs in `cjson-rs` that cause panics, infinite loops, or
  pathological resource consumption on adversarial input.
- Divergence from RFC 8259 conformance that creates a security-relevant
  ambiguity.
- Divergence from upstream cJSON's documented behaviour that breaks the
  drop-in compatibility claim.

Out of scope:

- Behavioural differences from upstream cJSON that are *not* security-
  relevant (e.g. exact byte format of `cJSON_PrintUnformatted` numeric
  output). Please file a regular GitHub issue for these.
- Bugs in the upstream cJSON test suite itself.
