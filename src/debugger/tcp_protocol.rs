use std::{
    fmt,
    io::{self, ErrorKind, Write},
    net::{TcpListener, TcpStream},
};

use crossbeam_utils::Backoff;
use serde::{Deserialize, Serialize};

use crate::Address;

use super::segmented_reader::{self, Segment, SegmentedReader};

const TCP_INTERFACE_ADDRESS: &str = "127.0.0.1:57017";
const DEBUGGER_PORT_PREFIX: &str = "Debugger-Port:";

#[derive(Debug, Deserialize)]
pub enum Request {
    StartExecution {},
    SetBreakpoints {
        locations: Vec<Address>,
    },
    RemoveBreakpoints {
        locations: Vec<Address>,
    },
    /// Continue normal execution i.e. stop breaking.
    Continue {},
    /// Execute one instruction while breaking.
    StepOne {},
}

#[derive(Debug, Serialize)]
pub enum Response {
    HitBreakpoint { location: Address },
    Breaking { location: Address },
}

pub struct TcpHandler {
    listener: TcpListener,
    client: Option<TcpStream>,
    reader: SegmentedReader,
}

pub enum PollReturn {
    Nothing,
    ClientConnected,
    ClientDisconnected,
    ReceivedRequests(Vec<Request>),
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Serde(serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl TcpHandler {
    pub fn start() -> Self {
        let listener =
            TcpListener::bind(TCP_INTERFACE_ADDRESS).expect("Cannot open debug TCP interface.");
        listener
            .set_nonblocking(true)
            .expect("Cannot set tcp listener to non-blocking.");

        if let Ok(address) = listener.local_addr() {
            println!("{}{}", DEBUGGER_PORT_PREFIX, address.port());
        }

        Self {
            listener,
            client: None,
            reader: SegmentedReader::new(),
        }
    }

    pub fn poll(&mut self) -> Result<PollReturn> {
        match self.client {
            None => self.tcp_accept(),
            Some(ref mut client) => match self.reader.read(client) {
                Ok(segments) => {
                    let result = self.parse_requests(&segments);
                    if result.is_err() {
                        self.disconnect();
                    }
                    result
                }
                Err(segmented_reader::Error::Disconnected) => {
                    self.disconnect();
                    Ok(PollReturn::ClientDisconnected)
                }
                Err(segmented_reader::Error::Io(error))
                    if error.kind() == io::ErrorKind::WouldBlock =>
                {
                    Ok(PollReturn::Nothing)
                }
                Err(segmented_reader::Error::Io(error)) => {
                    self.disconnect();
                    Err(Error::Io(error))
                }
            },
        }
    }

    pub fn send(&mut self, message: &Response) -> Result<()> {
        let mut json = serde_json::to_vec(message).map_err(Error::Serde)?;
        json.push(0);

        self.write_all(&json[..])
    }

    fn disconnect(&mut self) {
        self.client = None; // Dropping the stream disconnects it, if it is still active.
        self.reader.clear();
    }

    fn tcp_accept(&mut self) -> Result<PollReturn> {
        match self.listener.accept() {
            Ok((client, _)) => {
                client.set_nodelay(true).map_err(Error::Io)?;
                self.client = Some(client);
                Ok(PollReturn::ClientConnected)
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(PollReturn::Nothing),
            Err(error) => Err(Error::Io(error)),
        }
    }

    fn parse_requests(&mut self, segments: &Vec<Segment>) -> Result<PollReturn> {
        let mut requests = Vec::new();
        for segment in segments {
            let slice = self.reader.segment(segment);
            let request: Request = serde_json::from_slice(slice).map_err(Error::Serde)?;
            requests.push(request);
        }

        Ok(PollReturn::ReceivedRequests(requests))
    }

    /// Non-blocking version of write_all.
    fn write_all(&mut self, mut buffer: &[u8]) -> Result<()> {
        let client = match &mut self.client {
            Some(client) => client,
            None => return Ok(()),
        };

        let backoff = Backoff::new();
        while !buffer.is_empty() {
            match client.write(buffer) {
                Ok(0) => {
                    self.disconnect();
                    return Err(Error::Io(io::Error::new(
                        ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    )));
                }
                Ok(n) => buffer = &buffer[n..],
                Err(ref error) if error.kind() == ErrorKind::Interrupted => {}
                Err(ref error) if error.kind() == ErrorKind::WouldBlock => backoff.spin(),
                Err(error) => {
                    self.disconnect();
                    return Err(Error::Io(error));
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(error) => error.fmt(f),
            Error::Serde(error) => error.fmt(f),
        }
    }
}
