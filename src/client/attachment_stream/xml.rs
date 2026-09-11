use quick_xml::events::Event;

use crate::error::Result;

use super::{invalid, Decoder, Reader};

impl Reader {
    pub(super) async fn xml_document(&mut self) -> Result<()> {
        let mut state = XmlState::default();
        while let Some(byte) = self.peek().await? {
            if byte != b'<' {
                if state.in_data_value() && !byte.is_ascii_whitespace() {
                    self.xml_payload("value", false).await?;
                    state.stack.pop();
                    continue;
                }
                if state.stack.is_empty() && !byte.is_ascii_whitespace() {
                    return Err(invalid("content before attachment XML root"));
                }
                self.take().await?;
                continue;
            }
            let token = self.xml_markup(true).await?;
            if token == b"<![CDATA[" {
                if state.stack.is_empty() {
                    return Err(invalid("CDATA outside attachment XML root"));
                }
                if state.in_data_value() {
                    self.body.truncate(self.body.len() - token.len());
                    self.xml_payload("value", true).await?;
                    state.stack.pop();
                } else {
                    self.xml_cdata(None).await?;
                }
                continue;
            }
            let mut parser = quick_xml::Reader::from_reader(token.as_slice());
            // Tokens are parsed separately; xml_document checks the complete tag stack.
            parser.config_mut().allow_unmatched_ends = true;
            let event = parser
                .read_event()
                .map_err(|_| invalid("invalid attachment XML markup"))?;
            match event {
                Event::Start(tag) => {
                    let name: String = tag.name().as_ref().to_owned();
                    if state.stack.is_empty() {
                        if state.root_seen || name != "methodResponse" {
                            return Err(invalid("invalid XML-RPC root"));
                        }
                        state.root_seen = true;
                    }
                    if state.stack.len() >= 128 {
                        return Err(invalid("attachment XML nesting exceeds 128 levels"));
                    }
                    if state.in_data_value() && matches!(name.as_str(), "base64" | "string") {
                        self.xml_payload(&name, false).await?;
                        continue;
                    }
                    if name == "value" && state.members.last() == Some(&None) {
                        return Err(invalid("XML-RPC member value precedes its name"));
                    }
                    if name == "struct" {
                        state.struct_keys.push(std::collections::HashSet::new());
                    }
                    if name == "member" {
                        state.members.push(None);
                    }
                    if name == "name" {
                        state.name_start = Some(self.body.len());
                    }
                    state.stack.push(name.clone());
                }
                Event::End(tag) => {
                    let name: String = tag.name().as_ref().to_owned();
                    state.end(&name, &self.body)?;
                }
                Event::Empty(tag) => {
                    if state.stack.is_empty() {
                        return Err(invalid("empty element outside attachment XML root"));
                    }
                    let name = tag.name().as_ref().to_owned();
                    if state.members.last() == Some(&Some(true))
                        && ((state.stack.last().is_some_and(|s| s == "value")
                            && matches!(name.as_str(), "base64" | "string"))
                            || (name == "value"
                                && state.stack.last().is_some_and(|s| s == "member")))
                    {
                        self.body.truncate(self.body.len() - token.len());
                        self.retain(format!("<{name}>").as_bytes())?;
                        self.finish_payload(Decoder::default(), self.written)?;
                        self.retain(format!("</{name}>").as_bytes())?;
                    }
                }
                Event::DocType(_) => return Err(invalid("DTD is not supported in attachment XML")),
                Event::Comment(_) | Event::Decl(_) | Event::PI(_) => {}
                _ => return Err(invalid("unexpected attachment XML markup")),
            }
        }
        if state.root_seen && state.stack.is_empty() {
            Ok(())
        } else {
            Err(invalid("truncated attachment XML response"))
        }
    }

    async fn xml_markup(&mut self, retain: bool) -> Result<Vec<u8>> {
        let mut token = vec![self.required(retain).await?];
        let mut quote = None;
        loop {
            let byte = self.required(retain).await?;
            token.push(byte);
            if token.len() as u64 > self.limit {
                return Err(invalid("attachment XML markup exceeds metadata limit"));
            }
            if token == b"<![CDATA[" {
                return Ok(token);
            }
            if token.starts_with(b"<!--") {
                if token.ends_with(b"-->") {
                    return Ok(token);
                }
            } else if token.starts_with(b"<?") {
                if token.ends_with(b"?>") {
                    return Ok(token);
                }
            } else if quote == Some(byte) {
                quote = None;
            } else if quote.is_none() && matches!(byte, b'\'' | b'"') {
                quote = Some(byte);
            } else if quote.is_none() && byte == b'>' {
                return Ok(token);
            }
        }
    }

    async fn xml_payload(&mut self, closing_name: &str, starts_cdata: bool) -> Result<()> {
        let offset = self.written;
        let mut decoder = Decoder::default();
        if starts_cdata {
            self.xml_cdata(Some(&mut decoder)).await?;
        }
        loop {
            match self.peek().await? {
                None => return Err(invalid("truncated XML attachment data")),
                Some(b'<') => {
                    let token = self.xml_markup(false).await?;
                    if token == b"<![CDATA[" {
                        self.xml_cdata(Some(&mut decoder)).await?;
                        continue;
                    }
                    let mut parser = quick_xml::Reader::from_reader(token.as_slice());
                    // Tokens are parsed separately; xml_document checks the complete tag stack.
                    parser.config_mut().allow_unmatched_ends = true;
                    match parser
                        .read_event()
                        .map_err(|_| invalid("invalid XML attachment data markup"))?
                    {
                        Event::End(tag) if tag.name().as_ref() == closing_name => {
                            self.finish_payload(decoder, offset)?;
                            self.retain(&token)?;
                            return Ok(());
                        }
                        Event::Comment(_) => self.retain(&token)?,
                        _ => return Err(invalid("unexpected XML element inside attachment data")),
                    }
                }
                Some(b'&') => {
                    let mut entity = vec![self.required(false).await?];
                    while !entity.ends_with(b";") {
                        if entity.len() >= 32 {
                            return Err(invalid("oversized XML attachment entity"));
                        }
                        entity.push(self.required(false).await?);
                    }
                    let text = std::str::from_utf8(&entity)
                        .map_err(|_| invalid("invalid XML attachment entity"))?;
                    let value = quick_xml::escape::unescape(text)
                        .map_err(|_| invalid("invalid XML attachment entity"))?;
                    for byte in value.bytes() {
                        if !byte.is_ascii_whitespace() {
                            decoder.push(byte, self)?;
                        }
                    }
                }
                Some(_) => {
                    let byte = self.required(false).await?;
                    if !byte.is_ascii_whitespace() {
                        decoder.push(byte, self)?;
                    }
                }
            }
        }
    }

    async fn xml_cdata(&mut self, mut decoder: Option<&mut Decoder>) -> Result<()> {
        let mut pending = Vec::with_capacity(4);
        loop {
            let byte = self.required(decoder.is_none()).await?;
            pending.push(byte);
            if pending.ends_with(b"]]>") {
                if let Some(decoder) = decoder.as_deref_mut() {
                    for &byte in &pending[..pending.len() - 3] {
                        if !byte.is_ascii_whitespace() {
                            decoder.push(byte, self)?;
                        }
                    }
                }
                return Ok(());
            }
            if pending.len() > 3 {
                let byte = pending.remove(0);
                if let Some(decoder) = decoder.as_deref_mut() {
                    if !byte.is_ascii_whitespace() {
                        decoder.push(byte, self)?;
                    }
                }
            }
        }
    }
}

#[derive(Default)]
struct XmlState {
    stack: Vec<String>,
    // None until the member name is validated, then whether it names payload data.
    members: Vec<Option<bool>>,
    struct_keys: Vec<std::collections::HashSet<String>>,
    name_start: Option<usize>,
    root_seen: bool,
}

impl XmlState {
    fn in_data_value(&self) -> bool {
        self.members.last() == Some(&Some(true)) && self.stack.last().is_some_and(|s| s == "value")
    }

    fn end(&mut self, name: &str, body: &[u8]) -> Result<()> {
        if self.stack.pop().as_deref() != Some(name) {
            return Err(invalid("mismatched attachment XML tag"));
        }
        if name == "name" {
            let start = self
                .name_start
                .take()
                .ok_or_else(|| invalid("invalid XML-RPC member name"))?;
            let field = xml_text(&body[start..])?;
            if let Some(keys) = self.struct_keys.last_mut() {
                if !keys.insert(field.clone()) {
                    return Err(invalid("duplicate XML-RPC member"));
                }
            }
            if let Some(member) = self.members.last_mut() {
                if member.is_some() {
                    return Err(invalid("duplicate XML-RPC member name"));
                }
                *member = Some(field == "data");
            }
        }
        if name == "struct" {
            self.struct_keys.pop();
        }
        if name == "member" {
            self.members.pop();
        }
        Ok(())
    }
}

fn xml_text(fragment: &[u8]) -> Result<String> {
    let mut parser = quick_xml::Reader::from_reader(fragment);
    parser.config_mut().allow_unmatched_ends = true;
    crate::xmlrpc::protocol::parsing::read_text_content(&mut parser, "name", None)
}
