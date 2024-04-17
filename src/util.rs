use tokio::io::{self, AsyncWriteExt};

pub(crate) struct Writer<T: AsyncWriteExt + Unpin> {
    writer: T,
}

impl<T: AsyncWriteExt + Unpin> Writer<T> {
    pub(crate) fn new(writer: T) -> Self {
        Self { writer }
    }

    pub(crate) async fn write_flush(&mut self, data: &[u8]) -> io::Result<()> {
        self.writer.write_all(data).await?;
        self.writer.flush().await?;
        Ok(())
    }

    pub(crate) async fn writeln_flush(&mut self, data: &[u8]) -> io::Result<()> {
        self.writer.write_all(data).await?;
        self.writer.write_u8(b'\n').await?;
        self.writer.flush().await?;
        Ok(())
    }
}
