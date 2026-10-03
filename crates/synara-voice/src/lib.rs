pub trait Transcriber: Send + Sync {
    fn transcribe(&self, audio: &[u8]) -> anyhow::Result<String>;
}
pub trait Synthesizer: Send + Sync {
    fn synthesize(&self, text: &str) -> anyhow::Result<Vec<u8>>;
}

pub fn validate_text(text: &str) -> anyhow::Result<()> {
    if text.trim().is_empty() {
        anyhow::bail!("voice text must not be empty");
    }
    Ok(())
}

pub fn validate_audio(audio: &[u8]) -> anyhow::Result<()> {
    if audio.is_empty() {
        anyhow::bail!("audio payload must not be empty");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_empty_inputs() {
        assert!(validate_text(" ").is_err());
        assert!(validate_audio(&[]).is_err());
        assert!(validate_text("hello").is_ok());
        assert!(validate_audio(&[1]).is_ok());
    }
}
