mod segmented_reader;
mod tcp_protocol;

use std::{
    collections::{HashSet, VecDeque},
    thread,
    time::Duration,
};

use crossbeam_channel::{bounded, select, tick, Receiver, Sender, TryRecvError};
use crossbeam_utils::sync::WaitGroup;

use crate::{debugger::tcp_protocol::TcpHandler, processor::Processor, Address, Word};

use self::tcp_protocol::PollReturn;

const CHANNEL_BOUND: usize = 100;
const TCP_POLL_INTERVAL: Duration = Duration::from_millis(50);

struct Debugger {
    receiver: Receiver<DebugMessage>,
    breakpoint_sender: Sender<DebugCommand>,
    started: bool,
    start_notifications: Vec<WaitGroup>,
}

pub struct DebugHandle {
    state: BreakpointHandleState,
    breakpoints: HashSet<Address>,
    sender: Option<Sender<DebugMessage>>,
    receiver: Option<Receiver<DebugCommand>>,
    receive_cache: VecDeque<DebugCommand>,
    should_pause: bool,
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
    /// Notification that the debugger started breaking at the given instruction address due to a pause request.
    Pausing(Address),
    /// Notification that a register value changed. Also used to send initial register values of non-zero registers.
    Registers(Vec<Word>),
}

enum DebugCommand {
    SetBreakpoints(Vec<Address>),
    RemoveBreakpoints(Vec<Address>),
    /// Continue normal execution i.e. stop breaking.
    Continue,
    /// Execute one instruction while breaking.
    StepOne,
    /// Instructs breakpoint handler to break as soon as possible.
    Pause,
}

pub fn start_debugger() -> DebugHandle {
    let (sender, receiver) = bounded(CHANNEL_BOUND);
    let (breakpoint_sender, breakpoint_receiver) = bounded(CHANNEL_BOUND);

    thread::spawn(move || Debugger::new(receiver, breakpoint_sender).run());

    DebugHandle {
        state: BreakpointHandleState::WaitingForStart,
        breakpoints: HashSet::new(),
        sender: Some(sender),
        receiver: Some(breakpoint_receiver),
        receive_cache: VecDeque::new(),
        should_pause: false,
    }
}

impl DebugHandle {
    pub fn dummy() -> Self {
        Self {
            state: BreakpointHandleState::Running,
            breakpoints: HashSet::with_capacity(0),
            sender: None,
            receiver: None,
            receive_cache: VecDeque::with_capacity(0),
            should_pause: false,
        }
    }

    pub fn stop(&self) {
        self.send(DebugMessage::Stop);
    }

    pub fn before_instruction_execution(&mut self, processor: &Processor) {
        use BreakpointHandleState::*;

        let instruction_pointer = processor.get_instruction_pointer();

        if self.state == WaitingForStart {
            self.wait_for_start();
            self.state = Running;
            self.receive_cache.clear();
        }

        if self.state == Breaking {
            self.send_registers(&processor.registers);
            self.send(DebugMessage::Breaking(instruction_pointer));
        } else {
            self.start_breaking_if_requested(instruction_pointer, processor);
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

    fn start_breaking_if_requested(&mut self, instruction_pointer: Word, processor: &Processor) {
        use BreakpointHandleState::*;

        if self.state == Breaking {
            return;
        }

        self.receive_updates_non_blocking();

        let mut should_start_breaking = None;
        let hit_breakpoint = self.breakpoints.contains(&instruction_pointer);

        if self.should_pause {
            should_start_breaking = Some(DebugMessage::Pausing(instruction_pointer));
        } else if hit_breakpoint {
            should_start_breaking = Some(DebugMessage::HitBreakpoint(instruction_pointer));
        }

        self.should_pause = false;

        if let Some(break_message) = should_start_breaking {
            let registers = &processor.registers;
            self.state = Breaking;
            self.receive_cache.clear();
            self.send_registers(registers);
            self.send(break_message);
        }
    }

    fn breaking(&mut self) {
        use DebugCommand::*;

        loop {
            while let Some(message) = self.receive_cache.pop_front() {
                match message {
                    StepOne => return,
                    Continue => {
                        self.state = BreakpointHandleState::Running;
                        return;
                    },
                    Pause | SetBreakpoints(_) | RemoveBreakpoints(_) => panic!("BreakpointHandle: Message should never be added to the message cache but handled immediately."),
                }
            }

            self.receive_update_blocking();
        }
    }

    #[inline]
    fn send_registers<const SIZE: usize>(&self, registers: &crate::processor::Registers<SIZE>) {
        self.send(DebugMessage::Registers(registers.contents().to_vec()));
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
    fn handle_message(&mut self, message: DebugCommand) {
        match message {
            DebugCommand::Pause => {
                self.should_pause = true;
            }
            DebugCommand::SetBreakpoints(locations) => {
                self.breakpoints.extend(locations);
            }
            DebugCommand::RemoveBreakpoints(locations) => {
                for location in locations {
                    self.breakpoints.remove(&location);
                }
            }
            _ => self.receive_cache.push_back(message),
        }
    }
}

impl Debugger {
    fn new(receiver: Receiver<DebugMessage>, breakpoint_sender: Sender<DebugCommand>) -> Self {
        Self {
            receiver,
            breakpoint_sender,
            started: false,
            start_notifications: Vec::new(),
        }
    }

    fn run(mut self) {
        let mut tcp = TcpHandler::start();
        let tcp_poll = tick(TCP_POLL_INTERVAL);

        loop {
            select! {
                recv(tcp_poll) -> _ => self.handle_poll_result(tcp.poll()),
                recv(self.receiver) -> message => {
                    let message = message.expect("Debugger cannot receive message on debug interface.");
                    if let DebugMessage::Stop = message {
                        break;
                    }
                    self.handle_debug_message(message, &mut tcp)
                }
            };
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
            DebugMessage::Pausing(location) => {
                let message = tcp_protocol::Response::Pausing { location };
                self.handle_tcp_result(tcp.send(&message));
            }
            DebugMessage::Registers(registers) => {
                let message = tcp_protocol::Response::Registers { registers };
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
            tcp_protocol::Request::StartExecution { stop_on_entry } => {
                if stop_on_entry {
                    self.send_to_breakpoint_handler(DebugCommand::Pause);
                }
                self.started = true;
                self.start_notifications.clear(); // ==> notify all
            }
            tcp_protocol::Request::SetBreakpoints { locations } => {
                self.send_to_breakpoint_handler(DebugCommand::SetBreakpoints(locations))
            }
            tcp_protocol::Request::RemoveBreakpoints { locations } => {
                self.send_to_breakpoint_handler(DebugCommand::RemoveBreakpoints(locations))
            }
            tcp_protocol::Request::Continue {} => {
                self.send_to_breakpoint_handler(DebugCommand::Continue)
            }
            tcp_protocol::Request::StepOne {} => {
                self.send_to_breakpoint_handler(DebugCommand::StepOne)
            }
        }
    }

    fn send_to_breakpoint_handler(&mut self, message: DebugCommand) {
        match self.breakpoint_sender.try_send(message) {
            Ok(_) | Err(crossbeam_channel::TrySendError::Full(_)) => {}
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                panic!("Breakpoint channel closed before debugger was stopped.")
            }
        }
    }
}
