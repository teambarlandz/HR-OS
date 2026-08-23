#!/usr/bin/env python3
"""Grant RWE to the EXEC_BUFFER PT_LOAD segment of the RISC-V kernel ELF.

NOT needed for QEMU (softmmu executes RW RAM regardless of segment flags).
Needed only for real SiFive E310 / PMP-enforcing silicon bring-up.

Usage: patch-riscv-x.py [elf]
  default elf: target/riscv32imac-unknown-none-elf/release/holy-rust
"""
import struct
import sys
import pathlib

EXEC_BUFFER_VADDR = 0x80001000  # DTIM carve (linker/memory-riscv.x)


def main() -> None:
    elf = pathlib.Path(
        sys.argv[1] if len(sys.argv) > 1
        else "target/riscv32imac-unknown-none-elf/release/holy-rust"
    )
    data = bytearray(elf.read_bytes())

    e_phoff = struct.unpack_from("<I", data, 0x1C)[0]
    e_phentsize = struct.unpack_from("<H", data, 0x2A)[0]
    e_phnum = struct.unpack_from("<H", data, 0x2C)[0]

    hits = 0
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type = struct.unpack_from("<I", data, off)[0]
        p_vaddr = struct.unpack_from("<I", data, off + 8)[0]
        if p_type == 1 and p_vaddr == EXEC_BUFFER_VADDR:  # PT_LOAD
            flags_off = off + 24
            cur = struct.unpack_from("<I", data, flags_off)[0]
            if cur == 7:
                print(f"already RWE: {elf}")
                return
            if cur != 6:
                sys.exit(f"unexpected p_flags={cur} (expected RW=6); refusing")
            struct.pack_into("<I", data, flags_off, 7)
            print(f"patched PHDR {i} (vaddr 0x{p_vaddr:08X}): RW -> RWE in {elf}")
            hits += 1

    if hits == 0:
        sys.exit(f"EXEC_BUFFER segment 0x{EXEC_BUFFER_VADDR:08X} not found in {elf}")
    elf.write_bytes(data)


if __name__ == "__main__":
    main()
