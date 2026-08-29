def spreadsheet_text: tostring | if test("^[\\t\\r\\n]*[=+@-]") then "'" + . else . end;
def html_text: tostring | gsub("&"; "&amp;") | gsub("<"; "&lt;") | gsub(">"; "&gt;") | gsub("\""; "&quot;") | gsub("'"; "&#39;");
