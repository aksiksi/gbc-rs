use std::fs::File;
use std::io::Write;

use crate::cpu::Cpu;
use crate::instructions::Instruction;
use crate::memory::{MemoryRead, MemoryWrite};

const DEBUG_DUMP_FILE: &str = "dump.txt";

pub enum Mode {
    Step,
    StepN(u32),
    Continue,
}

pub struct Debugger {
    mode: Mode,
    checks: u32,
    steps: u32,
    breakpoints: Vec<(u16, bool)>,
    instructions: Vec<(Instruction, u16)>,
    instruction_dump: Option<File>,
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            mode: Mode::Step,
            steps: 0,
            checks: 0,
            breakpoints: Vec::new(),
            instructions: Vec::new(),
            instruction_dump: None,
        }
    }

    pub fn triggered(&mut self, cpu: &Cpu) -> bool {
        // If the CPU is currently halted, keep waiting
        if cpu.is_halted {
            return false;
        }

        let pc = cpu.registers.PC;

        // Keep track of each instruction the CPU executes
        let (inst, _, _) = cpu.fetch(None);
        self.instructions.push((inst, pc));

        // Dump each instruction to a file
        if let Some(f) = self.instruction_dump.as_mut() {
            write!(f, "{:#06x}: {}\n", pc, inst).unwrap();
        }

        self.checks += 1;

        let res = match self.mode {
            Mode::Step => true,
            Mode::StepN(n) => {
                if self.checks - self.steps == n {
                    self.steps = self.checks - 1;
                    true
                } else {
                    false
                }
            }
            Mode::Continue => {
                let mut breakpoint_hit = false;
                for (addr, enabled) in &self.breakpoints {
                    if *enabled && pc == *addr {
                        self.steps = self.checks - 1;
                        breakpoint_hit = true;
                    }
                }

                breakpoint_hit
            }
        };

        if res {
            // When a breakpoint is hit, print the last executed instruction
            if self.instructions.len() > 1 {
                println!("{}", self.instructions[self.instructions.len()-2].0);
            }
        }

        res
    }

    fn parse_u16(input: &str) -> Option<u16> {
        let addr: Option<u16>;

        if input.contains("0x") | input.contains("0X") {
            addr = u16::from_str_radix(&input[2..], 16).ok();
        } else {
            addr = u16::from_str_radix(input, 10).ok();
        }

        addr
    }

    pub fn repl(&mut self, cpu: &mut Cpu) {
        self.steps += 1;

        loop {
            print!("gbcdbg> ");
            std::io::stdout().flush().unwrap();

            let mut line = String::new();
            std::io::stdin().read_line(&mut line).unwrap();

            let line: Vec<&str> = line.trim().split(" ").collect();

            match line[0] {
                "" => (),
                "q" | "quit" => {
                    std::process::exit(0);
                }
                "b" if line.len() == 2 => {
                    let addr = match Self::parse_u16(line[1]) {
                        Some(v) => v,
                        None => {
                            eprintln!("Invalid address specified: {}", line[1]);
                            continue;
                        }
                    };

                    let mut found = false;
                    for (other, exists) in self.breakpoints.iter_mut() {
                        if *other == addr {
                            *exists = true;
                            found = true;
                        }
                    }

                    if !found {
                        self.breakpoints.push((addr, true));
                    }
                }
                "b" => eprintln!("'b' requires at least 1 argument"),
                "d" if line.len() == 2 => {
                    // Delete a breakpoint
                    let index: usize = line[1].parse().unwrap();
                    if index >= self.breakpoints.len() {
                        eprintln!("Invalid breakpoint {}", index);
                        continue;
                    }

                    self.breakpoints.remove(index);
                }
                "d" => eprintln!("'d' requires at least 1 argument"),
                "toggle" if line.len() == 2 => {
                    // Toggle a breakpoint
                    let index: usize = line[1].parse().unwrap();
                    if index >= self.breakpoints.len() {
                        eprintln!("Invalid breakpoint {}", index);
                        continue;
                    }

                    self.breakpoints[index].1 = !self.breakpoints[index].1;
                }
                "toggle" => eprintln!("'toggle' requires at least 1 argument"),
                "dump" if line.len() == 2 => {
                    let flag: u32 = line[1].parse().unwrap();
                    if flag == 0 {
                        let _ = self.instruction_dump.take();
                        println!("Disabled instruction dumping");
                    } else {
                        self.instruction_dump = Some(File::create(DEBUG_DUMP_FILE).unwrap());
                        println!("Dumping instructions to {}", DEBUG_DUMP_FILE);
                    }
                }
                "h" | "hist" => {
                    let count: usize = if line.len() < 2 {
                        5
                    } else {
                        line[1].parse().unwrap()
                    };

                    let total = self.instructions.len();
                    if total < 2 {
                        continue;
                    }

                    // Print the last 5 instructions we've hit
                    let range = if total < count {
                        0..total-1
                    } else {
                        total-count-1..total-1
                    };

                    for (inst, pc) in self.instructions[range].iter() {
                        println!("{:#06x}: {}", pc, inst);
                    }
                }
                "count" => {
                    println!("{}", self.instructions.len());
                }
                "reset" => {
                    // Reset the CPU
                    cpu.reset().unwrap();
                    self.checks = 0;
                    self.steps = 0;
                    self.instructions.clear();
                    println!("CPU reset");
                }
                "l" | "list" => {
                    // Number of instructions to disassemble, startng from address below
                    let count: usize = if line.len() >= 2 {
                        line[1].parse().unwrap()
                    } else {
                        5
                    };

                    // Start address - defaults to PC
                    let addr = if line.len() == 3 {
                        Self::parse_u16(line[2])
                    } else {
                        Some(cpu.registers.PC)
                    };

                    for (inst, addr) in cpu.disassemble(count, addr) {
                        println!("{:#06x}: {}", addr, inst);
                    }
                }
                "n" if line.len() == 2 => {
                    let n: u32 = line[1].parse().unwrap();
                    self.mode = Mode::StepN(n);
                    return;
                }
                "n" => {
                    self.mode = Mode::Step;
                    return;
                }
                "r" => {
                    self.mode = Mode::Continue;
                    return;
                }
                "p" if line.len() == 2 => {
                    let addr = match Self::parse_u16(line[1]) {
                        Some(v) => v,
                        None => {
                            eprintln!("Invalid address specified: {}", line[1]);
                            continue;
                        }
                    };

                    let value = cpu.memory.read(addr);

                    println!("{:#X}", value);
                }
                "p" => eprintln!("'p' requires at least 1 argument"),
                "w" if line.len() == 3 => {
                    let addr = match Self::parse_u16(line[1]) {
                        Some(v) => v,
                        None => {
                            eprintln!("Invalid address specified: {}", line[1]);
                            continue;
                        }
                    };

                    let value = Self::parse_u16(line[2]).unwrap();

                    cpu.memory.write(addr, value);
                }
                "w" => eprintln!("'w' requires at least 2 arguments"),
                "info" if line.len() == 2 => {
                    match line[1] {
                        "r" | "reg" | "registers" => {
                            println!("{}", cpu.registers);
                        }
                        "b" | "break" | "breakpoints" => {
                            let mut i = 0;
                            for (addr, enabled) in &self.breakpoints {
                                println!("{}: addr = {:#06X}, enabled = {}", i, addr, enabled);
                                i += 1;
                            }
                        }
                        unknown => eprintln!("Unknown option for 'info': {}", unknown),
                    }
                }
                "info" => eprintln!("'info' requires at least 1 argument"),
                unknown => eprintln!("Unknown command: {}", unknown),
            }
        }
    }
}
