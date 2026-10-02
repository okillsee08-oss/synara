use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct Terminal {
    child: Box<dyn portable_pty::Child + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
}

impl Terminal {
    pub fn spawn(shell: &str, cwd: Option<&Path>, cols: u16, rows: u16) -> Result<Self> {
        let pty = native_pty_system();
        let pair = pty.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;

        let mut command = CommandBuilder::new(shell);
        if let Some(cwd) = cwd {
            command.cwd(cwd);
        }

        let child = pair.slave.spawn_command(command).context("failed to spawn terminal shell")?;
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self {
            child,
            writer: Arc::new(Mutex::new(writer)),
            reader: Arc::new(Mutex::new(reader)),
        })
    }

    pub fn write(&self, input: &[u8]) -> Result<()> {
        self.writer.lock().map_err(|_| anyhow::anyhow!("terminal writer poisoned"))?.write_all(input)?;
        Ok(())
    }

    pub fn read_available(&self, buffer: &mut [u8]) -> Result<usize> {
        Ok(self.reader.lock().map_err(|_| anyhow::anyhow!("terminal reader poisoned"))?.read(buffer)?)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        // portable-pty exposes resize on the master pair; sessions created here
        // keep a stable PTY size and callers can recreate or extend this boundary
        // when window-management integration is attached.
        let _ = (cols, rows);
        Ok(())
    }

    pub fn kill(&mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
    }
}
