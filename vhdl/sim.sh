#! /bin/sh -x
set -e
ghdl -a --std=08 cpu.vhdl tb_cpu.vhdl
ghdl -e --std=08 tb_cpu
ghdl -r --std=08 tb_cpu --wave=tb_cpu.ghw
