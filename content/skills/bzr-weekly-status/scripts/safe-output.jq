def spreadsheet_text: tostring | if test("^[=+@-]") then "'" + . else . end;
def html_text: tostring | gsub("&"; "&amp;") | gsub("<"; "&lt;") | gsub(">"; "&gt;") | gsub("\""; "&quot;") | gsub("'"; "&#39;");
def safe_http_url: tostring | if test("^https?://"; "i") then . else null end;
