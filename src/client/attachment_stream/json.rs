use std::collections::HashSet;

use crate::error::Result;

use super::{invalid, Decoder, Reader};

impl Reader {
    async fn json_whitespace(&mut self) -> Result<()> {
        while self.peek().await?.is_some_and(|b| b.is_ascii_whitespace()) {
            self.take().await?;
        }
        Ok(())
    }

    pub(super) async fn json_value(&mut self, depth: usize) -> Result<()> {
        if depth >= 128 {
            return Err(invalid("attachment JSON nesting exceeds 128 levels"));
        }
        self.json_whitespace().await?;
        match self.peek().await? {
            Some(b'{') => self.json_object(depth).await,
            Some(b'[') => self.json_array(depth).await,
            Some(b'"') => self.json_string().await.map(|_| ()),
            Some(_) => {
                let start = self.body.len();
                while self
                    .peek()
                    .await?
                    .is_some_and(|b| !b.is_ascii_whitespace() && !b",]}".contains(&b))
                {
                    self.take().await?;
                }
                if self.body.len() == start {
                    return Err(invalid("expected attachment JSON value"));
                }
                Ok(())
            }
            None => Err(invalid("truncated attachment JSON")),
        }
    }

    async fn json_object(&mut self, depth: usize) -> Result<()> {
        self.take().await?;
        self.json_whitespace().await?;
        let mut keys = HashSet::new();
        if self.peek().await? == Some(b'}') {
            self.take().await?;
            return Ok(());
        }
        loop {
            self.json_whitespace().await?;
            let key = self.json_string().await?;
            if !keys.insert(key.clone()) {
                return Err(invalid("duplicate attachment JSON member"));
            }
            self.json_whitespace().await?;
            if self.required(true).await? != b':' {
                return Err(invalid("expected JSON member colon"));
            }
            self.json_whitespace().await?;
            if key == "data" && self.peek().await? == Some(b'"') {
                self.json_payload().await?;
            } else {
                Box::pin(self.json_value(depth + 1)).await?;
            }
            self.json_whitespace().await?;
            match self.required(true).await? {
                b'}' => return Ok(()),
                b',' => {}
                _ => return Err(invalid("expected attachment JSON object delimiter")),
            }
        }
    }

    async fn json_array(&mut self, depth: usize) -> Result<()> {
        self.take().await?;
        self.json_whitespace().await?;
        if self.peek().await? == Some(b']') {
            self.take().await?;
            return Ok(());
        }
        loop {
            Box::pin(self.json_value(depth + 1)).await?;
            self.json_whitespace().await?;
            match self.required(true).await? {
                b']' => return Ok(()),
                b',' => {}
                _ => return Err(invalid("expected attachment JSON array delimiter")),
            }
        }
    }

    async fn json_string(&mut self) -> Result<String> {
        let start = self.body.len();
        if self.required(true).await? != b'"' {
            return Err(invalid("expected JSON string"));
        }
        loop {
            match self.required(true).await? {
                b'"' => break,
                b'\\' => {
                    self.required(true).await?;
                }
                _ => {}
            }
        }
        serde_json::from_slice(&self.body[start..])
            .map_err(|_| invalid("invalid attachment JSON string"))
    }

    async fn json_payload(&mut self) -> Result<()> {
        self.take().await?;
        let offset = self.written;
        let mut decoder = Decoder::default();
        loop {
            match self.required(false).await? {
                b'"' => break,
                b'\\' => {
                    let escaped = self.required(false).await?;
                    let mut token = vec![b'"', b'\\', escaped];
                    if escaped == b'u' {
                        for _ in 0..4 {
                            token.push(self.required(false).await?);
                        }
                    }
                    token.push(b'"');
                    let decoded: String = serde_json::from_slice(&token)
                        .map_err(|_| invalid("invalid JSON escape in attachment data"))?;
                    for byte in decoded.bytes() {
                        decoder.push(byte, self)?;
                    }
                }
                byte if byte < 0x20 => {
                    return Err(invalid("unescaped control in attachment JSON data"))
                }
                byte => decoder.push(byte, self)?,
            }
        }
        self.finish_payload(decoder, offset)?;
        self.retain(b"\"")
    }
}
