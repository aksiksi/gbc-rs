#![allow(dead_code)]

mod cartridge;
mod cpu;
mod error;
mod instructions;
mod memory;
mod registers;

use cartridge::Cartridge;
use cpu::Cpu;
use error::Result;
use memory::MemoryBus;

struct Gameboy {
    cpu: Cpu,
    cartridge: Cartridge,
}

fn main() -> Result<()> {
    let mut cartridge = Cartridge::from_file("samples/pokemon_gold.gbc").unwrap();
    let memory = MemoryBus::from_cartridge(&mut cartridge)?;
    let mut cpu = Cpu::new(memory);
    let _memory = cpu.memory();

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    dbg!(&cpu);

    let _gameboy = Gameboy { cpu, cartridge };

    Ok(())
}
