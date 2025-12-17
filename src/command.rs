use bytes::{Buf, Bytes, BytesMut};
use std::io::Cursor;
use std::io::Write;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        val: Bytes,
        resp: Responder<()>,
    },
}


/// 管理任务可以使用该发送端将命令执行的结果传回给发出命令的任务
pub type Responder<T> = oneshot::Sender<Result<T>>;

use mini_redis::frame::Error;
use mini_redis::{Frame, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;

pub struct Connection {
    pub stream: BufWriter<TcpStream>,
    pub buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream: BufWriter::new(stream),
            buffer: BytesMut::with_capacity(4096),
        }
    }

    /// 从连接读取一个帧
    ///
    /// 如果遇到EOF，则返回 None
    pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                // 代码能执行到这里，说明了对端关闭了连接，
                // 需要看看缓冲区是否还有数据，若没有数据，说明所有数据成功被处理，
                // 若还有数据，说明对端在发送帧的过程中断开了连接，导致只发送了部分数据
                return if self.buffer.is_empty() {
                    Ok(None)
                } else {
                    Err("connection reset by peer".into())
                };
            }
        }
    }

    /// 将帧写入到连接中
    pub async fn write_frame(&mut self, frame: &Frame) -> Result<()> {
        match frame {
            Frame::Array(val) => {
                self.stream.write_u8(b'*').await?;
                self.write_decimal(val.len() as u64).await?;
                for x in val {
                    self.write_value(x).await?;
                }
            }
            _ => self.write_value(frame).await?,
        }

        self.stream.flush().await?;
        Ok(())
    }

    async fn write_value(&mut self, value: &Frame) -> Result<()> {
        match value {
            Frame::Simple(var) => {
                self.stream.write_u8(b'+').await?;
                self.stream.write_all(var.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Error(val) => {
                self.stream.write_u8(b'-').await?;
                self.stream.write_all(val.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Integer(val) => {
                self.stream.write_u8(b':').await?;
                self.write_decimal(*val).await?;
            }
            Frame::Null => {
                self.stream.write_all(b"$-1\r\n").await?;
            }
            Frame::Bulk(val) => {
                let len = val.len();
                self.stream.write_u8(b'$').await?;
                self.write_decimal(len as u64).await?;
                self.stream.write_all(val).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Array(_) => unimplemented!(),
        }
        Ok(())
    }

    fn parse_frame(&mut self) -> Result<Option<Frame>> {
        let mut buf = Cursor::new(&self.buffer[..]);

        match Frame::check(&mut buf) {
            Ok(_) => {
                let len = buf.position() as usize;
                buf.set_position(0);

                let frame = Frame::parse(&mut buf)?;

                self.buffer.advance(len);

                Ok(Some(frame))
            }
            Err(Error::Incomplete) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    async fn write_decimal(self: &mut Self, val: u64) -> Result<()> {
        let mut buf = [0u8; 20];
        let mut buf = Cursor::new(&mut buf[..]);

        write!(&mut buf, ":{}", val)?;

        let len = buf.position() as usize;
        self.stream.write_all(&buf.get_ref()[..len]).await?;
        self.stream.write_all(b"\r\n").await?;
        Ok(())
    }
}
