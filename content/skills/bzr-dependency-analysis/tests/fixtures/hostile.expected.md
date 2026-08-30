# Bugzilla dependency analysis

- Schema: bzr-dependency-analysis/v1
- Status: complete
- Analysis timestamp: 2026-08-28T12:00:00Z
- Bounds: maximum depth 5; maximum nodes 200; maximum relationships 200
- Resolved-node policy: include-no-traverse; resolved statuses RESOLVED
- Traversal direction: both
- Unassigned-assignee policy: none
- Duration assumptions: none; weighted critical-path analysis is unsupported
- Evidence gaps: 0 unknown nodes; 0 boundary nodes
- Graph cap reached: false
- Omitted discovered identities: 0
- Relationship cap reached: false
- Omitted relationships (lower bound): 0
- Scope truncated: false
- Limitations: none
- Collection commands: bug search, bug view, query run

## Scope provenance

- server primary; scope saved-query; saved query release &#91;nightly&#93; &lt;unsafe&gt;; parameter names none
- server primary; scope custom-search; saved query none; parameter names product, status

## Structural findings

- Longest dependency chain by edge count: 1
- Longest dependency chain components: c0001, c0002
- Structural roots: primary#80
- Structural leaves: primary#81
- Bottlenecks: none
- Unassigned blockers: primary#80
- Stale blockers: none
- Execution assumptions: resolved-include-no-traverse
- Execution component order: c0001, c0002
- Cycle impediments: none
- Incomplete boundaries: none
- Analysis warnings: none

## Structural execution groups

- Layer 1: c0001
- Layer 2: c0002

## Node inventory

````text
{"boundary_reason":null,"error_type":null,"identity":"primary#80","stale":false,"state":"known","status":"NEW","summary":"<script>alert(1)</script> ![image](https://evil.example/x) [outer [inner]](https://evil.example) <https://evil.example> ``` %%{init: {\"theme\":\"dark\"}}%% \"quoted\" \\\\ path\nnext line"}
{"boundary_reason":null,"error_type":null,"identity":"primary#81","stale":false,"state":"known","status":"NEW","summary":"</script> ```mermaid\nA[\"breakout\"] --> B\\tail"}
````

## Limitations

- none
