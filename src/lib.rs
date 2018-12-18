use std::sync::Arc;

#[cfg(feature = "ssl")]
use openssl::pkey::PKey;
#[cfg(feature = "ssl")]
use openssl::ssl::{SslAcceptor, SslMethod, SslStream};
#[cfg(feature = "ssl")]
use openssl::x509::X509;
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
    pub fn new(out: ws::Sender, hub: Arc<Hub>) -> Self {
        let console_ctx = hub.start_new_program();

        Self {
            out,
            ctx: ConnectorContext::new(hub, console_ctx),
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
        match self.ctx.on_recv_message(&msg.into_data()) {
            Some(callback) => {
                self.out.send(callback)?;
            }
            None => {}
        }

        Ok(())
    }

    fn on_timeout(&mut self, event: Token) -> ws::Result<()> {
        match event {
            TOKEN_TICK => {
                if self.ctx.need_exit() {
                    self.out.close(ws::CloseCode::Normal)
                } else {
                    match self.ctx.try_get_msg() {
                        Some(msg) => self.out.send(msg)?,
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

use simple_error::{SimpleError, SimpleResult};
use std::thread;
use std::time::Duration;

#[cfg(feautre = "ssl")]
pub fn start_connector_ssl(hub: Arc<Hub>, host: String, cert: &[u8], pkey: &[u8]) -> SimpleResult<()> {

    let acceptor = Arc::new({
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        builder.set_private_key(PKey::private_key_from_pem(pkey).unwrap()).map_err(From::from)?;
        builder.set_certificate(X509::from_pem(cert).unwrap()).map_err(From::from)?;
        builder.build()
    });

    let hub_inner = hub.clone();

    let socket = ws::Builder::new()
        .with_settings(ws::Settings {
            tcp_nodelay: true,
            ..ws::Settings::default()
        })
        .build(move |out| WebSocketServer::new(out, hub_inner.clone()).with_ssl(acceptor.clone()))
        .map_err(SimpleError::from)?;

    let handle = socket.broadcaster();

    let t = thread::spawn(move || {
        let ret = socket.listen(&host);

        match ret {
            Ok(_) => Ok(()),
            Err(err) => {
                println!("err: {}", err);
                Err(SimpleError::from(err))
            }
        }
    });

    while !hub.need_exit() {
        thread::sleep(Duration::from_secs(1));
    }

    handle.shutdown().map_err(SimpleError::from)?;

    match t.join() {
        Ok(ret) => ret,
        Err(_) => Err(SimpleError::new("join err")),
    }
}

pub fn start_connector(hub: Arc<Hub>, host: String) -> SimpleResult<()> {
    let hub_inner = hub.clone();

    let socket = ws::Builder::new()
        .with_settings(ws::Settings {
            tcp_nodelay: true,
            ..ws::Settings::default()
        })
        .build(move |out| WebSocketServer::new(out, hub_inner.clone()))
        .map_err(SimpleError::from)?;

    let handle = socket.broadcaster();

    let t = thread::spawn(move || {
        let ret = socket.listen(&host);

        match ret {
            Ok(_) => Ok(()),
            Err(err) => {
                println!("err: {}", err);
                Err(SimpleError::from(err))
            }
        }
    });

    while !hub.need_exit() {
        thread::sleep(Duration::from_secs(1));
    }

    handle.shutdown().map_err(SimpleError::from)?;

    match t.join() {
        Ok(ret) => ret,
        Err(_) => Err(SimpleError::new("join err")),
    }
}

pub mod prelude {
    pub use crate::*;
    pub use qni_core_rs::prelude as core;
}
