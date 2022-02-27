library ieee;
use ieee.std_logic_1164.ALL;
use ieee.numeric_std.ALL;

library std;
use std.env.finish;

entity tb_cpu is
begin
end entity;

architecture sim of tb_cpu is
	signal reset : std_logic;
	signal clk : std_logic := '0';

	constant address_width : integer := 32;
	constant word_width : integer := 32;

	subtype address is std_logic_vector(address_width - 1 downto 0);
	subtype insn is std_logic_vector(63 downto 0);
	subtype word is std_logic_vector(word_width - 1 downto 0);

	signal i_addr : address;
	signal i_rddata : insn;
	signal i_rdreq : std_logic;
	signal i_waitrequest : std_logic;

	signal d_addr : address;
	signal d_rddata : word;
	signal d_rdreq : std_logic;
	signal d_wrdata : word;
	signal d_wrreq : std_logic;
	signal d_waitrequest : std_logic;

	signal halted : std_logic;

	type insn_mem is array(16#0000# to 16#00ff#) of insn;
	signal i : insn_mem;

	type data_mem is array(16#0000# to 16#ffff#) of word;
	signal d : data_mem;
begin
	-- reset gen
	reset <= '1', '0' after 1 us;

	-- clk gen
	clk <= not clk after 100 ns;

	-- sim timeout
	process is
	begin
		wait for 30 us;
		report "sim timeout" severity error;
		finish;
	end process;

	-- normal exit
	process is
	begin
		wait until halted = '1';
		finish;
	end process;

	-- stimuli
	process is
		type insn_file_type is file of character;
		file rom : insn_file_type;
		variable fstatus : file_open_status;
		variable t : character;
		variable a : insn;
		variable x : integer;
	begin
		file_open(fstatus, rom, "../roms/hello_world.backseat", read_mode);
		x := 0;
		while not endfile(rom) loop
			a := (others => '0');
			for y in 0 to 7 loop
				read(rom, t);
				a := a(55 downto 0) & std_logic_vector(to_unsigned(character'pos(t), 8));
			end loop;
			i(x) <= a;
			x := x + 1;
		end loop;
		wait;
	end process;

	-- instruction bus
	i_rddata <= i(to_integer(unsigned(i_addr))) when i_rdreq = '1' else (others => 'U');
	i_waitrequest <= '0';

	-- data bus
	d_rddata <= d(to_integer(unsigned(d_addr))) when d_rdreq = '1' else (others => 'U');
	d(to_integer(unsigned(d_addr))) <= d_wrdata when rising_edge(clk) and d_wrreq = '1';
	d_waitrequest <= '0';

	-- dut
	dut : entity work.cpu
		port map(
			reset => reset,
			clk => clk,
			i_addr => i_addr,
			i_rddata => i_rddata,
			i_rdreq => i_rdreq,
			i_waitrequest => i_waitrequest,
			d_addr => d_addr,
			d_rddata => d_rddata,
			d_rdreq => d_rdreq,
			d_wrdata => d_wrdata,
			d_wrreq => d_wrreq,
			d_waitrequest => d_waitrequest,
			halted => halted
	);
end architecture;
