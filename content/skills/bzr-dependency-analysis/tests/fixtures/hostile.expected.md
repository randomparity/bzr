# Bugzilla dependency analysis

- Schema: bzr&#45;dependency&#45;analysis/v1
- Status: complete
- Analysis timestamp: 2026&#45;08&#45;28T12:00:00Z
- Bounds: maximum depth 5; maximum nodes 200; maximum relationships 200
- Resolved&#45;node policy: include&#45;no&#45;traverse; resolved statuses RESOLVED
- Duration assumptions: none; weighted critical&#45;path analysis is unsupported
- Evidence gaps: 0 unknown nodes; 0 boundary nodes
- Graph cap reached: false
- Omitted discovered identities: 0
- Relationship cap reached: false
- Omitted relationships &#40;lower bound&#41;: 0
- Scope truncated: false
- Limitations: none
- Collection commands: bug search, bug view, query run

## Scope provenance

- server primary; scope saved&#45;query; saved query release &#91;nightly&#93; &lt;unsafe&gt;; parameter names none
- server primary; scope custom&#45;search; saved query none; parameter names product, status

## Structural findings

- Longest dependency chain by edge count: 1
- Longest dependency chain components: c0001, c0002
- Structural roots: primary&#35;80
- Structural leaves: primary&#35;81
- Bottlenecks: none
- Unassigned blockers: primary&#35;80
- Stale blockers: none
- Execution assumptions: resolved&#45;include&#45;no&#45;traverse
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
