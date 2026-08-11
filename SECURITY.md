# Security Policy

## Scope

ccidkit talks to USB smart card readers and to the platform smart card service, and it
parses what they answer. The realistic threat is a malicious or broken device: a
crafted ATR, a CCID message whose `dwLength` lies, a descriptor that overruns, or an
interrupt stream that never settles. A panic, an unbounded allocation, or
non-termination driven by a device's answer is a security issue here, because callers
embed this in login flows and signing tools.

Specifically in scope:

- Panics on any byte sequence a reader or card can produce, including malformed ATRs,
  truncated CCID messages, and descriptor lengths that disagree with received counts
- Unbounded memory growth or non-termination driven by a device's answers
- Integer overflow in length or offset arithmetic producing a wrong read rather than a
  defined error
- A `SAFETY` contract in the PC/SC shim that does not hold

Out of scope: a reader that misbehaves functionally (that is a quirk entry, see
[the reader report template](.github/ISSUE_TEMPLATE/reader_report.yml)), and the
security of the cards themselves.

## Reporting

Report privately through GitHub's ["Report a vulnerability"][advisories] flow rather
than a public issue. Include the device (vid:pid if known), the bytes or capture that
trigger it, and the observed behavior.

Expect an acknowledgement within seven days.

## Supported versions

While the project is pre-1.0, only the latest release receives fixes.

[advisories]: https://github.com/P4suta/ccidkit/security/advisories/new
