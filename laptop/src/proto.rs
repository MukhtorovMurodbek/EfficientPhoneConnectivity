//! Wire protocol v1 — see PROTOCOL.md. Length-prefixed JSON frames over any byte stream.

use serde::{Deserialize, Serialize};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// RFCOMM service UUID the phone listens on.
pub const SERVICE_UUID: u128 = 0x540c3dea_5e95_4872_993a_c1e25e3b021d;
pub const VERSION: u32 = 1;
pub const MAX_FRAME: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Msg {
    Hello {
        v: u32,
        #[serde(default)]
        name: String,
    },
    Cfg {
        clipboard: bool,
        notifications: bool,
    },
    Clip {
        text: String,
    },
    Notif {
        key: String,
        #[serde(default)]
        app: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        text: String,
    },
    NotifRm {
        key: String,
    },
    /// Any message type this build doesn't know about. Ignored.
    #[serde(other)]
    Unknown,
}

pub async fn read_msg<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Msg> {
    let len = r.read_u32().await? as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("bad frame length {len}")));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub async fn write_msg<W: AsyncWrite + Unpin>(w: &mut W, msg: &Msg) -> io::Result<()> {
    let buf = serde_json::to_vec(msg).map_err(io::Error::other)?;
    if buf.len() > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "message too large"));
    }
    // One write per frame so RFCOMM doesn't send the header in its own packet.
    let mut frame = Vec::with_capacity(4 + buf.len());
    frame.extend_from_slice(&(buf.len() as u32).to_be_bytes());
    frame.extend_from_slice(&buf);
    w.write_all(&frame).await?;
    w.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip_all_types() {
        let (mut a, mut b) = tokio::io::duplex(4096);
        let msgs = vec![
            Msg::Hello { v: 1, name: "laptop".into() },
            Msg::Cfg { clipboard: true, notifications: false },
            Msg::Clip { text: "héllo 👋\nworld".into() },
            Msg::Notif { key: "k1".into(), app: "Signal".into(), title: "Bob".into(), text: "hi".into() },
            Msg::NotifRm { key: "k1".into() },
        ];
        for m in &msgs {
            write_msg(&mut a, m).await.unwrap();
        }
        for m in &msgs {
            assert_eq!(&read_msg(&mut b).await.unwrap(), m);
        }
    }

    #[tokio::test]
    async fn wire_format_matches_spec() {
        let (mut a, mut b) = tokio::io::duplex(4096);
        write_msg(&mut a, &Msg::Clip { text: "x".into() }).await.unwrap();
        let want = br#"{"t":"clip","text":"x"}"#;
        let mut raw = vec![0u8; 4 + want.len()];
        b.read_exact(&mut raw).await.unwrap();
        assert_eq!(&raw[..4], &(want.len() as u32).to_be_bytes());
        assert_eq!(&raw[4..], want);
    }

    #[tokio::test]
    async fn unknown_type_and_fields_are_tolerated() {
        let (mut a, mut b) = tokio::io::duplex(4096);
        for body in [&br#"{"t":"screen","w":1}"#[..], br#"{"t":"clip","text":"y","extra":5}"#] {
            a.write_u32(body.len() as u32).await.unwrap();
            a.write_all(body).await.unwrap();
        }
        assert_eq!(read_msg(&mut b).await.unwrap(), Msg::Unknown);
        assert_eq!(read_msg(&mut b).await.unwrap(), Msg::Clip { text: "y".into() });
    }

    #[tokio::test]
    async fn oversized_frame_rejected() {
        let (mut a, mut b) = tokio::io::duplex(64);
        a.write_u32((MAX_FRAME + 1) as u32).await.unwrap();
        assert!(read_msg(&mut b).await.is_err());
    }
}
