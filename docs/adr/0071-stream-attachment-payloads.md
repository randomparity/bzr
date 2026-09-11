# ADR 0071: Stream authenticated attachment payloads into private staging files

## Status

Accepted

## Context

Issue #756 owns the attachment download ceiling introduced by ADR0068. The operator
approved authenticated REST/XML-RPC streaming and replacement of the pre-release
buffered download API. An attachment can be larger than the structured-response limit
without permitting arbitrary metadata or diagnostics to grow without bound.

## Decision

Keep existing authenticated API endpoints. Incrementally decode attachment data into
an anonymous temporary file, retain a bounded payload-elided envelope, and validate it
with existing parsers plus complete XML framing and selected-ID checks. Return a
positioned, length-limited reader over the selected candidate range; extra records and
data-before-ID ordering remain supported. Command output copies from
that file. Metadata reads used by batch download omit data server-side in both protocols.
Replace the Vec-returning pre-release download interface with a length-limited File reader interface.
The existing pinned tempfile package becomes a runtime dependency.

## Consequences

RAM remains bounded by the envelope limit plus small transfer/decoder buffers while
temporary disk usage scales with decoded data. Network/decoding failures occur before
opening output destinations. Disk exhaustion is an I/O error. Existing output paths,
symlink behavior, result shapes, credentials and protocol dispatch remain unchanged.
Non-download structured responses retain ADR0068's shared bound.

## Considered & rejected

- **Use attachment.cgi.** verified: Bugzilla source commit
  `276673ab67562bd27676d8c3a3770ec9897637b8`, Auth/Login/APIKey.pm lines 27–35,
  rejects API keys outside web services; attachment.cgi invokes ordinary login.
  This cannot preserve API-key access to private/restricted attachments.
- **Raise or remove the response limit.** judgment: restores unbounded client memory
  exposure and contradicts #756's requirement to retain bounded structured reads.
- **Write directly to the final output while parsing.** judgment: later decoding,
  truncation, or envelope errors would damage destinations that remain untouched today.
- **Add cookie/password authentication.** judgment: unnecessary credential surface
  when existing authenticated API transports already provide the required bytes.
