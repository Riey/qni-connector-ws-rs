use std::sync::Arc;


use mio::Token;

use std::sync::mpsc::TryRecvError;
use qni_core_rs::prelude::*;

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
const TOKEN_CHECK_EXIT: Token = Token(2);

impl ws::Handler for WebSocketServer {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        self.out.timeout(500, TOKEN_CHECK_EXIT)?;
        self.out.timeout(100, TOKEN_CHECK_SEND)
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        match self.ctx.recv_message(&msg.into_data()) {
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
                match self.ctx.try_recv_send_messge() {
                    Ok(msg) => {
                        self.out.send(msg)?;
                        self.out.timeout(100, TOKEN_CHECK_SEND)
                    }
                    Err(TryRecvError::Empty) => {
                        self.out.timeout(50, TOKEN_CHECK_SEND)
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.out.timeout(300, TOKEN_CHECK_SEND)
                    }
                }
            }
            TOKEN_CHECK_EXIT => {
                match self.ctx.need_exit() {
                    true => self.out.close(ws::CloseCode::Normal),
                    false => self.out.timeout(500, TOKEN_CHECK_EXIT)
                }
            }
            _ => Ok(())
        }
    }
}

pub fn start_connector(hub: SharedHubPtr, host: &str) -> ws::Result<()> {

    let hub = unsafe {
        hub.read()
    };

    ws::listen(&host, move |out| {
        WebSocketServer::new(out, hub.clone())
    })
}
