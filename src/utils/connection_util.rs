use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tokio_native_tls::TlsStream;
use tokio_native_tls::TlsConnector;

use crate::utils::fix_util::FixCodec;


pub async fn connect_fix_endpoint(
    hostname: &str, port: u16,
) -> anyhow::Result<Framed<TlsStream<TcpStream>, FixCodec>> {
    let port = if port == 0 { 9000 } else { port };
    let addr = format!("{}:{}", hostname, port);
    let tcp = TcpStream::connect(addr).await?;
    let connector = native_tls::TlsConnector::builder().build()?;
    let tls = TlsConnector::from(connector);
    let stream = tls.connect(hostname, tcp).await?;
    Ok(Framed::new(stream, FixCodec))
}
