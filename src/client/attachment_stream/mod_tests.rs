#![expect(clippy::unwrap_used, clippy::panic)]

use std::io::Read;

use super::{extract_source, Protocol, Source};

async fn read(
    input: &[u8],
    protocol: Protocol,
    chunk: usize,
    limit: u64,
) -> super::Result<super::Extracted> {
    let chunks = input.chunks(chunk).map(<[u8]>::to_vec).collect();
    extract_source(Source::Chunks(chunks), protocol, limit).await
}

fn xml(data: &str) -> String {
    format!("<methodResponse><params><param><value><struct><member><name>data</name><value>{data}</value></member></struct></value></param></params></methodResponse>")
}

fn xml_marker(body: &str) -> String {
    use crate::xmlrpc::protocol::{parsing::parse_response, Value};
    let parsed = parse_response(body).unwrap();
    let Value::Struct(map) = parsed else {
        panic!("not a struct")
    };
    match &map["data"] {
        Value::Base64(bytes) => {
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
        }
        Value::String(s) => s.clone(),
        other => panic!("unexpected data {other:?}"),
    }
}

#[tokio::test]
async fn attachment_stream_json_chunk_boundaries_and_candidate_selection() {
    let input = br#"{"attachments":[{"data":"QQ==","id":1},{"da\u0074a":"Q\u0067==","id":2}]}"#;
    for chunk in 1..=input.len() {
        let extracted = read(input, Protocol::Json, chunk, 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_str(&extracted.body).unwrap();
        let marker = body["attachments"][1]["data"].as_str().unwrap();
        let mut stream = extracted.select(Some(marker), 2).unwrap();
        assert_eq!(stream.limit(), 1);
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"B");
    }
}

#[tokio::test]
async fn attachment_stream_xml_chunk_boundaries_entities_cdata_and_strings() {
    for payload in [
        "<base64>Q&#81;==</base64>",
        "<string>QQ==</string>",
        "<base64><![CDATA[QQ==]]></base64>",
        "<![CDATA[QQ==]]>",
        "<!-- prefix -->QQ==",
        "QQ==",
        "<base64>\n QQ== \n</base64>",
    ] {
        let input = xml(payload);
        for chunk in 1..=input.len() {
            let extracted = read(input.as_bytes(), Protocol::Xml, chunk, 1024)
                .await
                .unwrap();
            let marker = xml_marker(&extracted.body);
            let mut stream = extracted.select(Some(&marker), 1).unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes, b"A", "chunk {chunk}, payload {payload}");
        }
    }
}

#[tokio::test]
async fn attachment_stream_payload_can_exceed_metadata_limit() {
    for (input, protocol) in [
        (
            format!(r#"{{"data":"{}"}}"#, "QUFB".repeat(1024)),
            Protocol::Json,
        ),
        (
            xml(&format!("<base64>{}</base64>", "QUFB".repeat(1024))),
            Protocol::Xml,
        ),
    ] {
        let extracted = read(input.as_bytes(), protocol, 7, 512).await.unwrap();
        assert!(extracted.body.len() < 512);
        assert_eq!(extracted.file.metadata().unwrap().len(), 3072);
    }
}

#[tokio::test]
async fn attachment_stream_metadata_is_still_bounded() {
    for (input, protocol) in [
        (
            format!(r#"{{"message":"{}"}}"#, "a".repeat(200)),
            Protocol::Json,
        ),
        (
            xml(&format!("<string>{}</string>", "A".repeat(200)))
                .replace("<name>data", "<name>name"),
            Protocol::Xml,
        ),
    ] {
        assert!(matches!(
            read(input.as_bytes(), protocol, 1, 100).await,
            Err(crate::error::BzrError::ResponseTooLarge { .. })
        ));
    }
}

#[tokio::test]
async fn attachment_stream_invalid_base64_is_rejected_only_when_selected() {
    for encoded in ["A", "!!!!", "QQ==QQ==", "QR=="] {
        let input = format!(r#"{{"data":"{encoded}"}}"#);
        let extracted = read(input.as_bytes(), Protocol::Json, 1, 1024)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&extracted.body).unwrap();
        assert!(extracted.select(value["data"].as_str(), 1).is_err());
    }
    let extracted = read(
        br#"[{"data":"!!!!"},{"data":"QQ=="}]"#,
        Protocol::Json,
        1,
        1024,
    )
    .await
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&extracted.body).unwrap();
    assert!(extracted.select(value[1]["data"].as_str(), 1).is_ok());
}

#[tokio::test]
async fn attachment_stream_rejects_truncated_or_malformed_framing() {
    for input in [
        r#"{"data":"QQ==""#,
        r#"{"data":"Q\x41=="}"#,
        r#"{"data":"QQ==","data":"Qg=="}"#,
    ] {
        assert!(read(input.as_bytes(), Protocol::Json, 1, 1024)
            .await
            .is_err());
    }
    let complete = xml("<base64>QQ==</base64>");
    for len in 0..complete.len() {
        assert!(
            read(&complete.as_bytes()[..len], Protocol::Xml, 1, 1024)
                .await
                .is_err(),
            "prefix {len}"
        );
    }
}

#[tokio::test]
async fn attachment_stream_ignores_fake_markup_and_decodes_empty_data() {
    for input in [xml("<base64/>"), xml("<string/>")] {
        let input = input.replace("<params>", "<!-- <base64>AAAA</base64> --><params>");
        let extracted = read(input.as_bytes(), Protocol::Xml, 1, 1024)
            .await
            .unwrap();
        let marker = xml_marker(&extracted.body);
        assert_eq!(extracted.select(Some(&marker), 1).unwrap().limit(), 0);
    }
}

#[tokio::test]
async fn attachment_stream_rejects_duplicate_xml_members() {
    let input = xml("<base64>QQ==</base64>").replace(
        "</struct>",
        "<member><name> data </name><value><base64>Qg==</base64></value></member></struct>",
    );
    assert!(read(input.as_bytes(), Protocol::Xml, 1, 1024)
        .await
        .is_err());
}

#[tokio::test]
async fn attachment_stream_rejects_xml_value_before_member_name() {
    let input = xml("<string>YnpyLXBheWxvYWQtMA==</string>")
        .replace("<member><name>data</name><value>", "<member><value>")
        .replace("</value></member>", "</value><name>data</name></member>");
    assert!(read(input.as_bytes(), Protocol::Xml, 1, 1024)
        .await
        .is_err());
}

#[tokio::test]
async fn attachment_stream_xml_markup_limit_keeps_size_error() {
    let mock = wiremock::MockServer::start().await;
    let input = xml(&format!("<base64><!--{}-->QQ==</base64>", "a".repeat(512)));
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(input))
        .mount(&mock)
        .await;
    let response = reqwest::get(mock.uri()).await.unwrap();
    assert!(matches!(
        super::extract_with_limit(response, Protocol::Xml, 256).await,
        Err(crate::error::BzrError::ResponseTooLarge {
            limit_bytes: 256,
            ..
        })
    ));
}
