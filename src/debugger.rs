mod segmented_reader;
mod tcp_protocol;

use std::{
    collections::{HashSet, VecDeque},
    thread,
    time::Duration,
};

use crossbeam_channel::{bounded, select, tick, Receiver, Sender, TryRecvError};
use crossbeam_utils::sync::WaitGroup;

use self::tcp_protocol::{PollReturn, TcpHandler};
use crate::{memory::Memory, opcodes::Opcode, processor::Processor, Address, Register, Word};

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
    call_stack: Vec<Address>,
    did_execute_last_cycle: bool,
}

#[derive(Debug, PartialEq)]
pub enum ShouldExecuteInstruction {
    Yes,
    No,
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
    BreakState {
        registers: Vec<Word>,
        call_stack: Vec<Address>,
    },
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
    SetRegister(u8, Word),
    Terminate,
}

enum ShouldTerminate {
    Yes,
    No,
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
        call_stack: Vec::new(),
        did_execute_last_cycle: true,
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
            call_stack: Vec::with_capacity(0),
            did_execute_last_cycle: true,
        }
    }

    pub fn stop(&self) {
        self.send(DebugMessage::Stop);
    }

    pub fn before_instruction_execution(
        &mut self,
        processor: &mut Processor,
        memory: &mut Memory,
    ) -> ShouldExecuteInstruction {
        use BreakpointHandleState::*;

        let instruction_pointer = processor.get_instruction_pointer();

        if self.state == WaitingForStart {
            self.wait_for_start();
            self.state = Running;
            self.receive_cache.clear();
        }

        if self.state == Breaking {
            if self.did_execute_last_cycle {
                self.send_break_state(&processor.registers);
                self.send(DebugMessage::Breaking(instruction_pointer));
            }
        } else {
            self.start_breaking_if_requested(instruction_pointer, processor);
        }

        let result;
        if self.state == Breaking {
            result = self.breaking(processor);
        } else {
            result = ShouldExecuteInstruction::Yes
        }

        if let ShouldExecuteInstruction::Yes = result {
            self.track_call_stack(memory, instruction_pointer);
        }

        self.did_execute_last_cycle = result == ShouldExecuteInstruction::Yes;
        return result;
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
            self.state = Breaking;
            self.receive_cache.clear();
            self.send_break_state(&processor.registers);
            self.send(break_message);
        }
    }

    fn breaking(&mut self, processor: &mut Processor) -> ShouldExecuteInstruction {
        use DebugCommand::*;

        self.receive_updates_non_blocking();

        if let Some(message) = self.receive_cache.pop_front() {
            match message {
                Terminate => std::process::exit(0),
                StepOne => return ShouldExecuteInstruction::Yes,
                Continue => {
                    self.state = BreakpointHandleState::Running;
                    return ShouldExecuteInstruction::Yes;
                }
                SetRegister(register, value) => {
                    processor.registers[Register(register)] = value;
                }
                Pause | SetBreakpoints(_) | RemoveBreakpoints(_) => panic!("BreakpointHandle: Message should never be added to the message cache but handled immediately."),
            }
        }

        ShouldExecuteInstruction::No
    }

    #[inline]
    fn send_break_state<const SIZE: usize>(&self, registers: &crate::processor::Registers<SIZE>) {
        self.send(DebugMessage::BreakState {
            registers: registers.contents().to_vec(),
            call_stack: self.call_stack.clone(),
        });
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

    fn track_call_stack(&mut self, memory: &mut Memory, instruction_pointer: Address) {
        let opcode = memory.read_opcode(instruction_pointer);
        match opcode {
            Ok(Opcode::CallImmediate { .. })
            | Ok(Opcode::CallRegister { .. })
            | Ok(Opcode::CallPointer { .. }) => {
                self.call_stack.push(instruction_pointer);
            }
            Ok(Opcode::Return {}) => {
                self.call_stack.pop();
            }
            _ => {}
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
                recv(tcp_poll) -> _ => {
                    let poll_result = tcp.poll();
                    let result = self.handle_poll_result(&mut tcp, poll_result);
                    if let ShouldTerminate::Yes = result {
                        break;
                    }
                }
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

    fn handle_poll_result(
        &mut self,
        tcp: &mut TcpHandler,
        result: tcp_protocol::Result<PollReturn>,
    ) -> ShouldTerminate {
        let mut should_terminate = ShouldTerminate::No;

        match result {
            Ok(PollReturn::Nothing | PollReturn::ClientDisconnected) => {}
            Ok(PollReturn::ClientConnected) => {
                let message = &tcp_protocol::Response::Hello {
                    pid: std::process::id(),
                };
                self.handle_tcp_result(tcp.send(message));
            }
            Ok(PollReturn::ReceivedRequests(requests)) => {
                for request in requests {
                    if let tcp_protocol::Request::Terminate {} = request {
                        should_terminate = ShouldTerminate::Yes;
                    }
                    self.handle_request(request);
                }
            }
            Err(_) => self.handle_tcp_result(result),
        }

        should_terminate
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
            DebugMessage::BreakState {
                registers,
                call_stack,
            } => {
                let message = tcp_protocol::Response::BreakState {
                    registers,
                    call_stack,
                };
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
            tcp_protocol::Request::SetRegister { register, value } => {
                self.send_to_breakpoint_handler(DebugCommand::SetRegister(register, value))
            }
            tcp_protocol::Request::Terminate {} => {
                self.send_to_breakpoint_handler(DebugCommand::Terminate);
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
