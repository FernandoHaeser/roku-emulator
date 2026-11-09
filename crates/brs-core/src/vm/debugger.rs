//! Resumable debugger. Hosts with debug_wait preserve native callback stacks.
mod eval;
mod rpc;
use super::*;
pub(super) use rpc::Breakpoint;

const PROMPT: &str = "Brightscript Debugger> ";

#[derive(Clone, Copy)]
pub(super) enum StepMode {
    Into,
    Over,
    Out,
}

#[derive(Clone, Copy)]
pub(super) struct Position {
    function: u32,
    line: u32,
    depth: usize,
}

pub(super) struct DebugStep {
    thread: usize,
    from: Position,
    mode: StepMode,
}

impl Vm {
    fn debug_position(&self, before: bool) -> Option<Position> {
        let frame = self.frames.last()?;
        let proto = &self.program.functions[frame.function as usize];
        let ip = if before {
            frame.ip
        } else {
            frame.ip.saturating_sub(1)
        };
        Some(Position {
            function: frame.function,
            line: proto.lines.get(ip).copied().unwrap_or(proto.last_line),
            depth: self.frames.len(),
        })
    }

    pub(super) fn debug_should_step(&self) -> bool {
        let Some(step) = &self.debug_step else {
            return false;
        };
        if self.current != step.thread {
            return false;
        }
        let Some(now) = self.debug_position(true) else {
            return false;
        };
        let changed = now.function != step.from.function || now.line != step.from.line;
        match step.mode {
            StepMode::Into => changed,
            StepMode::Over => changed && now.depth <= step.from.depth,
            StepMode::Out => now.depth < step.from.depth,
        }
    }

    pub(super) fn debug_break(&mut self, before: bool) {
        if self.debug_eval_active {
            self.status = Status::Stopped;
            return;
        }
        if let Some(pos) = self.debug_position(before) {
            self.debug_last_position = Some((self.current, pos.function, pos.line));
        }
        self.debug_frame = 0;
        self.debug_inspect_thread = None;
        self.debug_thread = Some(self.current);
        self.debug_location = self.debug_position(before);
        self.debug_step = None;
        self.debug_saved_status = Some(self.status);
        self.status = Status::Stopped;
        self.write("\nBrightScript Micro Debugger.\nEnter any BrightScript statement, debug commands, or HELP.\n\n");
        let location = self.debug_location;
        if let Some(position) = location {
            let proto = &self.program.functions[position.function as usize];
            let file = proto.file.clone();
            let source = self.program.sources.get(&file).cloned();
            let text = source
                .as_deref()
                .and_then(|s| s.lines().nth(position.line.saturating_sub(1) as usize))
                .unwrap_or("");
            self.write(&format!(
                "Break in {file}({})\n{:03}:* {text}\n",
                position.line, position.line
            ));
        }
        self.debug_backtrace();
        self.write(PROMPT);
        let thread = self.current;
        while self.debug_thread.is_some() {
            let Some(command) = self.host.debug_wait() else {
                if self.sync_floor != 0 {
                    self.debug_thread = None;
                    self.debug_location = None;
                    self.debug_saved_status = None;
                }
                return;
            };
            self.debugger_command(&command);
            self.load(thread);
        }
        self.host.debug_resumed();
    }

    fn debug_backtrace(&mut self) {
        let mut out = String::new();
        for (depth, frame) in self.frames.iter().rev().enumerate() {
            let proto = &self.program.functions[frame.function as usize];
            let line = if depth == 0 {
                self.debug_location
                    .map(|p| p.line)
                    .unwrap_or(proto.first_line)
            } else {
                proto
                    .lines
                    .get(frame.ip.saturating_sub(1))
                    .copied()
                    .unwrap_or(proto.first_line)
            };
            out.push_str(&format!(
                "#{depth}  {}\n   file/line: {}({line})\n",
                function_signature(proto),
                proto.file
            ));
        }
        self.write(&out);
    }

    fn debug_locals(&mut self) {
        let Some(frame) = self
            .frames
            .len()
            .checked_sub(1 + self.debug_frame)
            .and_then(|i| self.frames.get(i))
        else {
            return;
        };
        let proto = &self.program.functions[frame.function as usize];
        let mut out = format!(
            "{:<17}Interface:ifGlobal\n{:<17}{}\n",
            "global",
            "m",
            describe_variable(&frame.m)
        );
        for (slot, name) in proto.locals.iter().enumerate() {
            if !name.starts_with('$') {
                let value = self
                    .stack
                    .get(frame.base + slot)
                    .unwrap_or(&Value::Uninitialized);
                out.push_str(&format!(
                    "{:<17}{}\n",
                    name.to_lowercase(),
                    describe_variable(value)
                ));
            }
        }
        self.write(&out);
    }

    /// Handles a bounded console command. Returns true when execution resumes.
    /// Expressions are read-only local/field lookups, not a general REPL.
    pub fn debugger_command(&mut self, command: &str) -> bool {
        let command = command.trim();
        if command.len() > 65_536 {
            return false;
        }
        if let Some(request) = command.strip_prefix("__emu ") {
            self.debug_rpc(request);
            return false;
        }
        if command == "\u{3}" || command.eq_ignore_ascii_case("break") {
            if self.debug_thread.is_none() && !self.crashed {
                let id = self
                    .threads
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.kind != ThreadKind::Render && !t.frames.is_empty())
                    .min_by_key(|(_, t)| t.status != Status::Running)
                    .map(|(id, _)| id);
                if let Some(id) = id {
                    self.load(id);
                    self.debug_break(true);
                    self.park();
                }
            }
            return false;
        }
        let Some(id) = self.debug_thread else {
            self.write("Not at a debugger prompt.\n");
            return false;
        };
        let owner = id;
        self.load(self.debug_inspect_thread.unwrap_or(id));
        let lower = command.to_ascii_lowercase();
        if let Some(frame) = lower
            .strip_prefix("frame ")
            .and_then(|v| v.parse::<usize>().ok())
        {
            if frame < self.frames.len() {
                self.debug_frame = frame;
            } else {
                self.write("Invalid frame index\n");
            }
            self.write(PROMPT);
            self.park();
            return false;
        }
        if let Some(thread) = lower
            .strip_prefix("thread ")
            .and_then(|v| v.parse::<usize>().ok())
        {
            if thread < self.threads.len()
                && (thread == self.current || !self.threads[thread].frames.is_empty())
            {
                self.load(thread);
                self.debug_inspect_thread = Some(thread);
                self.debug_frame = 0;
            } else {
                self.write("Invalid thread index\n");
            }
            self.write(PROMPT);
            self.park();
            return false;
        }
        let mode = match lower.as_str() {
            "s" | "step" => Some(StepMode::Into),
            "over" => Some(StepMode::Over),
            "out" => Some(StepMode::Out),
            _ => None,
        };
        if matches!(lower.as_str(), "c" | "cont" | "continue") || mode.is_some() {
            let selected = self.current;
            let from = if selected == owner {
                self.debug_location
            } else {
                self.debug_position(false)
            };
            self.debug_step = mode.and_then(|mode| {
                from.map(|from| DebugStep {
                    thread: selected,
                    from,
                    mode,
                })
            });
            self.load(owner);
            self.debug_thread = None;
            self.debug_location = None;
            self.status = self.debug_saved_status.take().unwrap_or(Status::Running);
            self.park();
            return true;
        }
        match lower.as_str() {
            "exit" | "quit" => {
                self.debug_thread = None;
                self.debug_step = None;
                self.status = Status::Finished;
                self.threads[MAIN_THREAD].status = Status::Finished;
                self.park();
                return true;
            }
            "bt" => self.debug_backtrace(),
            "var" => self.debug_locals(),
            "threads" => {
                if let Some(position) = self.debug_location {
                    let proto = &self.program.functions[position.function as usize];
                    let source = self.program.sources.get(&proto.file);
                    let text = source
                        .and_then(|s| s.lines().nth(position.line.saturating_sub(1) as usize))
                        .unwrap_or("");
                    self.write(&format!(
                        "Thread Location Source\n {id}* {}({}) {text}\n",
                        proto.file, position.line
                    ));
                }
            }
            "help" | "?" => {
                self.write("c/cont, step/s, over, out, bt, var, threads, thread <n>, print <expr>, <statement>\n")
            }
            "" => {}
            _ => {
                let code = command
                    .strip_prefix("? ")
                    .map(|expr| format!("print {expr}"))
                    .unwrap_or_else(|| command.to_string());
                if let Err(error) = self.debug_eval(&code) {
                    self.write(&format!("{error}\n"));
                }
            }
        }
        self.write(PROMPT);
        self.park();
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct TestHost(Rc<RefCell<String>>);
    impl Host for TestHost {
        fn write(&mut self, text: &str) {
            self.0.borrow_mut().push_str(text);
        }
        fn now_ms(&self) -> f64 {
            0.0
        }
    }
    fn vm(source: &str) -> (Vm, Rc<RefCell<String>>) {
        let file = crate::parse_file("pkg:/source/main.brs", source).unwrap();
        let program = crate::compiler::compile_program(vec![file]).unwrap();
        let output = Rc::new(RefCell::new(String::new()));
        let mut vm = Vm::new(
            program,
            Box::new(TestHost(output.clone())),
            ChannelInfo::default(),
        );
        vm.start("Main", vec![]).unwrap();
        (vm, output)
    }

    #[test]
    fn stop_preserves_locals_and_continue_finishes_once() {
        let (mut vm, output) = vm("sub Main()\n x = 41\n stop\n print x + 1\nend sub");
        assert_eq!(vm.run(1000), Status::Stopped);
        assert_eq!(vm.run(1000), Status::Stopped);
        vm.debugger_command("var");
        vm.debugger_command("bt");
        vm.debugger_command("threads");
        let text = output.borrow().clone();
        assert!(text.contains("Integer val:41"));
        assert!(text.contains("pkg:/source/main.brs(3)"));
        assert!(text.contains(" 0* "));
        assert!(vm.debugger_command("c"));
        assert_eq!(vm.run(1000), Status::Finished);
        assert_eq!(output.borrow().matches(" 42\n").count(), 1);
    }

    #[test]
    fn step_into_over_and_out_follow_source_lines() {
        let source = "sub Main()\n stop\n answer = Twice(21)\n print answer\nend sub\nfunction Twice(x)\n y = x * 2\n return y\nend function";
        for (command, expected) in [("step", "Twice"), ("over", "Main")] {
            let (mut vm, _) = vm(source);
            assert_eq!(vm.run(1000), Status::Stopped);
            vm.debugger_command("step");
            assert_eq!(vm.run(1000), Status::Stopped); // before the call
            vm.debugger_command(command);
            assert_eq!(vm.run(1000), Status::Stopped);
            let pos = vm.debug_location.unwrap();
            assert_eq!(vm.program.functions[pos.function as usize].name, expected);
            if command == "step" {
                vm.debugger_command("out");
                assert_eq!(vm.run(1000), Status::Stopped);
                assert_eq!(vm.debug_location.unwrap().depth, 1);
            }
            vm.debugger_command("c");
            assert_eq!(vm.run(1000), Status::Finished);
        }
    }

    #[test]
    fn task_stop_freezes_scheduler_and_can_resume() {
        let (mut vm, output) = vm("sub Main()\n port = CreateObject(\"roMessagePort\")\n wait(0, port)\nend sub\nsub Work()\n stop\n print \"task done\"\nend sub");
        let function = vm.program.lookup(0, "Work").unwrap();
        let node = crate::sg::runtime::create_node(&mut vm, "Task")
            .unwrap()
            .unwrap();
        vm.spawn_task(function, vm.global_m.clone(), node);
        assert_eq!(vm.run(1000), Status::Stopped);
        assert_eq!(vm.debug_thread, Some(2));
        assert_eq!(vm.run(1000), Status::Stopped);
        assert!(!output.borrow().contains("task done\n"));
        vm.debugger_command("c");
        assert_eq!(vm.run(1000), Status::Waiting);
        assert_eq!(output.borrow().matches("task done\n").count(), 1);
    }

    #[test]
    fn continuing_an_interrupted_wait_keeps_it_blocked() {
        let (mut vm, output) = vm("sub Main()\n port = CreateObject(\"roMessagePort\")\n msg = wait(0, port)\n print \"unexpected\"\nend sub");
        assert_eq!(vm.run(1000), Status::Waiting);
        vm.debugger_command("break");
        assert_eq!(vm.run(1000), Status::Stopped);
        vm.debugger_command("c");
        assert_eq!(vm.run(1000), Status::Waiting);
        assert!(!output.borrow().contains("unexpected\n"));
    }

    #[test]
    fn debugger_break_can_interrupt_a_running_loop() {
        let (mut vm, output) = vm("sub Main()\n n = 0\n while true\n n++\n end while\nend sub");
        assert_eq!(vm.run(100), Status::Running);
        vm.debugger_command("break");
        assert_eq!(vm.run(100), Status::Stopped);
        vm.debugger_command("print n");
        assert!(!output.borrow().contains("Unsupported"));
        vm.debugger_command("not-a-command");
        assert_eq!(vm.run(100), Status::Stopped);
        vm.debugger_command("c");
        assert_eq!(vm.run(100), Status::Running);
    }
}
