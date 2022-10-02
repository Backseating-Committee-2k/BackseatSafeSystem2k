mod segmented_reader;
mod tcp_protocol;

use std::{
    collections::{HashSet, VecDeque},
    thread,
    time::Duration,
};

use crossbeam_channel::{bounded, select, tick, Receiver, Sender, TryRecvError};
use crossbeam_utils::sync::WaitGroup;

use crate::{debugger::tcp_protocol::TcpHandler, Address};

use self::tcp_protocol::PollReturn;

const CHANNEL_BOUND: usize = 100;
const TCP_POLL_INTERVAL: Duration = Duration::from_millis(50);

struct Debugger {
    receiver: Receiver<DebugMessage>,
    breakpoint_sender: Sender<BreakpointMessage>,
    started: bool,
    start_notifications: Vec<WaitGroup>,
}

#[derive(Clone)]
pub struct DebugHandle {
    sender: Option<Sender<DebugMessage>>,
}

pub struct BreakpointHandle {
    state: BreakpointHandleState,
    breakpoints: HashSet<Address>,
    sender: Option<Sender<DebugMessage>>,
    receiver: Option<Receiver<BreakpointMessage>>,
    receive_cache: VecDeque<BreakpointMessage>,
}

#[derive(Debug, PartialEq)]
enum BreakpointHandleState {
    WaitingForStart,
    Running,
    Breaking,
}

enum DebugMessage {
    /// Request to stop the debugger thread.
    Stop,
    /// Request to wait for start and drop the sent wait group after starting.
    WaitForStart(WaitGroup),
    /// Notification that we hit a breakpoint and will start breaking.
    HitBreakpoint(Address),
    /// Notification that the debugger is (still) breaking at the given instruction address.
    Breaking(Address),
}

enum DebuggerCommand {
    TcpPoll,
    HandleMessage(DebugMessage),
}

enum BreakpointMessage {
    SetBreakpoints(Vec<Address>),
    RemoveBreakpoints(Vec<Address>),
    /// Continue normal execution i.e. stop breaking.
    Continue,
    /// Execute one instruction while breaking.
    StepOne,
}

pub fn start_debugger() -> (DebugHandle, BreakpointHandle) {
    let (sender, receiver) = bounded(CHANNEL_BOUND);
    let (breakpoint_sender, breakpoint_receiver) = bounded(CHANNEL_BOUND);

    thread::spawn(move || Debugger::new(receiver, breakpoint_sender).run());

    (
        DebugHandle {
            sender: Some(sender.clone()),
        },
        BreakpointHandle {
            state: BreakpointHandleState::WaitingForStart,
            breakpoints: HashSet::new(),
            sender: Some(sender),
            receiver: Some(breakpoint_receiver),
            receive_cache: VecDeque::new(),
        },
    )
}

impl DebugHandle {
    pub fn dummy() -> Self {
        Self { sender: None }
    }

    #[inline]
    fn send(&self, message: DebugMessage) {
        if let Some(sender) = &self.sender {
            sender
                .send(message)
                .expect("Cannot send message to debug interface.");
        }
    }

    pub fn stop(&self) {
        self.send(DebugMessage::Stop);
    }
}

impl BreakpointHandle {
    pub fn dummy() -> Self {
        Self {
            state: BreakpointHandleState::Running,
            breakpoints: HashSet::with_capacity(0),
            sender: None,
            receiver: None,
            receive_cache: VecDeque::with_capacity(0),
        }
    }

    pub fn before_instruction_execution(&mut self, instruction_pointer: Address) {
        use BreakpointHandleState::*;

        if self.state == WaitingForStart {
            self.wait_for_start();
            self.state = Running;
            self.receive_cache.clear();
        }

        self.receive_updates_non_blocking();

        let should_break = self.breakpoints.contains(&instruction_pointer);
        if self.state != Breaking && should_break {
            self.state = Breaking;
            self.receive_cache.clear();
            self.send(DebugMessage::HitBreakpoint(instruction_pointer));
        } else if self.state == Breaking {
            self.send(DebugMessage::Breaking(instruction_pointer));
        }

        if self.state == Breaking {
            self.breaking();
        }
    }

    /// Wait for start command from debugger interface
    /// or directly continue if not in debug mode.
    pub fn wait_for_start(&self) {
        if self.sender.is_some() {
            let wait_group = WaitGroup::new();
            self.send(DebugMessage::WaitForStart(wait_group.clone()));
            wait_group.wait();
        }
    }

    fn breaking(&mut self) {
        use BreakpointMessage::*;

        loop {
            while let Some(message) = self.receive_cache.pop_front() {
                match message {
                    StepOne => return,
                    Continue => {
                        self.state = BreakpointHandleState::Running;
                        return;
                    },
                    SetBreakpoints(_) | RemoveBreakpoints(_) => panic!("BreakpointHandle: SetBreakpoints and RemoveBreakpoints should never be added to the message cache but handled immediately."),
                }
            }

            self.receive_update_blocking();
        }
    }

    #[inline]
    fn send(&self, message: DebugMessage) {
        if let Some(sender) = &self.sender {
            sender
                .send(message)
                .expect("Cannot send message to debug interface.");
        }
    }

    fn receive_updates_non_blocking(&mut self) {
        loop {
            if let Some(ref receiver) = self.receiver {
                match receiver.try_recv() {
                    Ok(message) => self.handle_message(message),
                    Err(TryRecvError::Disconnected) => {
                        panic!("Cannot receive breakpoint updates after debugger has been stopped.")
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
        }
    }

    fn receive_update_blocking(&mut self) {
        if let Some(ref receiver) = self.receiver {
            match receiver.recv() {
                Ok(message) => self.handle_message(message),
                Err(_) => {
                    panic!("Cannot receive breakpoint updates after debugger has been stopped.")
                }
            }
        }
    }

    #[inline]
    fn handle_message(&mut self, message: BreakpointMessage) {
        match message {
            BreakpointMessage::SetBreakpoints(locations) => {
                self.breakpoints.extend(locations);
            }
            BreakpointMessage::RemoveBreakpoints(locations) => {
                for location in locations {
                    self.breakpoints.remove(&location);
                }
            }
            _ => self.receive_cache.push_back(message),
        }
    }
}

impl Debugger {
    fn new(receiver: Receiver<DebugMessage>, breakpoint_sender: Sender<BreakpointMessage>) -> Self {
        Self {
            receiver,
            breakpoint_sender,
            started: false,
            start_notifications: Vec::new(),
        }
    }

    fn run(mut self) {
        use DebuggerCommand::*;

        let mut tcp = TcpHandler::start();
        let tcp_poll = tick(TCP_POLL_INTERVAL);

        loop {
            let command = select! {
                recv(tcp_poll) -> _ => TcpPoll,
                recv(self.receiver) -> message =>
                    HandleMessage(message.expect("Debugger cannot receive message on debug interface.")),
            };

            match command {
                TcpPoll => self.handle_poll_result(tcp.poll()),
                HandleMessage(DebugMessage::Stop) => break,
                HandleMessage(message) => self.handle_debug_message(message, &mut tcp),
            }
        }
    }

    fn handle_poll_result(&mut self, result: tcp_protocol::Result<PollReturn>) {
        match result {
            Ok(
                PollReturn::Nothing | PollReturn::ClientConnected | PollReturn::ClientDisconnected,
            ) => {}
            Ok(PollReturn::ReceivedRequests(requests)) => {
                for request in requests {
                    self.handle_request(request);
                }
            }
            Err(_) => self.handle_tcp_result(result),
        }
    }

    fn handle_debug_message(&mut self, message: DebugMessage, tcp: &mut TcpHandler) {
        match message {
            DebugMessage::Stop => unreachable!(),
            DebugMessage::WaitForStart(wait_group) => {
                if !self.started {
                    self.start_notifications.push(wait_group);
                }
            }
            DebugMessage::HitBreakpoint(location) => {
                let message = tcp_protocol::Response::HitBreakpoint { location };
                self.handle_tcp_result(tcp.send(&message));
            }
            DebugMessage::Breaking(location) => {
                let message = tcp_protocol::Response::Breaking { location };
                self.handle_tcp_result(tcp.send(&message));
            }
        }
    }

    fn handle_tcp_result<T>(&self, result: tcp_protocol::Result<T>) {
        match result {
            Ok(_) => {}
            Err(tcp_protocol::Error::Io(ref error)) => eprintln!("Failed TCP operation: {}", error),
            Err(tcp_protocol::Error::Serde(ref error)) => {
                eprintln!("Failed (de)serialisation in TCP interface: {}", error)
            }
        }
    }

    fn handle_request(&mut self, request: tcp_protocol::Request) {
        match request {
            tcp_protocol::Request::StartExecution {} => {
                self.started = true;
                self.start_notifications.clear(); // ==> notify all
            }
            tcp_protocol::Request::SetBreakpoints { locations } => {
                self.send_to_breakpoint_handler(BreakpointMessage::SetBreakpoints(locations))
            }
            tcp_protocol::Request::RemoveBreakpoints { locations } => {
                self.send_to_breakpoint_handler(BreakpointMessage::RemoveBreakpoints(locations))
            }
            tcp_protocol::Request::Continue {} => {
                self.send_to_breakpoint_handler(BreakpointMessage::Continue)
            }
            tcp_protocol::Request::StepOne {} => {
                self.send_to_breakpoint_handler(BreakpointMessage::StepOne)
            }
        }
    }

    fn send_to_breakpoint_handler(&mut self, message: BreakpointMessage) {
        match self.breakpoint_sender.try_send(message) {
            Ok(_) | Err(crossbeam_channel::TrySendError::Full(_)) => {}
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                panic!("Breakpoint channel closed before debugger was stopped.")
            }
        }
    }
}
