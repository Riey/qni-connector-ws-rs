use log::debug;

use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[cfg(feature = "ssl")]
use openssl::ssl::{SslAcceptor, SslStream};
#[cfg(feature = "ssl")]
use ws::util::TcpStream;

use qni_core_rs::prelude::*;
use ws::util::Token;

pub struct WebSocketServer {
    out: ws::Sender,
    ctx: ConnectorContext,
    #[cfg(feature = "ssl")]
    ssl: Option<Arc<SslAcceptor>>,
}

impl WebSocketServer {
    pub fn new(out: ws::Sender, console_ctx: Arc<ConsoleContext>) -> Self {
        Self {
            out,
            ctx: ConnectorContext::new(console_ctx),
            #[cfg(feature = "ssl")]
            ssl: None,
        }
    }

    #[cfg(feature = "ssl")]
    pub fn with_ssl(mut self, ssl: Arc<SslAcceptor>) -> Self {
        self.ssl = Some(ssl);
        self
    }
}

const TOKEN_TICK: Token = Token(1);

impl ws::Handler for WebSocketServer {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        self.out.timeout(50, TOKEN_TICK)
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        if let Ok(msg) = protobuf::parse_from_bytes(&msg.into_data()) {
            match self.ctx.on_recv_message(msg) {
                Some(callback) => {
                    self.out
                        .send(protobuf::Message::write_to_bytes(&callback).expect("serialize"))?;
                }
                None => {}
            }
        }

        Ok(())
    }

    fn on_timeout(&mut self, event: Token) -> ws::Result<()> {
        match event {
            TOKEN_TICK => {
                if self.ctx.need_exit() {
                    self.out.shutdown()
                } else {
                    match self.ctx.try_get_msg() {
                        Some(msg) => self
                            .out
                            .send(protobuf::Message::write_to_bytes(&msg).expect("serialize"))?,
                        None => {}
                    }

                    self.out.timeout(50, TOKEN_TICK)
                }
            }
            _ => Ok(()),
        }
    }

    #[cfg(feature = "ssl")]
    fn upgrade_ssl_server(&mut self, sock: TcpStream) -> ws::Result<SslStream<TcpStream>> {
        self.ssl.as_ref().unwrap().accept(sock).map_err(From::from)
    }
}

#[cfg(feautre = "ssl")]
pub fn start_connector_ssl(
    ctx: &Arc<ConsoleContext>,
    host: &str,
    cert: &[u8],
    pkey: &[u8],
) -> ws::Result<()> {
    let acceptor = Arc::new({
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        builder
            .set_private_key(PKey::private_key_from_pem(pkey).unwrap())
            .map_err(From::from)?;
        builder
            .set_certificate(X509::from_pem(cert).unwrap())
            .map_err(From::from)?;
        builder.build()
    });

    let ctx_inner = ctx.clone();
    let socket = ws::Builder::new()
        .with_settings(ws::Settings {
            tcp_nodelay: true,
            ..ws::Settings::default()
        })
        .build(move |out| WebSocketServer::new(out, ctx_inner.clone()).with_ssl(acceptor.clone()))?;

    let sender = socket.broadcaster();

    let host_inner = Arc::new(host.to_string());
    let handle = thread::spawn(move || {
        socket
            .listen(&host_inner)
            .map(|_| ())
    });

    while !ctx.need_exit() {
        thread::sleep(Duration::from_secs(1));
    }

    debug!("Console exited shutdown ...");

    sender.shutdown()?;
    handle.join().map_err(|_| ws::Error::new(ws::ErrorKind::Internal, "join err"))??;

    Ok(())
}

pub fn start_connector(ctx: &Arc<ConsoleContext>, host: &str) -> ws::Result<()> {
    let ctx_inner = ctx.clone();
    let socket = ws::Builder::new()
        .with_settings(ws::Settings {
            tcp_nodelay: true,
            ..ws::Settings::default()
        })
        .build(move |out| WebSocketServer::new(out, ctx_inner.clone()))?;

    let sender = socket.broadcaster();

    let host_inner = Arc::new(host.to_string());
    let handle = thread::spawn(move || {
        socket
            .listen(host_inner.as_str())
            .map(|_| ())
    });

    while !ctx.need_exit() {
        thread::sleep(Duration::from_secs(1));
    }

    debug!("Console exited shutdown ...");

    sender.shutdown()?;
    handle.join().map_err(|_| ws::Error::new(ws::ErrorKind::Internal, "join err"))??;

    Ok(())
}

