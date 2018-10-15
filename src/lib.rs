use std::sync::Arc;

use qni_core_rs::prelude::*;
use ws::util::Token;

pub struct WebSocketServer {
    out: ws::Sender,
    ctx: ConnectorContext,
}

impl WebSocketServer {
    pub fn new(out: ws::Sender, hub: Arc<Hub>) -> Self {
        let console_ctx = hub.start_new_program();

        Self {
            out,
            ctx: ConnectorContext::new(hub, console_ctx),
        }
    }
}

const TOKEN_CHECK_SEND: Token = Token(1);

impl ws::Handler for WebSocketServer {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        self.out.timeout(50, TOKEN_CHECK_SEND)
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
            TOKEN_CHECK_SEND => {
                match self.ctx.try_get_msg() {
                    Some(msg) => self.out.send(msg)?,
                    None => {}
                }

                self.out.timeout(50, TOKEN_CHECK_SEND)
            }
            _ => Ok(()),
        }
    }
}

use simple_error::{SimpleError, SimpleResult};
use std::thread;
use std::time::Duration;

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
