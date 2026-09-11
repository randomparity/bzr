//! Attachment-only payload extraction. Structured readers retain their shared bound.

use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Take, Write};

use base64::Engine;

use crate::error::{io_with_context, BzrError, Result};

mod json;
mod xml;

pub(crate) enum Protocol {
    Json,
    Xml,
}

struct Span {
    offset: u64,
    length: u64,
    error: Option<String>,
}

pub(crate) struct Extracted {
    pub(crate) body: String,
    file: File,
    spans: Vec<Span>,
}

impl Extracted {
    pub(crate) fn select(mut self, marker: Option<&str>, attachment_id: u64) -> Result<Take<File>> {
        let index = marker
            .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
            .and_then(|v| String::from_utf8(v).ok())
            .and_then(|s| {
                s.strip_prefix("bzr-payload-")
                    .and_then(|n| n.parse::<usize>().ok())
            })
            .filter(|&i| i < self.spans.len())
            .ok_or_else(|| invalid("attachment has no streamed data"))?;
        let span = &self.spans[index];
        if let Some(error) = &span.error {
            return Err(invalid(&format!(
                "failed to decode attachment #{attachment_id}: {error}"
            )));
        }
        self.file
            .seek(SeekFrom::Start(span.offset))
            .map_err(staging_io)?;
        Ok(self.file.take(span.length))
    }
}

pub(crate) async fn extract(response: reqwest::Response, protocol: Protocol) -> Result<Extracted> {
    extract_with_limit(response, protocol, crate::http::MAX_RESPONSE_BODY_BYTES).await
}

async fn extract_with_limit(
    response: reqwest::Response,
    protocol: Protocol,
    limit: u64,
) -> Result<Extracted> {
    let xml = matches!(protocol, Protocol::Xml);
    extract_source(Source::Http(response), protocol, limit)
        .await
        .map_err(|error| match error {
            BzrError::DataIntegrity(message) if xml => BzrError::XmlRpc(message),
            BzrError::DataIntegrity(message) => BzrError::Deserialize(message),
            other => other,
        })
}

enum Source {
    Http(reqwest::Response),
    #[cfg(test)]
    Chunks(std::collections::VecDeque<Vec<u8>>),
}

async fn extract_source(response: Source, protocol: Protocol, limit: u64) -> Result<Extracted> {
    let file = tempfile::tempfile().map_err(staging_io)?;
    let mut reader = Reader {
        response,
        chunk: Vec::new(),
        position: 0,
        body: Vec::new(),
        limit,
        output: BufWriter::new(file),
        spans: Vec::new(),
        written: 0,
    };
    match protocol {
        Protocol::Json => reader.json_value(0).await?,
        Protocol::Xml => reader.xml_document().await?,
    }
    while let Some(byte) = reader.take().await? {
        if !byte.is_ascii_whitespace() {
            return Err(invalid("trailing content after attachment response"));
        }
    }
    reader.output.flush().map_err(staging_io)?;
    let file = reader
        .output
        .into_inner()
        .map_err(|e| staging_io(e.into_error()))?;
    let body = String::from_utf8(reader.body)
        .map_err(|_| invalid("attachment response is not valid UTF-8"))?;
    Ok(Extracted {
        body,
        file,
        spans: reader.spans,
    })
}

struct Reader {
    response: Source,
    chunk: Vec<u8>,
    position: usize,
    body: Vec<u8>,
    limit: u64,
    output: BufWriter<File>,
    spans: Vec<Span>,
    written: u64,
}

impl Reader {
    async fn peek(&mut self) -> Result<Option<u8>> {
        while self.position == self.chunk.len() {
            let chunk = match &mut self.response {
                Source::Http(response) => response.chunk().await?.map(|chunk| chunk.to_vec()),
                #[cfg(test)]
                Source::Chunks(chunks) => chunks.pop_front(),
            };
            let Some(chunk) = chunk else {
                return Ok(None);
            };
            self.chunk = chunk;
            self.position = 0;
        }
        Ok(self.chunk.get(self.position).copied())
    }

    async fn raw(&mut self) -> Result<Option<u8>> {
        let byte = self.peek().await?;
        self.position += usize::from(byte.is_some());
        Ok(byte)
    }

    fn retain(&mut self, bytes: &[u8]) -> Result<()> {
        if self.body.len() as u64 + bytes.len() as u64 > self.limit {
            return Err(BzrError::ResponseTooLarge {
                operation: "attachment response metadata".into(),
                limit_bytes: self.limit,
                status: None,
            });
        }
        self.body.extend_from_slice(bytes);
        Ok(())
    }

    async fn take(&mut self) -> Result<Option<u8>> {
        let byte = self.raw().await?;
        if let Some(b) = byte {
            self.retain(&[b])?;
        }
        Ok(byte)
    }

    async fn required(&mut self, retain: bool) -> Result<u8> {
        let byte = if retain {
            self.take().await?
        } else {
            self.raw().await?
        };
        byte.ok_or_else(|| invalid("truncated attachment response"))
    }

    fn finish_payload(&mut self, mut decoder: Decoder, offset: u64) -> Result<()> {
        if decoder.length != 0 && decoder.error.is_none() {
            decoder.error = Some("incomplete base64 quartet".into());
        }
        let marker = base64::engine::general_purpose::STANDARD
            .encode(format!("bzr-payload-{}", self.spans.len()));
        self.retain(marker.as_bytes())?;
        self.spans.push(Span {
            offset,
            length: self.written - offset,
            error: decoder.error,
        });
        Ok(())
    }
}

#[derive(Default)]
struct Decoder {
    quartet: [u8; 4],
    length: usize,
    padded: bool,
    error: Option<String>,
}

impl Decoder {
    fn push(&mut self, byte: u8, reader: &mut Reader) -> Result<()> {
        if self.error.is_some() {
            return Ok(());
        }
        if self.padded {
            self.error = Some("data after base64 padding".into());
            return Ok(());
        }
        self.quartet[self.length] = byte;
        self.length += 1;
        if self.length == 4 {
            let mut decoded = [0; 3];
            match base64::engine::general_purpose::STANDARD.decode_slice(self.quartet, &mut decoded)
            {
                Ok(length) => {
                    reader
                        .output
                        .write_all(&decoded[..length])
                        .map_err(staging_io)?;
                    reader.written += length as u64;
                    self.padded = self.quartet.contains(&b'=');
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            self.length = 0;
        }
        Ok(())
    }
}

fn invalid(message: &str) -> BzrError {
    BzrError::DataIntegrity(message.into())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "map_err transfers ownership of I/O failures"
)]
fn staging_io(error: std::io::Error) -> BzrError {
    io_with_context(
        "failed to stage attachment in temporary storage (check free disk space)",
        &error,
    )
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
