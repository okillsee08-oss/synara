use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::broadcast;

pub struct Terminal {
    child: Box<dyn portable_pty::Child + Send>,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
    output: broadcast::Sender<Vec<u8>>,
}

impl Terminal {
    pub fn spawn(shell: &str, cwd: Option<&Path>, cols: u16, rows: u16) -> Result<Self> {
        let pty = native_pty_system();
        let pair = pty.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut command = CommandBuilder::new(shell);
        if let Some(cwd) = cwd {
            command.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn terminal shell")?;
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let (output, _) = broadcast::channel(1024);

        Ok(Self {
            child,
            master: Arc::new(Mutex::new(pair.master)),
            writer: Arc::new(Mutex::new(writer)),
            reader: Arc::new(Mutex::new(reader)),
            output,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.output.subscribe()
    }

    pub fn start_output_stream(&self) -> Result<thread::JoinHandle<()>> {
        let reader = Arc::clone(&self.reader);
        let output = self.output.clone();
        Ok(thread::Builder::new()
            .name("synara-pty-reader".into())
            .spawn(move || {
                let mut buffer = vec![0u8; 16 * 1024];
                loop {
                    let read_result = {
                        let mut guard = match reader.lock() {
                            Ok(guard) => guard,
                            Err(_) => break,
                        };
                        guard.read(&mut buffer)
                    };

                    match read_result {
                        Ok(0) => break,
                        Ok(size) => {
                            let _ = output.send(buffer[..size].to_vec());
                        }
                        Err(_) => break,
                    }
                }
            })?)
    }

    pub fn write(&self, input: &[u8]) -> Result<()> {
        self.writer
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal writer poisoned"))?
            .write_all(input)?;
        Ok(())
    }

    pub fn read_available(&self, buffer: &mut [u8]) -> Result<usize> {
        Ok(self
            .reader
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal reader poisoned"))?
            .read(buffer)?)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal master poisoned"))?
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        Ok(())
    }

    pub fn kill(&mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_has_output_subscription() {
        let result = Terminal::spawn("/bin/sh", None, 80, 24);
        assert!(result.is_ok());
        let terminal = result.unwrap();
        let _receiver = terminal.subscribe();
    }
}
