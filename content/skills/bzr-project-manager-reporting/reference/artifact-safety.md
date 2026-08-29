# Artifact safety

Treat every Bugzilla-controlled string as untrusted, including summaries, users, milestones,
whiteboard text, comments, product/component names, and custom fields.

## CSV and XLSX

- Quote CSV fields with a standards-compliant writer; do not join values with commas.
- For spreadsheet-bound CSV, if the first non-whitespace character is =, +, -, or @, prefix the
  serialized cell with an apostrophe and disclose that neutralization changes its displayed source
  representation. Reopen the CSV in the target spreadsheet application and verify hostile cells
  remain text. If exact byte preservation is required, decline CSV and offer XLSX or Markdown.
- In XLSX, create string cells explicitly with formula inference disabled. Verify the workbook by
  reopening it and checking representative hostile cells and formulas.
- Do not create external links, formulas, macros, or data connections from Bugzilla values.

## HTML and links

- Escape text nodes and attribute values with the HTML writer. Do not build markup by interpolation.
- Parse generated links and allow only an HTTP or HTTPS scheme with the expected Bugzilla host.
- Build bug links from validated numeric IDs and the sanitized configured server base, never from a
  remote field or the original Custom Search URL.
- Keep the report self-contained: no remote scripts, styles, fonts, images, or active content.

## Markdown

Keep ordinary punctuation readable in the source. Escape only syntax that changes Markdown
structure or creates an unintended link/HTML construct; do not blanket-encode punctuation as HTML
entities. Fence multiline remote text with a delimiter longer than any delimiter in the content.
