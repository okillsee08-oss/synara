pub trait Transcriber: Send + Sync {
    fn transcribe(&self, audio: &[u8]) -> anyhow::Result<String>;
}
pub trait Synthesizer: Send + Sync {
    fn synthesize(&self, text: &str) -> anyhow::Result<Vec<u8>>;
}
