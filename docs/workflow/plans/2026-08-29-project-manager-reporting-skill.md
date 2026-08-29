# Project-manager reporting skill implementation plan

Goal: satisfy issue #567 with the smallest embedded skill and a prompt-to-PM-artifact demonstration.

1. Add failing embedded inventory, skill contract, and functional expectations for
   `bzr-project-manager-reporting`.
2. Add the skill entrypoint plus focused format-safety and report-template references. Validate
   documented commands against the compiled CLI and exercise hostile spreadsheet/HTML examples.
3. Extend the live functional suite with the saved-query, projected pagination, JSON/NDJSON,
   whiteboard, and comments path used by the demo.
4. Add a demo mode whose visible cast begins with an agent prompt and ends with a PM-ready report;
   document and record it.
5. Run skill validation, focused tests, the live Bugzilla phase, formatting/lint, and the full test
   guardrail. Review security boundaries and simplify before delivery.
