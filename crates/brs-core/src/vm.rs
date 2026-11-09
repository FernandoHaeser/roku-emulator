//! Resumable bytecode VM. `run(budget)` executes at most `budget`
//! instructions and returns, so the host (browser worker or desktop loop)
//! keeps control: sleeping, waiting and time slicing never block the host.

use std::collections::VecDeque;
use std::rc::Rc;

use crate::ast::TypeName;
use crate::builtins;
use crate::bytecode::{FunctionProto, Instr, Program};
use crate::error::RuntimeError;
use crate::ops;
use crate::value::{to_print_string, AssocArray, FunctionRef, Object, Value};

/// Services the embedding environment provides.
pub trait Host {
    /// Console output (the device's telnet port 8085).
    fn write(&mut self, text: &str);
    /// Monotonic clock in milliseconds.
    fn now_ms(&self) -> f64;
    /// Wall clock (milliseconds since the Unix epoch), for roDateTime.
    fn epoch_ms(&self) -> f64 {
        0.0
    }
}

/// Channel metadata shown by the debugger crash dump.
#[derive(Debug, Clone, Default)]
pub struct ChannelInfo {
    pub title: String,
    /// "major.minor.build" from the manifest.
    pub version: String,
    /// MD5 (hex) of the package zip; the dump shows its first 8 digits.
    pub package_md5: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    /// Budget exhausted; call `run` again.
    Running,
    /// Blocked in `Sleep()` until the host clock reaches this time.
    Sleeping {
        until_ms: f64,
    },
    Finished,
    /// Uncaught runtime error; the crash dump was written to the console.
    Crashed,
    /// `STOP` statement reached (debugger break).
    Stopped,
    /// Every thread is blocked on input (remote keys, messages) with no
    /// deadline; the host calls `run` again after delivering an event.
    Waiting,
}

/// Thread ids: the channel's `Main` and the SceneGraph render thread are
/// fixed; Task threads are appended.
pub const MAIN_THREAD: usize = 0;
pub const RENDER_THREAD: usize = 1;
const NO_THREAD: usize = usize::MAX;
/// Instructions a thread runs before the scheduler may switch (preemption).
const QUANTUM: u64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ThreadKind {
    Main,
    Render,
    Task,
}

/// Parked execution state of a thread (see `Vm::load`).
struct Thread {
    kind: ThreadKind,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    status: Status,
    wait: Option<WaitState>,
    sync_floor: usize,
    /// Task node, for state updates when the thread ends.
    task_node: Option<crate::value::ObjectRef>,
}

impl Thread {
    fn new(kind: ThreadKind, status: Status) -> Thread {
        Thread {
            kind,
            stack: Vec::new(),
            frames: Vec::new(),
            status,
            wait: None,
            sync_floor: 0,
            task_node: None,
        }
    }
}

/// A thread blocked in `wait(timeout, port)`.
#[derive(Debug, Clone, Copy)]
struct WaitState {
    port: usize,
    deadline_ms: Option<f64>,
}

struct Handler {
    target: u32,
    stack_height: usize,
}

struct Frame {
    function: u32,
    ip: usize,
    /// Stack index of the first local; the callee value sits right below.
    base: usize,
    m: Value,
    handlers: Vec<Handler>,
    /// Entry frame of a `call_sync`: its return value stays on the stack for
    /// the native caller instead of ending the thread.
    sync_root: bool,
}

pub struct Vm {
    program: Rc<Program>,
    // Registers of the thread being executed (`current`); other threads are
    // parked in `threads`.
    stack: Vec<Value>,
    frames: Vec<Frame>,
    status: Status,
    wait: Option<WaitState>,
    /// Frames below this index belong to native callers of `call_sync`; a
    /// TRY handler there must not catch errors raised inside the callback.
    sync_floor: usize,
    current: usize,
    threads: Vec<Thread>,
    next_thread: usize,
    /// Any thread crashed: the whole channel stops, like on the device.
    crashed: bool,
    global_m: Value,
    host: Box<dyn Host>,
    channel: ChannelInfo,
    column: usize,
    rng_state: u64,
    ports: Vec<VecDeque<Value>>,
    pub sg: crate::sg::SceneGraph,
    pub env: crate::environment::Environment,
}

impl Vm {
    pub fn new(program: Program, host: Box<dyn Host>, channel: ChannelInfo) -> Vm {
        Vm {
            program: Rc::new(program),
            stack: Vec::new(),
            frames: Vec::new(),
            status: Status::Finished,
            wait: None,
            sync_floor: 0,
            current: NO_THREAD,
            threads: vec![
                Thread::new(ThreadKind::Main, Status::Finished),
                Thread::new(ThreadKind::Render, Status::Finished),
            ],
            next_thread: 0,
            crashed: false,
            global_m: Value::object(Object::AssocArray(AssocArray::new())),
            host,
            channel,
            column: 0,
            rng_state: 0x2545_f491_4f6c_dd1d,
            ports: Vec::new(),
            sg: crate::sg::SceneGraph::default(),
            env: crate::environment::Environment::default(),
        }
    }

    pub fn channel_info(&self) -> &ChannelInfo {
        &self.channel
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn status(&self) -> Status {
        self.status
    }

    /// Prepares a call to the named entry function (e.g. "Main").
    pub fn start(&mut self, entry: &str, args: Vec<Value>) -> Result<(), String> {
        let index = *self
            .program
            .function_index
            .get(&entry.to_lowercase())
            .ok_or_else(|| format!("Entry point {entry} not found"))?;
        let proto = &self.program.functions[index as usize];
        let args: Vec<Value> = args.into_iter().take(proto.params.len()).collect();
        self.load(MAIN_THREAD);
        self.stack.clear();
        self.frames.clear();
        self.stack.push(Value::Function(FunctionRef::User(index)));
        let argc = args.len();
        self.stack.extend(args);
        self.status = Status::Running;
        let m = self.global_m.clone();
        if let Err(error) = self.enter_user_function(index, argc, m) {
            self.crash(&error);
        }
        self.park();
        Ok(())
    }

    // ---- console --------------------------------------------------------------

    pub fn write(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // The device counts print columns in UTF-8 bytes
        // (fidelity/runtime/datetime: "Terça-feira ...").
        match text.rfind('\n') {
            Some(position) => self.column = text.len() - position - 1,
            None => self.column += text.len(),
        }
        self.host.write(text);
    }

    pub fn now_ms(&self) -> f64 {
        self.host.now_ms()
    }

    pub fn epoch_ms(&self) -> f64 {
        self.host.epoch_ms()
    }

    pub fn next_random(&mut self) -> f64 {
        // xorshift64*: deterministic, good enough for Rnd().
        self.rng_state ^= self.rng_state >> 12;
        self.rng_state ^= self.rng_state << 25;
        self.rng_state ^= self.rng_state >> 27;
        let value = self.rng_state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        (value >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn global_m(&self) -> Value {
        self.global_m.clone()
    }

    pub fn sleep(&mut self, milliseconds: f64) {
        let until_ms = self.now_ms() + milliseconds.max(0.0);
        self.status = Status::Sleeping { until_ms };
    }

    /// "pkg:/source/main.brs(9)" for the instruction being executed.
    pub fn current_location(&self) -> String {
        let Some(frame) = self.frames.last() else {
            return String::new();
        };
        let proto = &self.program.functions[frame.function as usize];
        let line = proto
            .lines
            .get(frame.ip.saturating_sub(1))
            .copied()
            .unwrap_or(proto.first_line);
        format!("{}({line})", proto.file)
    }

    // ---- execution ------------------------------------------------------------

    /// Runs the channel for at most `budget` instructions across all
    /// threads. Returns `Running` when the budget ran out, `Sleeping` when
    /// every thread waits for a deadline, `Waiting` when only input can wake
    /// it, or the final status.
    pub fn run(&mut self, budget: u64) -> Status {
        let mut remaining = budget;
        loop {
            self.service();
            if self.crashed {
                return Status::Crashed;
            }
            match self.threads[MAIN_THREAD].status {
                Status::Finished => return Status::Finished,
                Status::Stopped => return Status::Stopped,
                _ => {}
            }
            let Some(thread) = self.pick_thread() else {
                return self.idle_status();
            };
            if remaining == 0 {
                return Status::Running;
            }
            let quantum = remaining.min(QUANTUM);
            self.load(thread);
            let used = self.execute_slice(quantum);
            self.park();
            remaining -= used.min(remaining);
            if remaining == 0 {
                self.service();
                return if self.crashed {
                    Status::Crashed
                } else {
                    Status::Running
                };
            }
        }
    }

    /// Next runnable Main/Task thread, round-robin.
    fn pick_thread(&mut self) -> Option<usize> {
        let count = self.threads.len();
        for offset in 0..count {
            let id = (self.next_thread + offset) % count;
            let thread = &self.threads[id];
            if thread.kind != ThreadKind::Render && thread.status == Status::Running {
                self.next_thread = (id + 1) % count;
                return Some(id);
            }
        }
        None
    }

    fn idle_status(&self) -> Status {
        let mut earliest: Option<f64> = None;
        let mut consider = |time: f64| {
            earliest = Some(earliest.map_or(time, |e: f64| e.min(time)));
        };
        for thread in &self.threads {
            match thread.status {
                Status::Sleeping { until_ms } => consider(until_ms),
                Status::Waiting => {
                    if let Some(deadline) = thread.wait.and_then(|w| w.deadline_ms) {
                        consider(deadline);
                    }
                }
                _ => {}
            }
        }
        for timer in &self.sg.timers {
            consider(timer.due_ms);
        }
        match earliest {
            Some(until_ms) => Status::Sleeping { until_ms },
            None => Status::Waiting,
        }
    }

    /// Housekeeping between slices: render-thread work (pending scenes,
    /// timers, keys), finished tasks, and waking blocked threads.
    fn service(&mut self) {
        if self.crashed {
            return;
        }
        crate::sg::runtime::service(self);
        let now = self.now_ms();
        for id in 0..self.threads.len() {
            let thread = &mut self.threads[id];
            match thread.status {
                Status::Sleeping { until_ms } if now >= until_ms => thread.status = Status::Running,
                Status::Waiting => {
                    let Some(wait) = thread.wait else { continue };
                    if let Some(message) =
                        self.ports.get_mut(wait.port).and_then(VecDeque::pop_front)
                    {
                        // The pending `wait()` call left a placeholder result.
                        if let Some(top) = thread.stack.last_mut() {
                            *top = message;
                        }
                        thread.wait = None;
                        thread.status = Status::Running;
                    } else if wait.deadline_ms.is_some_and(|deadline| now >= deadline) {
                        thread.wait = None;
                        thread.status = Status::Running;
                    }
                }
                _ => {}
            }
        }
    }

    /// Executes up to `budget` instructions of the loaded thread; returns how
    /// many ran.
    fn execute_slice(&mut self, budget: u64) -> u64 {
        let program = self.program.clone();
        let mut executed = 0;
        while executed < budget {
            executed += 1;
            let Some(frame) = self.frames.last_mut() else {
                self.status = Status::Finished;
                break;
            };
            let proto = &program.functions[frame.function as usize];
            let Some(instr) = proto.code.get(frame.ip) else {
                self.status = Status::Finished;
                break;
            };
            frame.ip += 1;
            if let Err(error) = self.execute(instr, proto, &program) {
                self.raise(error);
            }
            if self.status != Status::Running {
                break;
            }
        }
        executed
    }

    // ---- threads ----------------------------------------------------------------

    fn park(&mut self) {
        if self.current == NO_THREAD {
            return;
        }
        let thread = &mut self.threads[self.current];
        std::mem::swap(&mut thread.stack, &mut self.stack);
        std::mem::swap(&mut thread.frames, &mut self.frames);
        thread.status = self.status;
        thread.wait = self.wait.take();
        thread.sync_floor = self.sync_floor;
        self.current = NO_THREAD;
    }

    fn load(&mut self, id: usize) {
        if self.current == id {
            return;
        }
        self.park();
        let thread = &mut self.threads[id];
        std::mem::swap(&mut thread.stack, &mut self.stack);
        std::mem::swap(&mut thread.frames, &mut self.frames);
        self.status = thread.status;
        self.wait = thread.wait.take();
        self.sync_floor = thread.sync_floor;
        self.current = id;
    }

    pub fn current_thread(&self) -> usize {
        self.current
    }

    /// True when the thread can execute (not blocked or finished).
    pub fn thread_runnable(&self, id: usize) -> bool {
        let status = if id == self.current {
            self.status
        } else {
            self.threads[id].status
        };
        status == Status::Running
    }

    /// Scope of the function being executed (for observeField callbacks).
    pub fn current_function_scope(&self) -> u32 {
        self.frames
            .last()
            .map_or(0, |f| self.program.functions[f.function as usize].scope)
    }

    /// `m` of the function being executed.
    pub fn current_m(&self) -> Value {
        self.frames
            .last()
            .map_or_else(|| self.global_m.clone(), |f| f.m.clone())
    }

    /// Runs `f` on the render thread (callbacks, init, onKeyEvent), then
    /// returns to the thread that was executing.
    pub fn run_in_render<T>(&mut self, f: impl FnOnce(&mut Vm) -> T) -> T {
        let previous = self.current;
        self.load(RENDER_THREAD);
        let result = f(self);
        if previous == NO_THREAD {
            self.park();
        } else {
            self.load(previous);
        }
        result
    }

    /// Starts a Task thread running `function` with the task component's `m`.
    pub fn spawn_task(&mut self, function: u32, m: Value, node: crate::value::ObjectRef) {
        let mut thread = Thread::new(ThreadKind::Task, Status::Running);
        thread.task_node = Some(node);
        let id = self.threads.len();
        self.threads.push(thread);
        let previous = self.current;
        self.load(id);
        self.stack
            .push(Value::Function(FunctionRef::User(function)));
        if let Err(error) = self.enter_user_function(function, 0, m) {
            self.crash(&error);
        }
        if previous == NO_THREAD {
            self.park();
        } else {
            self.load(previous);
        }
    }

    /// Stops the Task thread attached to `node`, if running.
    pub fn stop_task(&mut self, node: &crate::value::ObjectRef) {
        for (id, thread) in self.threads.iter_mut().enumerate() {
            if id != self.current
                && thread.kind == ThreadKind::Task
                && thread
                    .task_node
                    .as_ref()
                    .is_some_and(|n| Rc::ptr_eq(n, node))
            {
                thread.status = Status::Finished;
                thread.frames.clear();
                thread.stack.clear();
                thread.task_node = None;
            }
        }
    }

    /// Task nodes whose thread has ended since the last call.
    pub fn take_finished_tasks(&mut self) -> Vec<crate::value::ObjectRef> {
        let mut finished = Vec::new();
        for thread in &mut self.threads {
            if thread.kind == ThreadKind::Task && thread.status == Status::Finished {
                if let Some(node) = thread.task_node.take() {
                    finished.push(node);
                }
            }
        }
        finished
    }

    /// Calls a BrightScript function to completion on the loaded thread and
    /// returns its result (observers, init, callFunc, onKeyEvent). Sleep and
    /// wait inside the callback do not block.
    pub fn call_sync(
        &mut self,
        function: u32,
        args: Vec<Value>,
        m: Value,
    ) -> Result<Value, RuntimeError> {
        if self.crashed {
            return Err(RuntimeError::aborted());
        }
        let params = self.program.functions[function as usize].params.len();
        let args: Vec<Value> = args.into_iter().take(params).collect();
        let depth = self.frames.len();
        let stack_height = self.stack.len();
        let saved_status = self.status;
        let saved_floor = self.sync_floor;
        let saved_wait = self.wait.take();
        self.status = Status::Running;
        self.sync_floor = depth;
        self.stack
            .push(Value::Function(FunctionRef::User(function)));
        let argc = args.len();
        self.stack.extend(args);
        if let Err(error) = self.enter_user_function(function, argc, m) {
            self.stack.truncate(stack_height);
            self.status = saved_status;
            self.sync_floor = saved_floor;
            self.wait = saved_wait;
            return Err(error);
        }
        if let Some(frame) = self.frames.last_mut() {
            frame.sync_root = true;
        }
        let program = self.program.clone();
        let result = loop {
            if self.frames.len() <= depth {
                break Ok(self.stack.pop().unwrap_or(Value::Invalid));
            }
            let frame = self.frames.last_mut().expect("frame");
            let proto = &program.functions[frame.function as usize];
            let Some(instr) = proto.code.get(frame.ip) else {
                break Err(RuntimeError::aborted());
            };
            frame.ip += 1;
            if let Err(error) = self.execute(instr, proto, &program) {
                self.raise(error);
            }
            match self.status {
                Status::Running => {}
                Status::Sleeping { .. } | Status::Waiting => {
                    self.status = Status::Running;
                    self.wait = None;
                }
                Status::Crashed => break Err(RuntimeError::aborted()),
                Status::Finished | Status::Stopped => {
                    // END/STOP inside a callback ends the channel.
                    self.threads[MAIN_THREAD].status = Status::Finished;
                    break Err(RuntimeError::aborted());
                }
            }
        };
        if result.is_err() {
            self.stack.truncate(stack_height);
            self.frames.truncate(depth);
        }
        self.status = if self.crashed {
            Status::Crashed
        } else {
            saved_status
        };
        self.sync_floor = saved_floor;
        self.wait = saved_wait;
        result
    }

    // ---- message ports ------------------------------------------------------------

    pub fn new_port(&mut self) -> usize {
        self.ports.push(VecDeque::new());
        self.ports.len() - 1
    }

    pub fn post_message(&mut self, port: usize, message: Value) {
        if let Some(queue) = self.ports.get_mut(port) {
            queue.push_back(message);
        }
    }

    pub fn take_message(&mut self, port: usize) -> Option<Value> {
        self.ports.get_mut(port).and_then(VecDeque::pop_front)
    }

    pub fn peek_message(&self, port: usize) -> Option<Value> {
        self.ports.get(port).and_then(|q| q.front().cloned())
    }

    /// `wait(timeout, port)`: returns a message if one is queued, otherwise
    /// blocks the thread (the result is patched in when it wakes).
    pub fn wait_for_message(&mut self, port: usize, timeout_ms: f64) -> Value {
        if let Some(message) = self.take_message(port) {
            return message;
        }
        let deadline_ms = (timeout_ms > 0.0).then(|| self.now_ms() + timeout_ms);
        // The render thread may not block on the device; there it never waits.
        if self.current != RENDER_THREAD && self.sync_floor == 0 {
            self.wait = Some(WaitState { port, deadline_ms });
            self.status = Status::Waiting;
        }
        Value::Invalid
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap_or(Value::Invalid)
    }

    fn pop_n(&mut self, count: usize) -> Vec<Value> {
        let start = self.stack.len().saturating_sub(count);
        self.stack.split_off(start)
    }

    fn frame(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("active frame")
    }

    fn jump(&mut self, target: u32) {
        self.frame().ip = target as usize;
    }

    fn local(&self, slot: u16) -> usize {
        self.frames.last().expect("active frame").base + slot as usize
    }

    fn execute(
        &mut self,
        instr: &Instr,
        proto: &FunctionProto,
        program: &Program,
    ) -> Result<(), RuntimeError> {
        match instr {
            Instr::Constant(index) => self.stack.push(program.constants[*index as usize].clone()),
            Instr::LoadLocal(slot) => {
                let value = self.stack[self.local(*slot)].clone();
                self.stack.push(value);
            }
            Instr::StoreLocal(slot) => {
                let value = self.pop();
                let index = self.local(*slot);
                self.stack[index] = value;
            }
            Instr::LoadFunction(index) => {
                self.stack.push(Value::Function(FunctionRef::User(*index)))
            }
            Instr::LoadBuiltin(index) => self
                .stack
                .push(Value::Function(FunctionRef::Builtin(*index))),
            Instr::LoadM => {
                let m = self
                    .frames
                    .last()
                    .map(|f| f.m.clone())
                    .unwrap_or(Value::Invalid);
                self.stack.push(m);
            }
            Instr::Pop => {
                self.pop();
            }
            Instr::Dup => {
                let top = self.stack.last().cloned().unwrap_or(Value::Invalid);
                self.stack.push(top);
            }
            Instr::Binary(op) => {
                let right = self.pop();
                let left = self.pop();
                self.stack.push(ops::binary(*op, &left, &right)?);
            }
            Instr::Unary(op) => {
                let operand = self.pop();
                self.stack.push(ops::unary(*op, &operand)?);
            }
            Instr::AndLeft(target) => {
                if matches!(self.stack.last(), Some(Value::Boolean(false))) {
                    self.jump(*target);
                }
            }
            Instr::OrLeft(target) => {
                if matches!(self.stack.last(), Some(Value::Boolean(true))) {
                    self.jump(*target);
                }
            }
            Instr::AndRight | Instr::OrRight => {
                let right = self.pop();
                let left = self.pop();
                let op = if matches!(instr, Instr::AndRight) {
                    crate::ast::BinaryOp::And
                } else {
                    crate::ast::BinaryOp::Or
                };
                self.stack.push(ops::bitwise(op, &left, &right)?);
            }
            Instr::Jump(target) => self.jump(*target),
            Instr::JumpIfFalse(target) => {
                let value = self.pop();
                if !ops::condition(&value)? {
                    self.jump(*target);
                }
            }
            Instr::JumpIfInitialized(slot, target) => {
                if !matches!(self.stack[self.local(*slot)], Value::Uninitialized) {
                    self.jump(*target);
                }
            }
            Instr::CoerceLocal(slot, ty) => {
                let index = self.local(*slot);
                let value = std::mem::replace(&mut self.stack[index], Value::Invalid);
                self.stack[index] = coerce(value, *ty)?;
            }
            Instr::Call(argc) => self.call(*argc as usize)?,
            Instr::CallMethod(name, argc) => {
                let name = program.constants[*name as usize]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                self.call_method(&name, *argc as usize)?;
            }
            Instr::GetField(name) => {
                let object = self.pop();
                let name = program.constants[*name as usize]
                    .as_str()
                    .unwrap_or_default();
                self.stack.push(get_field(&object, name)?);
            }
            Instr::SetField(name) => {
                let value = self.pop();
                let object = self.pop();
                let name = program.constants[*name as usize]
                    .as_str()
                    .unwrap_or_default();
                match crate::sg::node_ref(&object) {
                    Some(node) => crate::sg::runtime::set_field(self, &node, name, value)?,
                    None => set_field(&object, name, value)?,
                }
            }
            Instr::GetIndex(count) => {
                let indexes: Vec<Value> = self
                    .pop_n(*count as usize)
                    .iter()
                    .map(Value::unboxed)
                    .collect();
                let object = self.pop();
                self.stack.push(get_index(&object, &indexes)?);
            }
            Instr::SetIndex(count) => {
                let value = self.pop();
                let indexes: Vec<Value> = self
                    .pop_n(*count as usize)
                    .iter()
                    .map(Value::unboxed)
                    .collect();
                let object = self.pop();
                match (crate::sg::node_ref(&object), indexes.as_slice()) {
                    (Some(node), [Value::String(key)]) => {
                        crate::sg::runtime::set_field(self, &node, key, value)?
                    }
                    _ => set_index(&object, &indexes, value)?,
                }
            }
            Instr::NewArray(count) => {
                let items = self.pop_n(*count as usize);
                self.stack.push(Value::object(Object::Array(items)));
            }
            Instr::NewAssocArray(count) => {
                let values = self.pop_n(*count as usize * 2);
                let mut aa = AssocArray::new();
                for pair in values.chunks(2) {
                    if let [Value::String(key), value] = pair {
                        aa.set(key, value.clone());
                    }
                }
                self.stack.push(Value::object(Object::AssocArray(aa)));
            }
            Instr::NewDimArray(count) => {
                let dimensions = self.pop_n(*count as usize);
                let sizes: Vec<usize> = dimensions
                    .iter()
                    .map(|d| d.as_i64().unwrap_or(0).max(0) as usize + 1)
                    .collect();
                self.stack.push(dim_array(&sizes));
            }
            Instr::ForCheck {
                variable,
                end,
                step,
                exit,
            } => {
                let current = self.stack[self.local(*variable)].as_f64();
                let limit = self.stack[self.local(*end)].as_f64();
                let increment = self.stack[self.local(*step)].as_f64();
                let (Some(current), Some(limit), Some(increment)) = (current, limit, increment)
                else {
                    return Err(RuntimeError::type_mismatch(
                        "Type Mismatch. FOR loop requires numeric values.".into(),
                    ));
                };
                let done = if increment >= 0.0 {
                    current > limit
                } else {
                    current < limit
                };
                if done {
                    self.jump(*exit);
                }
            }
            Instr::IterInit(slot) => {
                let iterable = self.pop();
                let items = ops::iteration_items(&iterable)?;
                let index = self.local(*slot);
                self.stack[index] = Value::object(Object::Iterator { items, position: 0 });
            }
            Instr::IterNext {
                iterator,
                variable,
                exit,
            } => {
                let next = match &self.stack[self.local(*iterator)] {
                    Value::Object(object) => match &mut *object.borrow_mut() {
                        Object::Iterator { items, position } => {
                            let item = items.get(*position).cloned();
                            *position += 1;
                            item
                        }
                        _ => None,
                    },
                    _ => None,
                };
                match next {
                    Some(item) => {
                        let index = self.local(*variable);
                        self.stack[index] = item;
                    }
                    None => self.jump(*exit),
                }
            }
            Instr::PrintStacked { index, count } => {
                let position = self.stack.len() - *count as usize + *index as usize;
                let text = to_print_string(&self.stack[position]);
                self.write(&text);
            }
            Instr::PopN(count) => {
                let keep = self.stack.len() - *count as usize;
                self.stack.truncate(keep);
            }
            Instr::PrintComma => {
                // Next 16-column zone; a cursor exactly on a boundary moves on.
                let target = (self.column / 16 + 1) * 16;
                let padding = " ".repeat(target - self.column);
                self.write(&padding);
            }
            Instr::PrintNewline => self.write("\n"),
            Instr::TryBegin(target) => {
                let stack_height = self.stack.len();
                self.frame().handlers.push(Handler {
                    target: *target,
                    stack_height,
                });
            }
            Instr::TryEnd => {
                self.frame().handlers.pop();
            }
            Instr::Throw => {
                let value = self.pop();
                return Err(thrown_error(&value));
            }
            Instr::Return => self.return_from_function(Value::Invalid, proto)?,
            Instr::ReturnValue => {
                let value = self.pop();
                self.return_from_function(value, proto)?;
            }
            Instr::Stop => self.status = Status::Stopped,
            Instr::End => {
                self.frames.clear();
                self.status = Status::Finished;
            }
        }
        Ok(())
    }

    fn return_from_function(
        &mut self,
        value: Value,
        proto: &FunctionProto,
    ) -> Result<(), RuntimeError> {
        let value = match proto.return_type {
            Some(ty)
                if !matches!(ty, TypeName::Dynamic | TypeName::Void | TypeName::Object)
                    && !proto.is_sub =>
            {
                coerce(value, ty)?
            }
            _ => value,
        };
        let frame = self.frames.pop().expect("active frame");
        // Drop locals and the callee slot, then push the result.
        self.stack.truncate(frame.base - 1);
        if self.frames.is_empty() && !frame.sync_root {
            self.status = Status::Finished;
        } else {
            self.stack.push(value);
        }
        Ok(())
    }

    fn enter_user_function(
        &mut self,
        index: u32,
        argc: usize,
        m: Value,
    ) -> Result<(), RuntimeError> {
        let program = self.program.clone();
        let proto = &program.functions[index as usize];
        if argc > proto.params.len() {
            return Err(RuntimeError::wrong_argument_count());
        }
        if proto.params[argc..].iter().any(|p| !p.has_default) {
            return Err(RuntimeError::wrong_argument_count());
        }
        let base = self.stack.len() - argc;
        self.stack
            .resize(base + proto.locals.len(), Value::Uninitialized);
        self.frames.push(Frame {
            function: index,
            ip: 0,
            base,
            m,
            handlers: Vec::new(),
            sync_root: false,
        });
        Ok(())
    }

    fn call(&mut self, argc: usize) -> Result<(), RuntimeError> {
        let callee_index = self.stack.len() - argc - 1;
        match self.stack[callee_index].clone() {
            Value::Function(FunctionRef::User(index)) => {
                let m = self.global_m.clone();
                self.enter_user_function(index, argc, m)
            }
            Value::Function(FunctionRef::Builtin(index)) => {
                let args = self.pop_n(argc);
                self.pop(); // callee
                let result = builtins::call(self, index, args)?;
                self.stack.push(result);
                Ok(())
            }
            _ => Err(RuntimeError::not_a_function()),
        }
    }

    fn call_method(&mut self, name: &str, argc: usize) -> Result<(), RuntimeError> {
        let receiver_index = self.stack.len() - argc - 1;
        let receiver = self.stack[receiver_index].clone();
        // AA members holding functions are methods: `m` becomes the AA.
        if let Value::Object(object) = &receiver {
            let member = match &*object.borrow() {
                Object::AssocArray(aa) => aa.get(name).cloned(),
                _ => None,
            };
            if let Some(Value::Function(FunctionRef::User(index))) = member {
                self.stack[receiver_index] = Value::Function(FunctionRef::User(index));
                return self.enter_user_function(index, argc, receiver);
            }
        }
        let args = self.pop_n(argc);
        self.pop(); // receiver
        let result = builtins::call_method(self, &receiver, &name.to_lowercase(), args)?;
        self.stack.push(result);
        Ok(())
    }

    // ---- errors -----------------------------------------------------------------

    /// Unwinds to the nearest TRY handler, or crashes into the debugger.
    fn raise(&mut self, error: RuntimeError) {
        if error.is_aborted() {
            // A callback already crashed (and printed the dump).
            self.crashed = true;
            self.status = Status::Crashed;
            return;
        }
        // Frames are only unwound when a handler exists, so an uncaught error
        // still reports the frame where it happened.
        let floor = self.sync_floor;
        let Some(depth) = self
            .frames
            .iter()
            .enumerate()
            .skip(floor)
            .rev()
            .find(|(_, frame)| !frame.handlers.is_empty())
            .map(|(index, _)| index)
        else {
            self.crash(&error);
            return;
        };
        self.frames.truncate(depth + 1);
        let frame = self.frames.last_mut().expect("handler frame");
        let handler = frame.handlers.pop().expect("handler");
        frame.ip = handler.target as usize;
        self.stack.truncate(handler.stack_height);
        let mut details = AssocArray::new();
        details.set("message", Value::string(error.message.as_str()));
        details.set("number", Value::Integer(i32::from(error.code)));
        self.stack.push(Value::object(Object::AssocArray(details)));
    }

    /// Writes the BrightScript Micro Debugger dump, matching the device format.
    fn crash(&mut self, error: &RuntimeError) {
        let program = self.program.clone();
        let mut dump = String::from("\nBrightScript Micro Debugger.\nEnter any BrightScript statement, debug commands, or HELP.\n\n\n");
        let location = self.current_location();
        if let Some(frame) = self.frames.last() {
            let proto = &program.functions[frame.function as usize];
            let error_line = proto
                .lines
                .get(frame.ip.saturating_sub(1))
                .copied()
                .unwrap_or(proto.first_line);
            dump.push_str("Current Function:\n");
            if let Some(source) = program.sources.get(&proto.file) {
                let lines: Vec<&str> = source.lines().collect();
                let first = error_line.saturating_sub(8).max(proto.first_line);
                let last = (error_line + 4).min(proto.last_line);
                for number in first..=last {
                    let text = lines.get(number as usize - 1).copied().unwrap_or("");
                    let marker = if number == error_line { '*' } else { ' ' };
                    dump.push_str(&format!("{number:03}:{marker} {text}\n"));
                }
            }
            dump.push_str("Source Digest(s): \n");
            let digest = self
                .channel
                .package_md5
                .as_deref()
                .map(|md5| &md5[..8.min(md5.len())])
                .unwrap_or("00000000");
            dump.push_str(&format!(
                "pkg: dev {} {digest} {}\n\n",
                self.channel.version, self.channel.title
            ));
            dump.push_str(&format!("{error} in {location}\n"));
            dump.push_str("Backtrace:\n");
            for (depth, frame) in self.frames.iter().rev().enumerate() {
                let proto = &program.functions[frame.function as usize];
                let line = proto
                    .lines
                    .get(frame.ip.saturating_sub(1))
                    .copied()
                    .unwrap_or(proto.first_line);
                let signature = function_signature(proto);
                dump.push_str(&format!(
                    "#{depth}  {signature}\n   file/line: {}({line})\n",
                    proto.file
                ));
            }
            dump.push_str("Local Variables:\n");
            dump.push_str(&format!("{:<17}Interface:ifGlobal\n", "global"));
            let m_description = describe_variable(&frame.m);
            dump.push_str(&format!("{:<17}{m_description}\n", "m"));
            for (slot, name) in proto.locals.iter().enumerate() {
                if name.starts_with('$') {
                    continue;
                }
                let value = &self.stack[frame.base + slot];
                dump.push_str(&format!(
                    "{:<17}{}\n",
                    name.to_lowercase(),
                    describe_variable(value)
                ));
            }
        } else {
            dump.push_str(&format!("{error}\n"));
        }
        dump.push_str("\nBrightscript Debugger> ");
        self.write(&dump);
        self.status = Status::Crashed;
        self.crashed = true;
    }
}

fn function_signature(proto: &FunctionProto) -> String {
    let params: Vec<String> = proto
        .params
        .iter()
        .map(|p| format!("{} As Dynamic", p.name.to_lowercase()))
        .collect();
    let returns = if proto.is_sub {
        "Void".to_string()
    } else {
        match proto.return_type {
            Some(ty) => format!("{ty:?}"),
            None => "Dynamic".to_string(),
        }
    };
    format!(
        "Function {}({}) As {returns}",
        proto.name.to_lowercase(),
        params.join(", ")
    )
}

/// Variable description in the debugger's "Local Variables" listing.
fn describe_variable(value: &Value) -> String {
    match value {
        Value::Uninitialized => "<uninitialized>".into(),
        Value::String(text) => format!("String (VT_STR_CONST) val:\"{text}\""),
        Value::Integer(v) => format!("Integer val:{v}"),
        Value::LongInteger(v) => format!("LongInteger val:{v}"),
        Value::Float(v) => format!("Float val:{}", crate::value::format_float(*v)),
        Value::Double(v) => format!("Double val:{}", crate::value::format_double(*v)),
        Value::Boolean(b) => format!("Boolean val:{b}"),
        Value::Invalid => "Invalid val:invalid".into(),
        Value::Function(_) => "Function".into(),
        Value::Object(object) => {
            // The device reports its own reference count; the VM frame and the
            // caller each hold one, which matches the common case of 2.
            let refs = Rc::strong_count(object);
            match &*object.borrow() {
                Object::AssocArray(aa) => {
                    format!("roAssociativeArray refcnt={refs} count:{}", aa.len())
                }
                Object::Array(items) => format!("roArray refcnt={refs} count:{}", items.len()),
                other => format!("{} refcnt={refs}", other.class_name()),
            }
        }
    }
}

fn thrown_error(value: &Value) -> RuntimeError {
    match value {
        Value::String(message) => RuntimeError::new(RuntimeError::USER_THROW, message.to_string()),
        Value::Object(object) => match &*object.borrow() {
            Object::AssocArray(aa) => {
                let message = aa
                    .get("message")
                    .and_then(|m| m.as_str().map(str::to_string))
                    .unwrap_or_default();
                let code = aa
                    .get("number")
                    .and_then(Value::as_i64)
                    .unwrap_or(i64::from(RuntimeError::USER_THROW));
                RuntimeError::new(code as u8, message)
            }
            _ => RuntimeError::new(RuntimeError::USER_THROW, ""),
        },
        _ => RuntimeError::new(RuntimeError::USER_THROW, ""),
    }
}

/// Converts a value to a declared parameter/return type.
pub fn coerce(value: Value, ty: TypeName) -> Result<Value, RuntimeError> {
    let fail = |value: &Value| {
        RuntimeError::type_mismatch(format!(
            "Type Mismatch. Unable to cast \"{}\" to \"{}\".",
            value.type_name(),
            match ty {
                TypeName::LongInteger => "LongInteger".to_string(),
                other => format!("{other:?}"),
            }
        ))
    };
    Ok(match ty {
        TypeName::Integer if value.is_numeric() => {
            Value::Integer(value.as_f64().unwrap_or(0.0) as i32)
        }
        TypeName::LongInteger if value.is_numeric() => {
            Value::LongInteger(value.as_f64().unwrap_or(0.0) as i64)
        }
        TypeName::Float if value.is_numeric() => Value::Float(value.as_f64().unwrap_or(0.0) as f32),
        TypeName::Double if value.is_numeric() => Value::Double(value.as_f64().unwrap_or(0.0)),
        TypeName::String if matches!(value, Value::String(_)) => value,
        TypeName::Boolean if matches!(value, Value::Boolean(_)) => value,
        TypeName::Function if matches!(value, Value::Function(_)) => value,
        TypeName::Object | TypeName::Dynamic | TypeName::Void => value,
        _ => return Err(fail(&value)),
    })
}

fn dim_array(sizes: &[usize]) -> Value {
    match sizes.split_first() {
        Some((size, rest)) if !rest.is_empty() => {
            Value::object(Object::Array((0..*size).map(|_| dim_array(rest)).collect()))
        }
        Some((size, _)) => Value::object(Object::Array(vec![Value::Invalid; *size])),
        None => Value::object(Object::Array(Vec::new())),
    }
}

fn get_field(object: &Value, name: &str) -> Result<Value, RuntimeError> {
    if let Some(node) = crate::sg::node_ref(object) {
        return Ok(crate::sg::runtime::get_field(&node, name));
    }
    match object {
        Value::Object(object) => match &*object.borrow() {
            Object::AssocArray(aa) => Ok(aa.get(name).cloned().unwrap_or(Value::Invalid)),
            Object::XmlElement(element) => Ok(crate::components::named_children(element, name)),
            Object::XmlList(items) => {
                let mut found = Vec::new();
                for item in items {
                    if let Value::Object(item) = item {
                        if let Object::XmlElement(element) = &*item.borrow() {
                            if let Value::Object(list) =
                                crate::components::named_children(element, name)
                            {
                                if let Object::XmlList(more) = &*list.borrow() {
                                    found.extend(more.iter().cloned());
                                }
                            }
                        }
                    }
                }
                Ok(Value::object(Object::XmlList(found)))
            }
            _ => Err(RuntimeError::member_not_found()),
        },
        _ => Err(RuntimeError::invalid_dot()),
    }
}

fn set_field(object: &Value, name: &str, value: Value) -> Result<(), RuntimeError> {
    match object {
        Value::Object(object) => match &mut *object.borrow_mut() {
            Object::AssocArray(aa) => {
                aa.set(name, value);
                Ok(())
            }
            _ => Err(RuntimeError::member_not_found()),
        },
        _ => Err(RuntimeError::invalid_dot()),
    }
}

fn get_index(object: &Value, indexes: &[Value]) -> Result<Value, RuntimeError> {
    let Some((first, rest)) = indexes.split_first() else {
        return Err(RuntimeError::invalid_index());
    };
    let item = match object {
        Value::Object(object) => match &*object.borrow() {
            Object::Node(node) => {
                let key = first.as_str().ok_or_else(RuntimeError::invalid_index)?;
                node.field(key)
                    .map_or(Value::Invalid, |field| field.kind.read(&field.value))
            }
            Object::ByteArray(bytes) => {
                let position = first.as_i64().ok_or_else(RuntimeError::invalid_index)?;
                usize::try_from(position)
                    .ok()
                    .and_then(|i| bytes.get(i))
                    .map_or(Value::Invalid, |b| Value::Integer(i32::from(*b)))
            }
            Object::Array(items) | Object::List(items) | Object::XmlList(items) => {
                let position = first.as_i64().ok_or_else(RuntimeError::invalid_index)?;
                if position < 0 {
                    Value::Invalid
                } else {
                    items
                        .get(position as usize)
                        .cloned()
                        .unwrap_or(Value::Invalid)
                }
            }
            Object::AssocArray(aa) => {
                let key = first.as_str().ok_or_else(RuntimeError::invalid_index)?;
                aa.get(key).cloned().unwrap_or(Value::Invalid)
            }
            _ => return Err(RuntimeError::invalid_index()),
        },
        _ => return Err(RuntimeError::invalid_index()),
    };
    if rest.is_empty() {
        Ok(item)
    } else {
        get_index(&item, rest)
    }
}

fn set_index(object: &Value, indexes: &[Value], value: Value) -> Result<(), RuntimeError> {
    let Some((first, rest)) = indexes.split_first() else {
        return Err(RuntimeError::invalid_index());
    };
    if !rest.is_empty() {
        let inner = get_index(object, std::slice::from_ref(first))?;
        return set_index(&inner, rest, value);
    }
    match object {
        Value::Object(object) => match &mut *object.borrow_mut() {
            Object::ByteArray(bytes) => {
                let position = first
                    .as_i64()
                    .filter(|p| *p >= 0)
                    .ok_or_else(RuntimeError::invalid_index)?
                    as usize;
                if position >= bytes.len() {
                    bytes.resize(position + 1, 0);
                }
                bytes[position] = value.as_i64().unwrap_or(0) as u8;
                Ok(())
            }
            Object::Array(items) | Object::List(items) => {
                let position = first
                    .as_i64()
                    .filter(|p| *p >= 0)
                    .ok_or_else(RuntimeError::invalid_index)?
                    as usize;
                if position >= items.len() {
                    items.resize(position + 1, Value::Invalid);
                }
                items[position] = value;
                Ok(())
            }
            Object::AssocArray(aa) => {
                let key = first.as_str().ok_or_else(RuntimeError::invalid_index)?;
                aa.set(key, value);
                Ok(())
            }
            _ => Err(RuntimeError::invalid_index()),
        },
        _ => Err(RuntimeError::invalid_index()),
    }
}
