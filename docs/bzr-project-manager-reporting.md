# Project-manager reporting

The bundled `bzr-project-manager-reporting` skill turns a saved query or Bugzilla Custom Search URL
into a decision-ready CSV, XLSX, self-contained HTML, or Markdown artifact when the active agent has
the matching artifact capability. It falls back explicitly instead of pretending a text file is a
workbook or webpage.

Install it into a project with:

```sh
bzr skills install --agent codex --project .
```

The [asciinema cast](assets/bzr-project-manager-reporting-demo.cast) shows the intended interaction:
a program manager asks an agent for an analysis, and the final screen is the complete report they can
use. The recording deliberately hides setup and transformation plumbing.

Regenerate it against a populated local functional Bugzilla server:

```sh
BZR_BIN="$PWD/target/release/bzr" tools/record-demo.sh project-manager-reporting
```

Status Whiteboard is a standard Bugzilla field that an installation may disable. It represents a
mutable current snapshot; Bugzilla comments provide the durable update history.
