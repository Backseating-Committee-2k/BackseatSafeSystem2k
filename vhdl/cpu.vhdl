library ieee;
use ieee.std_logic_1164.ALL;
use ieee.numeric_std.ALL;
use ieee.std_logic_misc.or_reduce;

entity cpu is
	generic(
		address_width : integer range 1 to 32 := 32
	);
	port(
		-- async reset
		reset : in std_logic;

		-- clock
		clk : in std_logic;

		-- instruction bus (Avalon-MM)
		i_addr : out std_logic_vector(address_width - 1 downto 0);
		i_rddata : in std_logic_vector(63 downto 0);
		i_rdreq : out std_logic;
		i_waitrequest : in std_logic;

		-- data bus (Avalon-MM)
		d_addr : out std_logic_vector(address_width - 1 downto 0);
		d_rddata : in std_logic_vector(31 downto 0);
		d_rdreq : out std_logic;
		d_wrdata : out std_logic_vector(31 downto 0);
		d_wrreq : out std_logic;
		d_waitrequest : in std_logic;

		-- status
		halted : out std_logic
	);
end entity;

architecture simple of cpu is
	subtype address is std_logic_vector(address_width - 1 downto 0);

	subtype word is std_logic_vector(31 downto 0);

	type state is (ifetch, ifetch2, decode, load, load2, store, store2, halt);

	signal s : state;

	subtype reg is integer range 16#00# to 16#ff#;
	constant sp : reg := 16#fe#;
	constant ip : reg := 16#ff#;

	function to_index(rn : in std_logic_vector(7 downto 0)) return reg is
	begin
		return to_integer(unsigned(rn));
	end function;

	type reg_file is array(reg) of word;

	signal r : reg_file;

	type flags is record
		c : std_logic;
		z : std_logic;
	end record;
	signal f : flags;

	constant reset_ip : word := x"00000000";

	subtype insn is std_logic_vector(63 downto 0);

	-- current instruction
	signal i : insn;

	-- address for memory operation
	signal m_addr : address;
	-- register for memory operation
	signal m_reg : reg;
begin
	halted <= '1' when s = halt else '0';

	process(reset, clk) is
		procedure done is
		begin
			r(ip) <= word(unsigned(r(ip)) + 1);
			s <= ifetch;
		end procedure;

		procedure execute_insn is
			alias opcode : std_logic_vector(15 downto 0) is i(63 downto 48);
			alias reg1 : std_logic_vector(7 downto 0) is i(47 downto 40);
			alias reg2 : std_logic_vector(7 downto 0) is i(39 downto 32);
			alias reg3 : std_logic_vector(7 downto 0) is i(31 downto 24);
			alias reg4 : std_logic_vector(7 downto 0) is i(23 downto 16);
			alias c : std_logic_vector(31 downto 0) is i(31 downto 0);

			variable r1, r2, r3, r4 : reg;

			-- 32 bit wide temporary
			variable tmp32 : std_logic_vector(31 downto 0);

			-- 33 bit wide temporary
			variable tmp33 : std_logic_vector(32 downto 0);

			-- 64 bit wide temporary
			variable tmp64 : std_logic_vector(63 downto 0);
		begin
			r1 := to_index(reg1);
			r2 := to_index(reg2);
			r3 := to_index(reg3);
			r4 := to_index(reg4);

			case opcode is
				when x"0000" =>
					-- LI
					r(r1) <= c;
					done;
				when x"0001" =>
					-- LD abs
					m_addr <= c;
					m_reg <= r1;
					s <= load;
				when x"0002" =>
					-- MOV
					r(r1) <= r(r2);
					done;
				when x"0003" =>
					-- ST abs
					m_addr <= c;
					m_reg <= r1;
					s <= store;
				when x"0004" =>
					-- LD [r]
					m_addr <= r(r2);
					m_reg <= r1;
					s <= load;
				when x"0005" =>
					-- ST [r]
					m_addr <= r(r1);
					m_reg <= r2;
					s <= store;
				when x"0006" =>
					-- HCF
					s <= halt;
				when x"0007" =>
					-- ADD
					tmp33 := std_logic_vector(unsigned('0' & r(r2)) + unsigned('0' & r(r3)));
					r(r1) <= tmp33(31 downto 0);
					f.c <= tmp33(32);
					f.z <= not or_reduce(tmp33(31 downto 0));
					done;
				when x"0008" =>
					-- SUB
					tmp33 := std_logic_vector(unsigned('0' & r(r2)) - unsigned('0' & r(r3)));
					r(r1) <= tmp33(31 downto 0);
					f.c <= tmp33(32);
					f.z <= not or_reduce(tmp33(31 downto 0));
					done;
				when x"0009" =>
					-- SBC
					tmp33 := std_logic_vector(unsigned('0' & r(r2)) - unsigned('0' & r(r3)) - unsigned'("" & f.c));
					r(r1) <= tmp33(31 downto 0);
					f.c <= tmp33(32);
					f.z <= not or_reduce(tmp33(31 downto 0));
					done;
				when x"000a" =>
					-- MUL
					tmp64 := std_logic_vector(unsigned(r(r3)) * unsigned(r(r4)));
					r(r1) <= tmp64(63 downto 32);
					r(r2) <= tmp64(31 downto 0);
					f.c <= '0';
					f.z <= not or_reduce(tmp64);
					done;
				when x"000b" =>
					-- DIVMOD
					tmp32 := std_logic_vector(unsigned(r(r3)) / unsigned(r(r4)));
					r(r1) <= tmp32;
					f.c <= not or_reduce(r(r4));
					f.z <= not or_reduce(tmp32);
					tmp32 := std_logic_vector(unsigned(r(r3)) mod unsigned(r(r4)));
					r(r2) <= tmp32;
					done;
				when x"000c" =>
					-- AND
					tmp32 := r(r2) and r(r3);
					r(r1) <= tmp32;
					f.c <= '0';
					f.z <= not or_reduce(tmp32);
					done;
				when x"000d" =>
					-- OR
					tmp32 := r(r2) or r(r3);
					r(r1) <= tmp32;
					f.c <= '0';
					f.z <= not or_reduce(tmp32);
					done;
				when x"000e" =>
					-- XOR
					tmp32 := r(r2) xor r(r3);
					r(r1) <= tmp32;
					f.c <= '0';
					f.z <= not or_reduce(tmp32);
					done;
				when x"000f" =>
					-- NOT
					tmp32 := not r(r2);
					r(r1) <= tmp32;
					f.c <= '0';
					f.z <= not or_reduce(tmp32);
					done;
				when x"0010" =>
					-- SHL
					tmp33 := std_logic_vector(unsigned('0' & r(r2)) sll to_integer(unsigned(r(r3))));
					r(r1) <= tmp33(31 downto 0);
					f.c <= tmp33(32);
					f.z <= not or_reduce(tmp33(31 downto 0));
					done;
				when x"0011" =>
					-- SHR
					tmp33 := std_logic_vector(unsigned(r(r2) & '0') srl to_integer(unsigned(r(r3))));
					r(r1) <= tmp33(32 downto 1);
					f.c <= tmp33(0);
					f.z <= not or_reduce(tmp33(32 downto 1));
					done;
				when x"0012" =>
					-- ADDI
					tmp33 := std_logic_vector(unsigned('0' & r(r2)) + unsigned('0' & c));
					r(r1) <= tmp33(31 downto 0);
					f.c <= tmp33(32);
					f.z <= not or_reduce(tmp33(31 downto 0));
					done;
				when x"0013" =>
					-- SUBI
					tmp33 := std_logic_vector(unsigned('0' & r(r2)) - unsigned('0' & c));
					r(r1) <= tmp33(31 downto 0);
					f.c <= tmp33(32);
					f.z <= not or_reduce(tmp33(31 downto 0));
					done;
				when x"0014" =>
					if(r(r2) = r(r3)) then
						r(r1) <= x"00000000";
						f.c <= '0';
						f.z <= '1';
					elsif(unsigned(r(r2)) > unsigned(r(r3))) then
						r(r1) <= x"00000001";
						f.c <= '0';
						f.z <= '0';
					else
						r(r1) <= x"ffffffff";
						f.c <= '1';
						f.z <= '0';
					end if;
					done;
				when others =>
					report "invalid opcode encountered" severity error;
					s <= halt;
			end case;
		end procedure;
	begin
		if(reset = '1') then
			s <= ifetch;
			r(ip) <= reset_ip;
			i_rdreq <= '0';
			d_rdreq <= '0';
			d_wrreq <= '0';
		elsif(rising_edge(clk)) then
			i_rdreq <= '0';
			d_rdreq <= '0';
			d_wrreq <= '0';
			case s is
				when ifetch =>
					i_addr <= r(ip);
					i_rdreq <= '1';
					s <= ifetch2;
				when ifetch2 =>
					i_addr <= r(ip);
					i_rdreq <= '1';
					if(i_waitrequest = '0') then
						i <= i_rddata;
						s <= decode;
					end if;
				when decode =>
					-- defined above because long
					execute_insn;
				when load =>
					d_addr <= m_addr;
					d_rdreq <= '1';
					s <= load2;
				when load2 =>
					d_addr <= m_addr;
					d_rdreq <= '1';
					if(d_waitrequest = '0') then
						r(m_reg) <= d_rddata;
						done;
					end if;
				when store =>
					d_addr <= m_addr;
					d_wrdata <= r(m_reg);
					d_wrreq <= '1';
					s <= store2;
				when store2 =>
					d_addr <= m_addr;
					d_wrdata <= r(m_reg);
					d_wrreq <= '1';
					if(d_waitrequest = '0') then
						done;
					end if;
				when halt =>
					null;
			end case;
		end if;
	end process;
end architecture;
