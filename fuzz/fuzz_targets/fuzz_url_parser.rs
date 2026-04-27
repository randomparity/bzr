#![no_main]
use libfuzzer_sys::fuzz_target;

use bzr::config::Config;

fuzz_target!(|data: &str| {
    let config: Config = toml::from_str(
        r#"
default_server = "test"

[servers.test]
url = "https://bugzilla.example.com"
api_key = "dummy"
"#,
    )
    .expect("static TOML config is always valid");

    let _ = bzr::url_parser::parse_bugzilla_url(data, &config);
});
