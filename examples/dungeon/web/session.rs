//! Explicit opt-in private-LAN session; clients never configure or execute models.
use super::*;
use std::{
    io::Read,
    net::Ipv4Addr,
    time::{Duration, Instant},
};
pub struct Session {
    pub address: Ipv4Addr,
    pub code: String,
    token: Option<String>,
    failures: u8,
    reset: Instant,
}
fn secret() -> Result<String> {
    let mut b = [0u8; 24];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .map_err(|_| Error::new("LAN requires an operating-system random source."))?;
    Ok(b.iter().map(|n| format!("{n:02x}")).collect())
}
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
impl Session {
    pub fn new(address: Ipv4Addr) -> Result<Self> {
        if !address.is_private() && !address.is_loopback() {
            return Err(Error::new("Choose the host's private LAN IPv4 address."));
        }
        Ok(Self { address, code: secret()?, token: None, failures: 0, reset: Instant::now() })
    }
    pub fn join(&mut self, code: &str) -> Result<String> {
        if self.reset.elapsed() > Duration::from_secs(60) {
            self.failures = 0;
            self.reset = Instant::now();
        }
        if self.failures >= 5 {
            return Err(Error::new("Too many join attempts; wait one minute."));
        }
        if !same(code, &self.code) {
            self.failures += 1;
            return Err(Error::new("Invalid session code."));
        }
        if self.token.is_some() {
            return Err(Error::new("Player 2 is already connected. Ask the host to reset the invitation."));
        }
        let token = secret()?;
        self.token = Some(token.clone());
        Ok(token)
    }
    pub fn authorized(&self, cookie: Option<&str>) -> bool {
        self.token.as_ref().is_some_and(|token| {
            cookie.is_some_and(|s| {
                s.split(';').filter_map(|p| p.trim().strip_prefix("lantern_guest=")).any(|v| same(v, token))
            })
        })
    }
    pub fn info(&self, port: u16) -> Value {
        gamectx::obj! {"enabled"=>true,"url"=>format!("http://{}:{port}/join",self.address),"code"=>self.code.clone(),"guest_connected"=>self.token.is_some(),"host_player"=>"player_1","guest_player"=>"player_2"}
    }
    pub fn rotate(&mut self) -> Result<()> {
        self.code = secret()?;
        self.token = None;
        self.failures = 0;
        Ok(())
    }
}
