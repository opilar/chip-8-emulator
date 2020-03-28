use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use super::{
    memory::{Memory, SPRITE_SIZE, SPRITE_START_LOCATION},
    registers::Registers,
    graphics::Graphics,
    stack::Stack,
    input::Input,
};

pub struct VM {
    memory: Memory,
    registers: Registers,
    stack: Stack,
    graphics: Graphics,
    input: Input,
    rng: SmallRng,
}

impl VM {
    pub fn new() -> VM {
        Self {
            memory: Memory::new_with_initial_sprites(),
            registers: Registers::new(),
            stack: Stack::new(),
            graphics: Graphics::new(),
            input: Input::new(),
            rng: SmallRng::seed_from_u64(0),
        }
    }

    /// Return from a subroutine.
    ///
    /// Code: `00EE`
    ///
    /// The interpreter sets the program counter to the address at the top of
    /// the stack, then subtracts 1 from the stack pointer.
    fn ret(&mut self) {
        self.registers.program_counter = self.stack.pop();
    }

    /// Jump to a machine code routine at `addr`.
    ///
    /// Code: `0nnn`
    ///
    /// This instruction is only used on the old computers on which Chip-8 was
    /// originally implemented. It is ignored by modern interpreters.
    fn jp(&mut self, addr: u16) {
        assert!((addr & 0xF000) == 0);
        self.registers.program_counter = addr;
    }

    /// Clear the display.
    ///
    /// Code: `00E0`
    fn cls(&mut self) {
        self.graphics.clear();
        self.registers.program_counter += 1;
    }

    /// Call subroutine at `addr`.
    ///
    /// Code: `2nnn`
    ///
    /// The interpreter increments the stack pointer, then puts the current
    /// program counter on the top of the stack. The program counter is then
    /// set to `addr`.
    fn call(&mut self, addr: u16) {
        assert!((addr & 0xF000) == 0);

        self.stack.push(self.registers.program_counter);
        self.registers.program_counter = addr;
    }

    /// Skip next instruction if `Vx` = `value`.
    ///
    /// Code: `3xkk`
    ///
    /// The interpreter compares register `Vx` to `value`, and if they are
    /// equal, increments the program counter by 2.
    fn se(&mut self, x: u8, value: u8) {
        if self.registers.v[x as usize] == value {
            self.registers.program_counter += 2;
        } else {
            self.registers.program_counter += 1;
        }
    }

    /// Skip next instruction if `Vx` != `value`.
    ///
    /// Code: `4xkk`
    ///
    /// The interpreter compares register `Vx` to `value`, and if they are not
    /// equal, increments the program counter by 2.
    fn sne(&mut self, x: u8, value: u8) {
        if self.registers.v[x as usize] != value {
            self.registers.program_counter += 2;
        } else {
            self.registers.program_counter += 1;
        }
    }

    /// Skip next instruction if `Vx` = `Vy`.
    ///
    /// Code: `5xy0`
    ///
    /// The interpreter compares register `Vx` to register `Vy`, and if they
    /// are equal, increments the program counter by 2.
    fn se_v(&mut self, x: u8, y: u8) {
        if self.registers.v[x as usize] == self.registers.v[y as usize] {
            self.registers.program_counter += 2;
        } else {
            self.registers.program_counter += 1;
        }
    }

    /// Set `Vx` = `value`.
    ///
    /// Code: `6xkk`
    ///
    /// The interpreter puts the value `value` into register `Vx`.
    fn ld_vx(&mut self, x: u8, value: u8) {
        self.registers.v[x as usize] = value;
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vx` + `value`.
    ///
    /// Code: `7xkk`
    ///
    /// Adds the value `value` to the value of register `Vx`, then stores the
    /// result in `Vx`.
    fn add_vx(&mut self, x: u8, value: u8) {
        let result = self.registers.v[x as usize].wrapping_add(value);
        self.registers.v[x as usize] = result;
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vy`.
    ///
    /// Code: `8xy0`
    ///
    /// Stores the value of register `Vy` in register `Vx`.
    fn ld_vx_vy(&mut self, x: u8, y: u8) {
        self.registers.v[x as usize] = self.registers.v[y as usize];
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vx` OR `Vy`.
    ///
    /// Code: `8xy1`
    ///
    /// Performs a bitwise OR on the values of `Vx` and `Vy`, then stores the
    /// result in `Vx`. A bitwise OR compares the corrseponding bits from two
    /// values, and if either bit is 1, then the same bit in the result is also
    /// 1. Otherwise, it is 0.
    fn or(&mut self, vx: u8, vy: u8) {
        self.registers.v[vx as usize] |= self.registers.v[vy as usize];
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vx` AND `Vy`.
    ///
    /// Code: `8xy2`
    ///
    /// Performs a bitwise AND on the values of `Vx` and `Vy`, then stores the
    /// result in `Vx`. A bitwise AND compares the corrseponding bits from two
    /// values, and if both bits are 1, then the same bit in the result is also
    /// 1. Otherwise, it is 0.
    fn and(&mut self, vx: u8, vy: u8) {
        self.registers.v[vx as usize] &= self.registers.v[vy as usize];
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vx` XOR `Vy`.
    ///
    /// Code: `8xy3`
    ///
    /// Performs a bitwise exclusive OR on the values of `Vx` and `Vy`, then
    /// stores the result in `Vx`. An exclusive OR compares the corrseponding
    /// bits from two values, and if the bits are not both the same, then the
    /// corresponding bit in the result is set to 1. Otherwise, it is 0.
    fn xor(&mut self, vx: u8, vy: u8) {
        self.registers.v[vx as usize] ^= self.registers.v[vy as usize];
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vx` + `Vy`, set `VF` = carry.
    ///
    /// Code: `8xy4`
    ///
    /// The values of `Vx` and `Vy` are added together. If the result is greater
    /// than 8 bits (i.e., > 255,) `VF` is set to 1, otherwise 0. Only the
    /// lowest 8 bits of the result are kept, and stored in `Vx`.
    fn add_vx_vy(&mut self, x: u8, y: u8) {
        let (result, is_overflow) = self.registers.v[x as usize]
            .overflowing_add(self.registers.v[y as usize]);
        self.registers.v[x as usize] = result;
        self.registers.v[0xF] = if is_overflow { 1 } else { 0 };
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vx` - `Vy`, set `VF` = NOT borrow.
    ///
    /// Code: `8xy5`
    ///
    /// If `Vx` > `Vy`, then `VF` is set to 1, otherwise 0. Then `Vy` is
    /// subtracted from `Vx`, and the results stored in `Vx`.
    fn sub(&mut self, x: u8, y: u8) {
        let (result, is_overflow) = self.registers.v[x as usize]
            .overflowing_sub(self.registers.v[y as usize]);
        self.registers.v[x as usize] = result;
        self.registers.v[0xF] = if is_overflow { 1 } else { 0 }; // FIXME
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vx` SHR 1.
    ///
    /// Code: `8xy6`
    ///
    /// If the least-significant bit of `Vx` is 1, then `VF` is set to 1,
    /// otherwise 0. Then `Vx` is divided by 2.
    fn shr(&mut self, vx: u8) {
        self.registers.v[0xF] = self.registers.v[vx as usize] % 2;
        self.registers.v[vx as usize] >>= 1;
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vy` - `Vx`, set `VF` = NOT borrow.
    ///
    /// Code: `8xy7`
    ///
    /// If `Vy` > `Vx`, then `VF` is set to 1, otherwise 0. Then `Vx` is
    /// subtracted from `Vy`, and the results stored in `Vx`.
    fn subn(&mut self, x: u8, y: u8) {
        let (result, is_overflow) = self.registers.v[y as usize]
            .overflowing_sub(self.registers.v[x as usize]);
        self.registers.v[x as usize] = result;
        self.registers.v[0xF] = if is_overflow { 1 } else { 0 }; // FIXME
        self.registers.program_counter += 1;
    }

    /// Set `Vx` = `Vx` SHL 1.
    ///
    /// Code: `8xyE`
    ///
    /// If the most-significant bit of `Vx` is 1, then `VF` is set to 1,
    /// otherwise to 0. Then `Vx` is multiplied by 2.
    fn shl(&mut self, vx: u8) {
        let significant_bit = self.registers.v[vx as usize] >= 0b10000000;
        self.registers.v[0xF] = if significant_bit { 1 } else { 0 };
        self.registers.v[vx as usize] <<= 1;
        self.registers.program_counter += 1;
    }

    /// Set `I` = `value`.
    ///
    /// Code: `Annn`
    ///
    /// The value of register `I` is set to `value`.
    fn ld_i(&mut self, value: u16) {
        assert!((value & 0xF000) == 0);
        self.registers.i = value;
        self.registers.program_counter += 1;
    }

    /// Jump to location `addr` + `V0`.
    ///
    /// Code: `Bnnn`
    ///
    /// The program counter is set to `addr` plus the value of `V0`.
    fn jp_v0(&mut self, addr: u16) {
        assert!((addr & 0xF000) == 0);
        self.registers.program_counter = addr + (self.registers.v[0] as u16);
    }

    /// Set `Vx` = random byte AND `mask`.
    ///
    /// Code: `Cxkk`
    ///
    /// The interpreter generates a random number from 0 to 255, which is then
    /// ANDed with the value `mask`. The results are stored in `Vx`. See
    /// instruction `8xy2` for more information on AND.
    fn rnd(&mut self, vx: u8, mask: u8) {
        let value = self.rng.gen::<u8>() & mask;
        self.registers.v[vx as usize] = value;
        self.registers.program_counter += 1;
    }

    /// Display `n`-byte sprite starting at memory location `I` at (`Vx`, `Vy`),
    /// set `VF` = collision.
    ///
    /// Code: `Dxyn`
    ///
    /// The interpreter reads `n` bytes from memory, starting at the address
    /// stored in `I`. These bytes are then displayed as sprites on screen at
    /// coordinates (`Vx`, `Vy`). Sprites are XORed onto the existing screen.
    /// If this causes any pixels to be erased, `VF` is set to 1, otherwise it
    /// is set to 0. If the sprite is positioned so part of it is outside the
    /// coordinates of the display, it wraps around to the opposite side of the
    /// screen. See instruction `8xy3` for more information on XOR, and section
    /// Display for more information on the Chip-8 screen and sprites.
    fn drw(&mut self, vx: u8, vy: u8, n: u8) {
        let sprite_start = self.registers.i as usize;
        let sprite_end = sprite_start + n as usize;
        let sprite = self.memory.get_slice(sprite_start, sprite_end);

        let is_collision = self.graphics.draw_sprite(vx as usize, vy as usize, sprite);

        self.registers.v[0xF] = if is_collision { 1 } else { 0 };
        self.registers.program_counter += 1;
    }

    /// Skip next instruction if key with the value of `Vx` is pressed.
    ///
    /// Code: `Ex9E`
    ///
    /// Checks the keyboard, and if the key corresponding to the value of `Vx`
    /// is currently in the down position, program counter is increased by 2.
    fn skp(&mut self, x: u8) {
        let increment_value = if self.input.is_key_pressed(x) { 2 } else { 1 };
        self.registers.program_counter += increment_value;
    }

    /// Skip next instruction if key with the value of `Vx` is not pressed.
    ///
    /// Code: `ExA1`
    ///
    /// Checks the keyboard, and if the key corresponding to the value of `Vx`
    /// is currently in the up position, program counter is increased by 2.
    fn sknp(&mut self, x: u8) {
        self.registers.program_counter += if self.input.is_key_pressed(x) { 1 } else { 2 };
    }

    /// Set `Vx` = delay timer value.
    ///
    /// Code: `Fx07`
    ///
    /// The value of delay timer is placed into `Vx`.
    fn ld_vx_dt(&mut self, x: u8) {
        self.registers.v[x as usize] = self.registers.delay_timer;
        self.registers.program_counter += 1;
    }

    /// Set delay timer = `Vx`.
    ///
    /// Code: `Fx15`
    ///
    /// Delay timer is set equal to the value of `Vx`.
    fn ld_dt_vx(&mut self, x: u8) {
        self.registers.delay_timer = self.registers.v[x as usize];
        self.registers.program_counter += 1;
    }

    /// Wait for a key press, store the value of the key in `Vx`.
    ///
    /// Code: `Fx0A`
    ///
    /// All execution stops until a key is pressed, then the value of that key
    /// is stored in `Vx`.
    fn ld_vx_k(&mut self, x: u8) {
        // TODO: implement
        unimplemented!();
    }

    /// Set sound timer = `Vx`.
    ///
    /// Code: `Fx18`
    ///
    /// Sound timer is set equal to the value of `Vx`.
    fn ld_st(&mut self, x: u8) {
        self.registers.sound_timer = self.registers.v[x as usize];
        self.registers.program_counter += 1;
    }

    /// Set `I` = `I` + `Vx`.
    ///
    /// Code: `Fx1E`
    ///
    /// The values of `I` and `Vx` are added, and the results are stored in `I`.
    fn add_i(&mut self, x: u8) {
        self.registers.i += self.registers.v[x as usize] as u16;
        self.registers.program_counter += 1;
    }

    /// Set `I` = location of sprite for digit `Vx`.
    ///
    /// Code: `Fx29`
    ///
    /// The value of `I` is set to the location for the hexadecimal sprite
    /// corresponding to the value of `Vx`. See section Display for more
    /// information on the Chip-8 hexadecimal font.
    fn ld_f(&mut self, x: u8) {
        let sprite_num = self.registers.v[x as usize] as usize;
        let sprite_location = SPRITE_START_LOCATION + (sprite_num * SPRITE_SIZE);
        self.registers.i = sprite_location as u16;
        self.registers.program_counter += 1;
    }

    /// Store BCD representation of `Vx` in memory locations `I`, `I+1`, and
    /// `I+2`.
    ///
    /// Code: `Fx33`
    ///
    /// The interpreter takes the decimal value of `Vx`, and places the
    /// hundreds digit in memory at location in `I`, the tens digit at location
    /// `I+1`, and the ones digit at location `I+2`.
    fn ld_b(&mut self, x: u8) {
        let number = self.registers.v[x as usize];
        let ones = number % 10;
        let tens = number / 10 % 10;
        let hundreds = number / 100;

        let start_pos = self.registers.i as usize;
        let slice = self.memory.get_slice_mut(start_pos, start_pos + 3);
        slice[0] = hundreds;
        slice[1] = tens;
        slice[2] = ones;
        self.registers.program_counter += 1;
    }

    fn ld_i_vx(&mut self, x: u8) {
        let registers = &self.registers.v[0..=x as usize];
        let start_memory_pos = self.registers.i as usize;
        let finis_memory_pos = start_memory_pos + registers.len();
        let memory = self.memory.get_slice_mut(start_memory_pos, finis_memory_pos);

        memory.copy_from_slice(registers);

        self.registers.program_counter += 1;
    }

    fn ld_vx_i(&mut self, x: u8) {
        let registers = &mut self.registers.v[0..=x as usize];
        let start_memory_pos = self.registers.i as usize;
        let finis_memory_pos = start_memory_pos + registers.len();
        let memory = self.memory.get_slice(start_memory_pos, finis_memory_pos);

        registers.copy_from_slice(memory);

        self.registers.program_counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::u64;
    use super::super::graphics::DISPLAY_ROWS;

    #[test]
    fn test_jp() {
        let mut vm = VM::new();
        let addr = 16u16;

        vm.jp(addr);

        assert_eq!(vm.registers.program_counter, addr);
    }

    #[test]
    fn test_jp_edge_case() {
        let mut vm = VM::new();
        let addr = 0x0FFF;

        vm.jp(addr);

        assert_eq!(vm.registers.program_counter, addr);
    }

    #[test]
    #[should_panic]
    fn test_jp_incorrect_addr() {
        let mut vm = VM::new();
        vm.jp(0x1000);
    }

    #[test]
    fn test_cls() {
        let mut vm = VM::new();
        vm.graphics.display = [u64::MAX; DISPLAY_ROWS];
        assert_eq!(vm.registers.program_counter, 0);

        vm.cls();

        assert!(vm.graphics.display.iter().all(|&row| row == 0));
        assert_eq!(vm.registers.program_counter, 1);
    }

    #[test]
    fn test_ret() {
        let mut vm = VM::new();
        vm.registers.program_counter = 1;
        vm.stack.push(2);
        vm.stack.push(3);

        vm.ret();

        assert_eq!(vm.registers.program_counter, 3);
        assert_eq!(vm.stack.pointer, 1);
        assert_eq!(vm.stack.stack[0], 2);
    }

    #[test]
    fn test_call() {
        let mut vm = VM::new();
        vm.registers.program_counter = 1;
        vm.stack.push(2);
        vm.stack.push(3);

        vm.call(4);

        assert_eq!(vm.registers.program_counter, 4);
        assert_eq!(vm.stack.pointer, 3);
        assert_eq!(vm.stack.stack[0], 2);
        assert_eq!(vm.stack.stack[1], 3);
        assert_eq!(vm.stack.stack[2], 1);
    }

    #[test]
    #[should_panic]
    fn test_call_invalid_addr() {
        let mut vm = VM::new();
        vm.call(0x1111);
    }

    #[test]
    #[should_panic]
    fn test_call_invalid_addr_edge_case() {
        let mut vm = VM::new();
        vm.call(0x1000);
    }

    #[test]
    fn test_se_equal() {
        let mut vm = VM::new();
        vm.registers.v[1] = 1;
        vm.registers.program_counter = 5;

        vm.se(1, 1);

        assert_eq!(vm.registers.v[1], 1);
        assert_eq!(vm.registers.program_counter, 7);
    }

    #[test]
    fn test_se_not_equal() {
        let mut vm = VM::new();
        vm.registers.v[1] = 1;
        vm.registers.program_counter = 5;

        vm.se(1, 2);

        assert_eq!(vm.registers.v[1], 1);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_se_invalid() {
        let mut vm = VM::new();
        vm.sne(16, 1);
    }

    #[test]
    fn test_sne_equal() {
        let mut vm = VM::new();
        vm.registers.v[1] = 1;
        vm.registers.program_counter = 5;

        vm.sne(1, 1);

        assert_eq!(vm.registers.v[1], 1);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_sne_not_equal() {
        let mut vm = VM::new();
        vm.registers.v[1] = 1;
        vm.registers.program_counter = 5;

        vm.sne(1, 2);

        assert_eq!(vm.registers.v[1], 1);
        assert_eq!(vm.registers.program_counter, 7);
    }

    #[test]
    #[should_panic]
    fn test_sne_invalid() {
        let mut vm = VM::new();
        vm.sne(16, 1);
    }

    #[test]
    fn test_se_v_equal() {
        let mut vm = VM::new();
        vm.registers.v[1] = 4;
        vm.registers.v[2] = 4;
        vm.registers.program_counter = 5;

        vm.se_v(1, 2);

        assert_eq!(vm.registers.v[1], 4);
        assert_eq!(vm.registers.v[2], 4);
        assert_eq!(vm.registers.program_counter, 7);
    }

    #[test]
    fn test_se_v_not_equal() {
        let mut vm = VM::new();
        vm.registers.v[1] = 4;
        vm.registers.v[2] = 5;
        vm.registers.program_counter = 5;

        vm.se_v(1, 2);

        assert_eq!(vm.registers.v[1], 4);
        assert_eq!(vm.registers.v[2], 5);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_se_v_invalid_first() {
        let mut vm = VM::new();
        vm.se_v(16, 1);
    }

    #[test]
    #[should_panic]
    fn test_se_v_invalid_second() {
        let mut vm = VM::new();
        vm.se_v(0, 16);
    }

    #[test]
    fn test_ld_vx() {
        let mut vm = VM::new();
        vm.registers.v[1] = 4;
        vm.registers.program_counter = 5;

        vm.ld_vx(1, 2);

        assert_eq!(vm.registers.v[1], 2);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_ld_vx_invalid() {
        let mut vm = VM::new();
        vm.ld_vx(16, 1);
    }

    #[test]
    fn test_add_vx() {
        let mut vm = VM::new();
        vm.registers.v[1] = 200;
        vm.registers.program_counter = 5;

        vm.add_vx(1, 50);

        assert_eq!(vm.registers.v[1], 250);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_add_vx_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 255;
        vm.registers.program_counter = 5;

        vm.add_vx(1, 1);

        assert_eq!(vm.registers.v[1], 0);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_ld_vx_vy() {
        let mut vm = VM::new();
        vm.registers.v[1] = 4;
        vm.registers.v[2] = 7;
        vm.registers.program_counter = 5;

        vm.ld_vx_vy(1, 2);

        assert_eq!(vm.registers.v[1], 7);
        assert_eq!(vm.registers.v[2], 7);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_ld_vx_vy_invalid_x() {
        let mut vm = VM::new();
        vm.ld_vx_vy(16, 1);
    }

    #[test]
    #[should_panic]
    fn test_ld_vx_vy_invalid_y() {
        let mut vm = VM::new();
        vm.ld_vx_vy(1, 16);
    }

    #[test]
    fn test_or() {
        let mut vm = VM::new();
        vm.registers.v[1] = 0xF0;
        vm.registers.v[2] = 0x0F;
        vm.registers.program_counter = 5;

        vm.or(1, 2);

        assert_eq!(vm.registers.v[1], 0xFF);
        assert_eq!(vm.registers.v[2], 0x0F);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_or_invalid_first() {
        let mut vm = VM::new();
        vm.or(16, 1);
    }

    #[test]
    #[should_panic]
    fn test_or_invalid_second() {
        let mut vm = VM::new();
        vm.or(0, 16);
    }

    #[test]
    fn test_and() {
        let mut vm = VM::new();
        vm.registers.v[1] = 0b0101;
        vm.registers.v[2] = 0b1110;
        vm.registers.program_counter = 5;

        vm.and(1, 2);

        assert_eq!(vm.registers.v[1], 0b0100);
        assert_eq!(vm.registers.v[2], 0b1110);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_and_invalid_first() {
        let mut vm = VM::new();
        vm.and(16, 1);
    }

    #[test]
    #[should_panic]
    fn test_and_invalid_second() {
        let mut vm = VM::new();
        vm.and(0, 16);
    }

    #[test]
    fn test_xor() {
        let mut vm = VM::new();
        vm.registers.v[1] = 0b0100;
        vm.registers.v[2] = 0b1110;
        vm.registers.program_counter = 5;

        vm.xor(1, 2);

        assert_eq!(vm.registers.v[1], 0b1010);
        assert_eq!(vm.registers.v[2], 0b1110);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_xor_invalid_first() {
        let mut vm = VM::new();
        vm.xor(16, 1);
    }

    #[test]
    #[should_panic]
    fn test_xor_invalid_second() {
        let mut vm = VM::new();
        vm.xor(0, 16);
    }

    #[test]
    fn test_add_vx_vy_with_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 200;
        vm.registers.v[2] = 100;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.add_vx_vy(1, 2);

        assert_eq!(vm.registers.v[1], 44);
        assert_eq!(vm.registers.v[2], 100);
        assert_eq!(vm.registers.v[0xF], 1);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_add_vx_vy_without_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 50;
        vm.registers.v[2] = 100;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.add_vx_vy(1, 2);

        assert_eq!(vm.registers.v[1], 150);
        assert_eq!(vm.registers.v[2], 100);
        assert_eq!(vm.registers.v[0xF], 0);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_add_vx_vy_invalid_first() {
        let mut vm = VM::new();
        vm.add_vx_vy(16, 1);
    }

    #[test]
    #[should_panic]
    fn test_add_vx_vy_invalid_second() {
        let mut vm = VM::new();
        vm.add_vx_vy(0, 16);
    }

    #[test]
    fn test_sub_with_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 100;
        vm.registers.v[2] = 200;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.sub(1, 2);

        assert_eq!(vm.registers.v[1], 156);
        assert_eq!(vm.registers.v[2], 200);
        assert_eq!(vm.registers.v[0xF], 1);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_sub_without_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 150;
        vm.registers.v[2] = 100;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.sub(1, 2);

        assert_eq!(vm.registers.v[1], 50);
        assert_eq!(vm.registers.v[2], 100);
        assert_eq!(vm.registers.v[0xF], 0);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_sub_invalid_first() {
        let mut vm = VM::new();
        vm.sub(16, 1);
    }

    #[test]
    #[should_panic]
    fn test_sub_invalid_second() {
        let mut vm = VM::new();
        vm.sub(0, 16);
    }

    #[test]
    fn test_shr_with_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 0b0101;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.shr(1);

        assert_eq!(vm.registers.v[1], 0b0010);
        assert_eq!(vm.registers.v[0xF], 1);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_shr_without_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 0b1010;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.shr(1);

        assert_eq!(vm.registers.v[1], 0b0101);
        assert_eq!(vm.registers.v[0xF], 0);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_shr_invalid() {
        let mut vm = VM::new();
        vm.shr(16);
    }

    #[test]
    fn test_subn_with_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 200;
        vm.registers.v[2] = 100;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.subn(1, 2);

        assert_eq!(vm.registers.v[1], 156);
        assert_eq!(vm.registers.v[2], 100);
        assert_eq!(vm.registers.v[0xF], 1);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_subn_without_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 100;
        vm.registers.v[2] = 150;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.subn(1, 2);

        assert_eq!(vm.registers.v[1], 50);
        assert_eq!(vm.registers.v[2], 150);
        assert_eq!(vm.registers.v[0xF], 0);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_subn_invalid_first() {
        let mut vm = VM::new();
        vm.subn(16, 1);
    }

    #[test]
    #[should_panic]
    fn test_subn_invalid_second() {
        let mut vm = VM::new();
        vm.subn(0, 16);
    }

    #[test]
    fn test_shl_with_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 0b10101010;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.shl(1);

        assert_eq!(vm.registers.v[1], 0b01010100);
        assert_eq!(vm.registers.v[0xF], 1);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_shl_without_overflow() {
        let mut vm = VM::new();
        vm.registers.v[1] = 0b01101010;
        vm.registers.v[0xF] = 4;
        vm.registers.program_counter = 5;

        vm.shl(1);

        assert_eq!(vm.registers.v[1], 0b11010100);
        assert_eq!(vm.registers.v[0xF], 0);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_shl_invalid() {
        let mut vm = VM::new();
        vm.shr(16);
    }

    #[test]
    fn test_ld_i() {
        let mut vm = VM::new();
        vm.registers.i = 5;
        vm.registers.program_counter = 5;

        vm.ld_i(0x0111);

        assert_eq!(vm.registers.i, 0x0111);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    #[should_panic]
    fn test_ld_i_invalid() {
        let mut vm = VM::new();
        vm.ld_i(0xF000);
    }

    #[test]
    fn test_jp_v0() {
        let mut vm = VM::new();
        vm.registers.program_counter = 100;
        vm.registers.v[0] = 5;

        vm.jp_v0(20);

        assert_eq!(vm.registers.program_counter, 25);
    }

    #[test]
    #[should_panic]
    fn test_jp_v0_invalid() {
        let mut vm = VM::new();
        vm.jp_v0(0xF000);
    }

    #[test]
    fn test_rnd() {
        let mut vm = VM::new();
        vm.rng = SmallRng::seed_from_u64(0xFF);
        vm.registers.program_counter = 5;
        vm.registers.v[1] = 0xAF;

        vm.rnd(1, 0xFF);

        assert_eq!(vm.registers.v[1], 181);
        assert_eq!(vm.registers.program_counter, 6);

        vm.rnd(1, 0x0F);

        assert_eq!(vm.registers.v[1], 5);
        assert_eq!(vm.registers.program_counter, 7);
    }

    #[test]
    #[should_panic]
    fn test_rnd_invalid() {
        let mut vm = VM::new();
        vm.rnd(0xFF, 0);
    }

    #[test]
    fn test_drw() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        let location = 0x100;
        vm.registers.i = location as u16;
        vm.registers.v[0xF] = 2;
        let sprite = [0x20, 0x60, 0x20, 0x20, 0x70];
        vm.memory.get_slice_mut(location, location + sprite.len()).copy_from_slice(&sprite);

        vm.drw(4, 4, 5);

        let screen = [0, 0, 0, 0, 0x200, 0x600, 0x200, 0x200, 0x700, 0];
        assert_eq!(&vm.graphics.display[0..10], &screen);
        assert_eq!(vm.registers.v[0xF], 0);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_drw_collision() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        let location = 0x100;
        vm.registers.i = location as u16;
        vm.registers.v[0xF] = 2;
        let sprite = [0xFF];
        vm.memory.get_slice_mut(location, location + sprite.len()).copy_from_slice(&sprite);
        vm.graphics.display[0] = 0x1;

        vm.drw(0, 0, 1);

        assert_eq!(vm.graphics.display[0], 0xFE);
        assert_eq!(vm.registers.v[0xF], 1);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_skp_key_pressed() {
        let mut vm = VM::new();
        vm.input = Input::new_with_state(0b100);
        vm.registers.program_counter = 5;

        vm.skp(2);

        assert_eq!(vm.registers.program_counter, 7);
    }

    #[test]
    fn test_skp_key_unpressed() {
        let mut vm = VM::new();
        vm.input = Input::new_with_state(0b100);
        vm.registers.program_counter = 5;

        vm.skp(4);

        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_sknp_key_pressed() {
        let mut vm = VM::new();
        vm.input = Input::new_with_state(0b100);
        vm.registers.program_counter = 5;

        vm.sknp(2);

        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_sknp_key_unpressed() {
        let mut vm = VM::new();
        vm.input = Input::new_with_state(0b100);
        vm.registers.program_counter = 5;

        vm.sknp(4);

        assert_eq!(vm.registers.program_counter, 7);
    }

    #[test]
    fn test_ld_vx_dt() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        let value = 0xFA;
        vm.registers.delay_timer = value;

        vm.ld_vx_dt(0x2);

        assert_eq!(vm.registers.v[0x2], value);
        assert_eq!(vm.registers.delay_timer, value);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_ld_dt_vx() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        let value = 0xFA;
        vm.registers.v[0x1] = value;

        vm.ld_dt_vx(0x1);

        assert_eq!(vm.registers.delay_timer, value);
        assert_eq!(vm.registers.v[0x1], value);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_ld_st() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        let sound_timer_value = 0xFA;
        vm.registers.v[0x2] = sound_timer_value;

        vm.ld_st(0x2);

        assert_eq!(vm.registers.v[0x2], sound_timer_value);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_add_i() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        vm.registers.i = 10;
        vm.registers.v[0x2] = 5;

        vm.add_i(0x2);

        assert_eq!(vm.registers.i, 15);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_ld_f() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        vm.registers.i = 10;
        vm.registers.v[0x2] = 5;

        vm.ld_f(0x2);

        assert_eq!(vm.registers.i, 25);
        let sprite_five = [0xF0, 0x80, 0xF0, 0x10, 0xF0];
        let sprite = vm.memory.get_slice(vm.registers.i as usize, vm.registers.i as usize + SPRITE_SIZE);
        assert_eq!(sprite, &sprite_five);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_ld_b() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        vm.registers.v[0x5] = 123;
        vm.registers.i = 100;

        vm.ld_b(0x5);

        assert_eq!(vm.memory.get_slice(100, 103), &[1, 2, 3]);
        assert_eq!(vm.registers.i, 100);
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_ld_i_vx() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        vm.registers.i = 0x100;
        let registers = (0x0..=0xF).collect::<Vec<u8>>();
        vm.registers.v.copy_from_slice(&registers);

        vm.ld_i_vx(0xF);

        assert_eq!(vm.memory.get_slice(0x100, 0x110), registers.as_slice());
        assert_eq!(vm.registers.program_counter, 6);
    }

    #[test]
    fn test_ld_vx_i() {
        let mut vm = VM::new();
        vm.registers.program_counter = 5;
        vm.registers.i = 0x100;
        let memory = (0x0..=0xF).collect::<Vec<u8>>();
        vm.memory.get_slice_mut(0x100, 0x110).copy_from_slice(&memory);

        vm.ld_vx_i(0xF);

        assert_eq!(vm.registers.v, memory.as_slice());
        assert_eq!(vm.registers.program_counter, 6);
    }
}
