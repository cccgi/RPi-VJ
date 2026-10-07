use crate::{control::State, ipc::Message};
use anyhow::Result;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
pub struct Link {
    path: Option<PathBuf>,
    pub session: u64,
    revision: u64,
    retry: Instant,
    #[cfg(unix)]
    peer: Option<crate::ipc::Connection>,
}
impl Link {
    pub fn new(path: Option<PathBuf>) -> Result<Self> {
        #[cfg(not(unix))]
        anyhow::ensure!(path.is_none(), "production controller IPC requires Unix");
        Ok(Self {
            path,
            session: 0,
            revision: 0,
            retry: Instant::now() - Duration::from_secs(2),
            #[cfg(unix)]
            peer: None,
        })
    }
    pub fn external(&self) -> bool {
        self.path.is_some()
    }
    pub fn poll(&mut self, state: &mut State, show: &crate::show::Show) -> bool {
        #[cfg(unix)]
        {
            if self.peer.is_none() && self.retry.elapsed() > Duration::from_secs(1) {
                if let Some(path) = &self.path {
                    self.retry = Instant::now();
                    self.peer = crate::ipc::Connection::connect(path).ok();
                }
            }
            let mut new_session = false;
            if let Some(peer) = &mut self.peer {
                match peer.poll() {
                    Ok(messages) => {
                        for message in messages {
                            if let Message::Snapshot {
                                session,
                                revision,
                                state: next,
                            } = message
                            {
                                if !next.validate_for(show) {
                                    eprintln!("incompatible controller state rejected");
                                    continue;
                                }
                                if self.session != session || revision >= self.revision {
                                    new_session |= self.session != session;
                                    self.session = session;
                                    self.revision = revision;
                                    *state = next;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!("controller link lost; retaining output: {error:#}");
                        self.peer = None;
                    }
                }
            }
            new_session
        }
        #[cfg(not(unix))]
        {
            let _ = (state, show, &self.revision, &self.retry);
            false
        }
    }
    pub fn report(&mut self, message: &Message) {
        #[cfg(unix)]
        if let Some(peer) = &mut self.peer {
            if let Err(error) = peer.queue(message) {
                eprintln!("report rejected: {error:#}");
            }
        }
        #[cfg(not(unix))]
        let _ = message;
    }
}
