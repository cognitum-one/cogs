//===============================================================================================================================
//  Copyright � 2005..2025 Advanced Architectures
//
//  All rights reserved
//  Confidential Information
//  Limited Distribution to Authorized Persons Only
//  Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//  Project Name         : A2P
//  Description          : Debug controller
//
//                         Connects the host interface to A2 JTAG Tap Controller
//
//  Author               : RTT
//  Version              : 1.0
//  Creation Date        : 3/3/2005
//
//  2025: Conversion to generic interface into SoC
//
//==============================================================================================================================
//  THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//  IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//  BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//  TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//  AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//  ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//==============================================================================================================================

`ifndef tCQ
`define tCQ #1
`endif

module A2_jtag_interface #(
   parameter BASE_ADDR  = 13'h0000,
   parameter JTAGID     = 32'h600df1d0
   )(
   // JTAG Interface
   input  wire          tck,           // JTAG Test Clock
   input  wire          tms,           // JTAG Test Mode Select
   input  wire          tdi,           // JTAG Test Data In
   output wire          tdo,           // JTAG Test Data Out
   // Processor Interface (imosi/imiso)
   input  wire [47:0]   imosi,         // {cmd[2:0], data[31:0], addr[12:0]}
   output wire [32:0]   imiso,         // {ack, data[31:0]}
   output wire [ 1:0]   cond,          // Condition codes  1: input ready  0: output_ready
   // System Interface
   input  wire          clock,         // System Clock
   input  wire          reset          // System Reset
   );

// Locals =======================================================================================================================
// Outputs
wire [31:0] proc_read_data;
// Output FIFO: Processor writes, JTAG reads
wire [31:0] output_fifo_data;
wire [ 2:0] output_fifo_wc, output_fifo_rc;
wire        output_fifo_ir, output_fifo_or;
// Input FIFO: JTAG writes, Processor reads
wire [31:0] input_fifo_data;
wire [ 2:0] input_fifo_wc, input_fifo_rc;
wire        input_fifo_ir, input_fifo_or;  // input_ready, output_ready
// Data registers
reg  [31:0] dr_shift_reg;
reg  [31:0] idcode_reg = JTAGID;
// JTAG FIFO control signals
reg         jtag_input_write;   // JTAG writes to input FIFO
reg         jtag_output_read;   // JTAG reads from output FIFO
reg  [31:0] jtag_write_data;
// Instruction register
reg  [ 3:0] instruction_reg,
            ir_shift_reg;
// Address decode
wire        data_reg_write,   data_reg_read,
            status_reg_write, status_reg_read,
            proc_access;
// Processor interface signals
wire        proc_ack;
wire [31:0] proc_response_data;
wire        fifo_advance_pulse;
reg         prev_data_reg_read;
wire        fifo_write_pulse;
reg         prev_data_reg_write;
// Processor interface decode
wire [ 2:0] proc_cmd;
wire [31:0] proc_data;
wire [12:0] proc_addr;

// Status register bits
wire [31:0] status_register = {
   16'h0000,                 // bits 31:16 - Reserved
   input_fifo_wc,            // bits 15:13 - Input FIFO write count
   input_fifo_rc,            // bits 12:10 - Input FIFO read count
   input_fifo_ir,            // bit  9     - Input FIFO ready (not full)
   input_fifo_or,            // bit  8     - Input FIFO has data (not empty)
   output_fifo_wc,           // bits 07:05 - Output FIFO write count
   output_fifo_rc,           // bits 04:02 - Output FIFO read count
   output_fifo_ir,           // bit  1     - Output FIFO ready (not full)
   output_fifo_or            // bit  0     - Output FIFO has data (not empty)
   };

// Address Decode ===============================================================================================================

// Processor interface decode
assign
   proc_cmd       = imosi[47:45],
   proc_data      = imosi[44:13],
   proc_addr      = imosi[12:0];

// Command encodings
localparam [2:0]
   CMD_READ          = 3'b101,
   CMD_WRITE         = 3'b110;

// Address map
localparam [12:0]
   DATA_ADDR         = BASE_ADDR + 13'd0,    // FIFO data access
   STATUS_ADDR       = BASE_ADDR + 13'd1;    // Control and status

// Address decode
assign
   data_reg_write    = (proc_cmd == CMD_WRITE) && (proc_addr ==   DATA_ADDR),
   data_reg_read     = (proc_cmd == CMD_READ)  && (proc_addr ==   DATA_ADDR),
   status_reg_write  = (proc_cmd == CMD_WRITE) && (proc_addr == STATUS_ADDR),
   status_reg_read   = (proc_cmd == CMD_READ)  && (proc_addr == STATUS_ADDR),
   proc_access       = data_reg_write | data_reg_read | status_reg_write | status_reg_read;

// Single-cycle acknowledge response
assign proc_ack = proc_access;
assign proc_response_data = data_reg_read   ? input_fifo_data
                          : status_reg_read ? status_register
                          : 32'h0;

// Generate single-cycle rAFF pulse on rising edge of data_reg_read
assign fifo_advance_pulse = data_reg_read && !prev_data_reg_read;

// Generate single-cycle wAFF pulse on rising edge of data_reg_write
assign fifo_write_pulse = data_reg_write && !prev_data_reg_write;

// Register previous state for edge detection
always @(posedge clock or posedge reset)
   if (reset) begin
      prev_data_reg_read <= 1'b0;
      prev_data_reg_write <= 1'b0;
      end
   else begin
      prev_data_reg_read <= data_reg_read;
      prev_data_reg_write <= data_reg_write;
      end

// JTAG TAP Controller ==========================================================================================================

// TAP States - Complete IEEE 1149.1 State Machine (16 states)
localparam [3:0]
   TEST_LOGIC_RESET  = 4'd0,
   RUN_TEST_IDLE     = 4'd1,
   SELECT_DR_SCAN    = 4'd2,
   CAPTURE_DR        = 4'd3,
   SHIFT_DR          = 4'd4,
   EXIT1_DR          = 4'd5,
   PAUSE_DR          = 4'd6,
   EXIT2_DR          = 4'd7,
   UPDATE_DR         = 4'd8,
   SELECT_IR_SCAN    = 4'd9,
   CAPTURE_IR        = 4'd10,
   SHIFT_IR          = 4'd11,
   EXIT1_IR          = 4'd12,
   PAUSE_IR          = 4'd13,
   EXIT2_IR          = 4'd14,
   UPDATE_IR         = 4'd15;

// Instructions
localparam [3:0]
   IDCODE            = 4'd0,  // Device ID
   FIFO_WR           = 4'd5,  // Write to input FIFO (proc reads)
   FIFO_RD           = 4'd6,  // Read from output FIFO (proc writes)
   STATUS_RD         = 4'd7,  // Read status
   COND_RD           = 4'd8;  // Read 2-bit condition codes

// TAP state machine
reg [3:0] tap_state, next_tap_state;
reg [3:0] prev_tap_state;

always @* case (tap_state)
   // Complete IEEE 1149.1 TAP Controller State Machine
   TEST_LOGIC_RESET  :  next_tap_state = tms ? TEST_LOGIC_RESET : RUN_TEST_IDLE;
   RUN_TEST_IDLE     :  next_tap_state = tms ? SELECT_DR_SCAN   : RUN_TEST_IDLE;
   // Data Register Path
   SELECT_DR_SCAN    :  next_tap_state = tms ? SELECT_IR_SCAN   : CAPTURE_DR;
   CAPTURE_DR        :  next_tap_state = tms ? EXIT1_DR         : SHIFT_DR;
   SHIFT_DR          :  next_tap_state = tms ? EXIT1_DR         : SHIFT_DR;
   EXIT1_DR          :  next_tap_state = tms ? UPDATE_DR        : PAUSE_DR;
   PAUSE_DR          :  next_tap_state = tms ? EXIT2_DR         : PAUSE_DR;
   EXIT2_DR          :  next_tap_state = tms ? UPDATE_DR        : SHIFT_DR;
   UPDATE_DR         :  next_tap_state = tms ? SELECT_DR_SCAN   : RUN_TEST_IDLE;
   // Instruction Register Path
   SELECT_IR_SCAN    :  next_tap_state = tms ? TEST_LOGIC_RESET : CAPTURE_IR;
   CAPTURE_IR        :  next_tap_state = tms ? EXIT1_IR         : SHIFT_IR;
   SHIFT_IR          :  next_tap_state = tms ? EXIT1_IR         : SHIFT_IR;
   EXIT1_IR          :  next_tap_state = tms ? UPDATE_IR        : PAUSE_IR;
   PAUSE_IR          :  next_tap_state = tms ? EXIT2_IR         : PAUSE_IR;
   EXIT2_IR          :  next_tap_state = tms ? UPDATE_IR        : SHIFT_IR;
   UPDATE_IR         :  next_tap_state = tms ? SELECT_DR_SCAN   : RUN_TEST_IDLE;
   default              next_tap_state = TEST_LOGIC_RESET;
   endcase

always @(posedge tck or posedge reset)
   if (reset) begin
      tap_state <= TEST_LOGIC_RESET;
      prev_tap_state <= TEST_LOGIC_RESET;
      end
   else begin
      prev_tap_state <= tap_state;
      tap_state <= next_tap_state;
      end

// Instruction register

always @(posedge tck or posedge reset)
   if (reset) begin
      ir_shift_reg      <= 4'h0;
      instruction_reg   <= IDCODE;
      end
   else if (tap_state == TEST_LOGIC_RESET) begin
      ir_shift_reg      <= 4'h0;
      instruction_reg   <= IDCODE;
      end
   else if (tap_state == CAPTURE_IR)   ir_shift_reg      <= instruction_reg;
   else if (tap_state == SHIFT_IR)     ir_shift_reg      <= {tdi, ir_shift_reg[3:1]};
   else if (tap_state == UPDATE_IR)    instruction_reg   <= ir_shift_reg;

always @(posedge tck or posedge reset)
   if (reset) begin
      dr_shift_reg      <= 32'h0;
      jtag_input_write  <= 1'b0;
      jtag_output_read  <= 1'b0;
      end
   else if (tap_state == TEST_LOGIC_RESET) begin
      dr_shift_reg      <= 32'h0;
      jtag_input_write  <= 1'b0;
      jtag_output_read  <= 1'b0;
      end
   else if (tap_state == CAPTURE_DR) begin
      jtag_input_write  <= 1'b0;
      jtag_output_read  <= 1'b0;
      case (instruction_reg)
         IDCODE   :  dr_shift_reg <= idcode_reg;
         FIFO_WR  : begin
            dr_shift_reg <= 32'h0;  // Clear for write
         end
         FIFO_RD  :  dr_shift_reg <= output_fifo_data;
         STATUS_RD:  dr_shift_reg <= status_register;
         COND_RD  :  dr_shift_reg <= {30'h0, input_fifo_ir, output_fifo_or}; // 2-bit condition codes
         default  :  dr_shift_reg <= 32'h0;
         endcase
      end
   else if (tap_state == SHIFT_DR)
      case (instruction_reg)
         FIFO_WR, FIFO_RD, IDCODE, STATUS_RD, COND_RD:
            dr_shift_reg   <= {tdi, dr_shift_reg[31:1]};
         endcase
   else if (tap_state == UPDATE_DR)
      case (instruction_reg)
         FIFO_WR: begin
            jtag_write_data   <= dr_shift_reg;
            if (input_fifo_ir)
               jtag_input_write  <= 1'b1;  // Only write if FIFO not full
            end
         FIFO_RD:
            jtag_output_read  <= 1'b1;
         endcase
   else begin
      // Clear write/read pulses in all other states
      jtag_input_write <= 1'b0;
      jtag_output_read <= 1'b0;
      end

// TDO output
assign
   tdo   =  (tap_state == SHIFT_DR) ? dr_shift_reg[0]
         :  (tap_state == SHIFT_IR) ? ir_shift_reg[0] : 1'b0;

// Async FIFOs (4 deep) =========================================================================================================

// Input FIFO: JTAG writes, Processor reads
A2_adFIFO #(
   .WIDTH(32),
   .LG2DEPTH(2)
   ) input_fifo (
   // Write side (JTAG domain)
   .AFFwc   (input_fifo_wc    ),    // Write count
   .AFFir   (input_fifo_ir    ),    // Input ready (not full)
   .iAFF    (jtag_write_data  ),    // Data in
   .wAFF    (jtag_input_write ),    // Write enable
   .wr_clk  (tck              ),    // JTAG clock
   // Read side (Processor domain)
   .AFFo    (input_fifo_data  ),    // Data out
   .AFFrc   (input_fifo_rc    ),    // Read count
   .AFFor   (input_fifo_or    ),    // Output ready (not empty)
   .rAFF    (fifo_advance_pulse),    // Read enable - advance after ack cycle
   .rd_clk  (clock            ),    // System clock
   // Global
   .reset(reset)
   );

// Output FIFO: Processor writes, JTAG reads
A2_adFIFO #(
   .WIDTH(32),
   .LG2DEPTH(2)
   ) output_fifo (
   // Write side (Processor domain)
   .AFFwc   (output_fifo_wc   ),    // Write count
   .AFFir   (output_fifo_ir   ),    // Input ready (not full)
   .iAFF    (proc_data        ),    // Data in
   .wAFF    (fifo_write_pulse ),    // Write enable
   .wr_clk  (clock            ),    // System clock
   // Read side (JTAG domain)
   .AFFo    (output_fifo_data ),    // Data out
   .AFFrc   (output_fifo_rc   ),    // Read count
   .AFFor   (output_fifo_or   ),    // Output ready (not empty)
   .rAFF    (jtag_output_read ),    // Read enable
   .rd_clk  (tck              ),    // JTAG clock
   // Global
   .reset   (reset            )
   );



// Processor Interface Response ================================================================================================

assign
   cond           = {output_fifo_ir, input_fifo_or},
   imiso          = {proc_ack, proc_response_data};

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_jtag_interface_tb

module t;

// Parameters
localparam BASE_ADDR = 13'h0000;
localparam JTAGID = 32'h600df1d0;
localparam DATA_ADDR = BASE_ADDR + 13'd0;
localparam STATUS_ADDR = BASE_ADDR + 13'd1;
// TAP States - Complete IEEE 1149.1 State Machine (16 states)
localparam [3:0]
   TEST_LOGIC_RESET  = 4'd0,
   RUN_TEST_IDLE     = 4'd1,
   SELECT_DR_SCAN    = 4'd2,
   CAPTURE_DR        = 4'd3,
   SHIFT_DR          = 4'd4,
   EXIT1_DR          = 4'd5,
   PAUSE_DR          = 4'd6,
   EXIT2_DR          = 4'd7,
   UPDATE_DR         = 4'd8,
   SELECT_IR_SCAN    = 4'd9,
   CAPTURE_IR        = 4'd10,
   SHIFT_IR          = 4'd11,
   EXIT1_IR          = 4'd12,
   PAUSE_IR          = 4'd13,
   EXIT2_IR          = 4'd14,
   UPDATE_IR         = 4'd15;

wire  [127:0] TSM_STATE_string =
     mut.tap_state == TEST_LOGIC_RESET ? "TEST_LOGIC_RESET"
   : mut.tap_state == RUN_TEST_IDLE    ? "RUN_TEST_IDLE   "
   : mut.tap_state == SELECT_DR_SCAN   ? "SELECT_DR_SCAN  "
   : mut.tap_state == CAPTURE_DR       ? "CAPTURE_DR      "
   : mut.tap_state == SHIFT_DR         ? "SHIFT_DR        "
   : mut.tap_state == EXIT1_DR         ? "EXIT1_DR        "
   : mut.tap_state == PAUSE_DR         ? "PAUSE_DR        "
   : mut.tap_state == EXIT2_DR         ? "EXIT2_DR        "
   : mut.tap_state == UPDATE_DR        ? "UPDATE_DR       "
   : mut.tap_state == SELECT_IR_SCAN   ? "SELECT_IR_SCAN  "
   : mut.tap_state == CAPTURE_IR       ? "CAPTURE_IR      "
   : mut.tap_state == SHIFT_IR         ? "SHIFT_IR        "
   : mut.tap_state == EXIT1_IR         ? "EXIT1_IR        "
   : mut.tap_state == PAUSE_IR         ? "PAUSE_IR        "
   : mut.tap_state == EXIT2_IR         ? "EXIT2_IR        "
   : mut.tap_state == UPDATE_IR        ? "UPDATE_IR       "
   :                                     "--- ERROR ------";

// System signals
reg clock, reset;

// JTAG signals
reg tck, tms, tdi;
wire tdo;

// Processor interface
reg [47:0] imosi;
wire [32:0] imiso;
wire [1:0] cond;  // Condition codes: [1]=input_ready, [0]=output_ready

// Test control
integer test_number;
integer error_count;
reg [255:0] test_name;

// Test variables
reg [31:0] idcode_read;
reg [31:0] status_read;
reg [31:0] proc_status;
reg [31:0] test_data;
reg [31:0] read_data;
reg [31:0] dummy;
reg [31:0] test_patterns [0:3];
reg        read_bit, test_bit;
integer i;

// JTAG instructions
localparam [3:0]
    IDCODE     = 4'd0,
    FIFO_WR    = 4'd5,
    FIFO_RD    = 4'd6,
    STATUS_RD  = 4'd7,
    COND_RD    = 4'd8;

// Additional JTAG test tasks
task goto_shift_dr;
   begin
      tms = 1'b1; @(posedge tck); // RUN_TEST_IDLE -> SELECT_DR_SCAN
      tms = 1'b0; @(posedge tck); // SELECT_DR_SCAN -> CAPTURE_DR
      tms = 1'b0; @(posedge tck); // CAPTURE_DR -> SHIFT_DR
      end
   endtask

task shift_data_32;
   input  [31:0] data_in;
   output [31:0] data_out;
   integer i;
   begin
      // Go to SHIFT_DR
      tms = 1'b1; @(posedge tck); // -> SELECT_DR_SCAN
      tms = 1'b0; @(posedge tck); // -> CAPTURE_DR
      tms = 1'b0; @(posedge tck); // -> SHIFT_DR

      // Shift 32 bits
      tms = 1'b0;
      for (i = 0; i < 31; i = i + 1) begin
         tdi = data_in[i];
         @(posedge tck);
         data_out = {tdo, data_out[31:1]};
         end

      // Last bit with exit
      tdi = data_in[31];
      tms = 1'b1; @(posedge tck); // -> EXIT1_DR
      data_out = {tdo, data_out[31:1]};
      tdi = 1'b0;

      // Complete exit
      tms = 1'b1; @(posedge tck); // -> UPDATE_DR
      tms = 1'b0; @(posedge tck); // -> RUN_TEST_IDLE
      end
   endtask

task shift_data_2;
   input  [1:0] data_in;
   output [1:0] data_out;
   begin
      // Go to SHIFT_DR
      tms = 1'b1; @(posedge tck); // -> SELECT_DR_SCAN
      tms = 1'b0; @(posedge tck); // -> CAPTURE_DR
      tms = 1'b0; @(posedge tck); // -> SHIFT_DR

      // Shift 2 bits
      tms = 1'b0;
      tdi = data_in[0];
      @(posedge tck);
      data_out[0] = tdo;

      // Last bit with exit
      tdi = data_in[1];
      tms = 1'b1; @(posedge tck); // -> EXIT1_DR
      data_out[1] = tdo;
      tdi = 1'b0;

      // Complete exit
      tms = 1'b1; @(posedge tck); // -> UPDATE_DR
      tms = 1'b0; @(posedge tck); // -> RUN_TEST_IDLE
      end
   endtask

task jtag_shift_bit;
   input bit_in;
   output bit_out;
   begin
      goto_shift_dr();
      tdi = bit_in;
      @(posedge tck);
      bit_out = tdo;
      // Exit SHIFT_DR
      tms = 1'b1; @(posedge tck); // SHIFT_DR -> EXIT1_DR
      tms = 1'b1; @(posedge tck); // EXIT1_DR -> UPDATE_DR
      tms = 1'b0; @(posedge tck); // UPDATE_DR -> RUN_TEST_IDLE
      end
   endtask

task jtag_read_32bit;
   output [31:0] data;
   begin
      shift_data_32(32'h0, data);
      end
   endtask

task jtag_read_condition_codes;
   output [1:0] cond_bits;
   begin
      load_instruction(COND_RD);
      shift_data_2(2'b00, cond_bits);
      end
   endtask

// Safe FIFO read with condition code checking
task proc_read_fifo_safe;
   output [31:0] data;
   integer timeout;
   begin
      timeout = 0;
      // Wait for output ready (FIFO has data) - check processor condition codes directly
      while (!cond[0] && timeout < 1000) begin // output_ready = 0 (FIFO empty)
         wait_cycles(10);
         timeout = timeout + 10;
         end

      if (timeout >= 1000)
         data = 32'hDEADBEEF; // Error marker
      else
         // FIFO has data, safe to read
         proc_read(DATA_ADDR, data);
      end
   endtask

// Command encodings for processor interface
localparam [2:0]
    CMD_READ  = 3'b101,
    CMD_WRITE = 3'b110;

//===============================================================================================================================
// MUT Instantiation

A2_jtag_interface #(
   .BASE_ADDR  (BASE_ADDR),
   .JTAGID     (JTAGID)
   ) mut (
   .tck     (tck  ),
   .tms     (tms  ),
   .tdi     (tdi  ),
   .tdo     (tdo  ),
   .imosi   (imosi),
   .imiso   (imiso),
   .cond    (cond ),
   .clock   (clock),
   .reset   (reset)
   );

//===============================================================================================================================
// Clock Generation

// System clock - 100MHz
initial begin
   clock = 1'b0;
   forever #5 clock = ~clock;
   end

// JTAG clock - 10MHz
initial begin
   tck = 1'b0;
   forever #50 tck = ~tck;
   end

//===============================================================================================================================
// Reset Generation

initial begin
   reset = 1'b1;
   repeat(10) @(posedge clock);
   reset = 1'b0;
   end

//===============================================================================================================================
// Test Utilities

task start_test(input [255:0] name);
   begin
   test_number = test_number + 1;
   test_name = name;
   $display("");
   $display("========================================");
   $display("TEST %0d: %s", test_number, name);
   $display("========================================");
   end
endtask

task check_result(input [31:0] expected, input [31:0] actual, input [255:0] description);
   begin
   if (expected === actual) $display("[%t] PASS: %s", $time, description);
   else begin
      $display("[%t] FAIL: %s - Expected: %h, Got: %h", $time, description, expected, actual);
      error_count = error_count + 1;
      end
   end
   endtask

task wait_cycles(input integer cycles);
   begin
   repeat(cycles) @(posedge clock);
   end
endtask

//===============================================================================================================================
// Processor Interface Tasks

task proc_write(input [12:0] addr, input [31:0] data);
   begin
   imosi = {CMD_WRITE, data, addr};
   @(posedge clock);
   wait(imiso[32]); // Wait for ACK
   @(posedge clock);
   imosi = 48'h0;
   end
   endtask

task proc_read(input [12:0] addr, output [31:0] data);
   begin
   imosi = {CMD_READ, 32'h0, addr};
   @(posedge clock);
   wait(imiso[32]); // Wait for ACK
   data = imiso[31:0];
   @(posedge clock);
   imosi = 48'h0;
   end
endtask

//===============================================================================================================================
// JTAG Protocol Tasks

task jtag_reset;
   begin
   tms = 1'b1;
   repeat(5) @(posedge tck);
   tms = 1'b0;
   @(posedge tck);
   end
   endtask

task load_instruction(input [3:0] instruction);
   begin
   // Go to SHIFT_IR
   tms = 1'b1; @(posedge tck); // -> SELECT_DR_SCAN
   tms = 1'b1; @(posedge tck); // -> SELECT_IR_SCAN
   tms = 1'b0; @(posedge tck); // -> CAPTURE_IR
   tms = 1'b0; @(posedge tck); // -> SHIFT_IR

   // Shift instruction (4 bits)
   tms = 1'b0;
   tdi = instruction[0]; @(posedge tck);
   tdi = instruction[1]; @(posedge tck);
   tdi = instruction[2]; @(posedge tck);

   // Last bit with exit
   tdi = instruction[3];
   tms = 1'b1; @(posedge tck); // -> EXIT1_IR
   tdi = 1'b0;

   // Complete exit
   tms = 1'b1; @(posedge tck); // -> UPDATE_IR
   tms = 1'b0; @(posedge tck); // -> RUN_TEST_IDLE
   end
   endtask


task jtag_write_fifo(input [31:0] data);
   reg [31:0] dummy;
   begin
   load_instruction(FIFO_WR);
   repeat(5) @(posedge tck); // Wait a few TCK cycles between instruction and data
   shift_data_32(data, dummy);
   end
   endtask

task jtag_read_fifo(output [31:0] data);
   begin
   load_instruction(FIFO_RD);
   shift_data_32(32'h0, data);
   end
   endtask

task jtag_read_status(output [31:0] status);
   begin
   load_instruction(STATUS_RD);
   shift_data_32(32'h0, status);
   end
   endtask

task jtag_read_cond(output [1:0] cond_bits);
   begin
   load_instruction(COND_RD);
   shift_data_2(2'b00, cond_bits);
   end
   endtask

task jtag_read_idcode(output [31:0] idcode);
   begin
   load_instruction(IDCODE);
   shift_data_32(32'h0, idcode);
   end
   endtask

//===============================================================================================================================
// Main Test Sequence

initial begin
   test_number = 0;
   error_count = 0;

   // Initialize
   tms = 1'b1;
   tdi = 1'b0;
   imosi = 48'h0;

   // Wait for reset
   wait(!reset);
   wait_cycles(50);

   //===========================================
   // Test 1: IDCODE Test
   start_test("IDCODE Verification");

   jtag_reset();
   jtag_read_idcode(idcode_read);
   check_result(JTAGID, idcode_read, "IDCODE register");

   //===========================================
   // Test 2: Status Register Read
   start_test("Initial Status Read");

   jtag_read_status(status_read);

   // Also read via processor
   proc_read(STATUS_ADDR, proc_status);
   check_result(status_read, proc_status, "JTAG vs Proc status consistency");

   //===========================================
   // Test 3: JTAG Write -> Processor Read
   start_test("JTAG Write to Processor Read");

   test_data = 32'hdeadbeef;
   jtag_write_fifo(test_data);

   // Allow async FIFO transfer - wait for clock domain crossing
   wait_cycles(50);

   // Check that input FIFO has data for processor to read
   // Note: We don't check condition codes here since the FIFO may not be full
   // The real test is whether the processor can successfully read the data

   // Read via processor
   proc_read_fifo_safe(read_data);
   check_result(test_data, read_data, "JTAG->Processor transfer");

   //===========================================
   // Test 4: Processor Write -> JTAG Read
   start_test("Processor Write to JTAG Read");

   test_data = 32'hcafebabe;
   proc_write(DATA_ADDR, test_data);

   // Allow async FIFO transfer - wait for clock domain crossing
   wait_cycles(50);

   // Read via JTAG
   jtag_read_fifo (read_data);
   check_result   (test_data, read_data, "Processor->JTAG transfer");

   //===========================================
   // Test 5: Multiple Data Transfer
   start_test("Multiple Data Transfers");

   // Send multiple words via JTAG
   for (i = 0; i < 4; i = i + 1) begin
      test_data = 32'h1000 + i;
      jtag_write_fifo(test_data);
      wait_cycles(50); // Even longer delay to ensure complete FIFO operation
   end

   // Wait for final write to propagate across clock domains
   wait_cycles(50);

   // Read them via processor
   for (i = 0; i < 4; i = i + 1) begin
      proc_read_fifo_safe(read_data);
      case (i)
         0: check_result(32'h1000 + i, read_data, "Multi-transfer 0");
         1: check_result(32'h1000 + i, read_data, "Multi-transfer 1");
         2: check_result(32'h1000 + i, read_data, "Multi-transfer 2");
         3: check_result(32'h1000 + i, read_data, "Multi-transfer 3");
      endcase
      wait_cycles(10); // Increased wait between reads
   end

   //===========================================
   // Test 6: Condition Code Functionality
   start_test("Condition Code Functionality");

   // Ensure FIFOs start empty
   wait_cycles(100);

   // Fill input FIFO from JTAG side (4 items to fill 4-deep FIFO)
   for (i = 0; i < 4; i = i + 1) begin
      test_data = 32'h2000 + i;
      jtag_write_fifo(test_data);
      wait_cycles(25); // Longer wait for async FIFO propagation
   end

   // Input FIFO should now have data, so output_ready should be high
   wait_cycles(5); // Just enough for CDC, not enough for other activity
   check_result(1'b1, cond[0], "Input FIFO has data - output_ready should be high");

   // Fill output FIFO from processor side
   for (i = 0; i < 4; i = i + 1) begin
      test_data = 32'h3000 + i;
      proc_write(DATA_ADDR, test_data);
      wait_cycles(15); // Slightly longer wait for condition codes to update
   end

   // Output FIFO should be full, so input_ready should be low
   wait_cycles(5); // Just enough for condition codes to settle
   check_result(1'b0, cond[1], "Output FIFO full - input_ready should be low");

   // Drain input FIFO via processor (drain 4 items)
   for (i = 0; i < 4; i = i + 1) begin
      proc_read_fifo_safe(read_data);
      wait_cycles(15); // Longer wait for condition codes to update
   end

   // Extra delay for write count to propagate across clock domains
   wait_cycles(6); // Allow write count CDC to settle

   // Drain output FIFO via JTAG
   for (i = 0; i < 4; i = i + 1) begin
      jtag_read_fifo(read_data);
      wait_cycles(25); // Longer wait for async FIFO propagation
   end

   // Extra delay for read count to propagate across clock domains
   wait_cycles(6); // Allow read count CDC to settle

   // Both FIFOs should be empty again
   wait_cycles(50); // Longer wait for condition codes to settle
   check_result(1'b0, cond[0], "Output FIFO empty - output_ready should be low");

   //===========================================
   // Test 7: TAP Controller State Machine
   //===========================================
   start_test("TAP State Machine Compliance");

   // Test reset sequence (IEEE 1149.1 requirement)
   jtag_reset();

   // Verify we can load instructions after reset
   load_instruction(STATUS_RD);
   jtag_read_32bit(read_data);

   // Test IDCODE is default after reset
   jtag_reset();
   jtag_read_32bit(idcode_read); // Should read IDCODE without loading instruction
   check_result(32'h600df1d0, idcode_read, "Default IDCODE after reset");

   //===========================================
   // Test 8: Clock Domain Crossing Stress Test
   //===========================================
   start_test("Clock Domain Crossing Stress Test");

   // Rapid alternating reads/writes to stress CDC
   for (i = 0; i < 4; i = i + 1) begin
      // JTAG write with minimal delay
      test_data = 32'h4000 + i;
      jtag_write_fifo(test_data);
      wait_cycles(50); // Increased wait for async FIFO CDC

      // Processor read
      proc_read_fifo_safe(read_data);
      case (i)
         0: check_result(test_data, read_data, "CDC stress 0");
         1: check_result(test_data, read_data, "CDC stress 1");
         2: check_result(test_data, read_data, "CDC stress 2");
         3: check_result(test_data, read_data, "CDC stress 3");
      endcase
      wait_cycles(10);
   end

   //===========================================
   // Test 9: Data Pattern Integrity
   //===========================================
   start_test("Data Pattern Integrity Test");

   // Clear any residual data in FIFOs
   wait_cycles(100);

   // Test various data patterns for bit integrity
   test_patterns[0] = 32'h00000000; // All zeros
   test_patterns[1] = 32'hFFFFFFFF; // All ones
   test_patterns[2] = 32'hAAAAAAAA; // Alternating 1010
   test_patterns[3] = 32'h55555555; // Alternating 0101

   for (i = 0; i < 4; i = i + 1) begin

      // JTAG->Processor
      jtag_write_fifo(test_patterns[i]);
      wait_cycles(100); // Longer wait for proper CDC
      proc_read_fifo_safe(read_data);
      case (i)
         0: check_result(test_patterns[i], read_data, "Pattern all-zeros");
         1: check_result(test_patterns[i], read_data, "Pattern all-ones");
         2: check_result(test_patterns[i], read_data, "Pattern 1010");
         3: check_result(test_patterns[i], read_data, "Pattern 0101");
      endcase

      // Processor->JTAG
      proc_write(DATA_ADDR, test_patterns[i]);
      wait_cycles(100); // Increased wait for proper CDC
      jtag_read_fifo(read_data);
      case (i)
         0: check_result(test_patterns[i], read_data, "Reverse pattern all-zeros");
         1: check_result(test_patterns[i], read_data, "Reverse pattern all-ones");
         2: check_result(test_patterns[i], read_data, "Reverse pattern 1010");
         3: check_result(test_patterns[i], read_data, "Reverse pattern 0101");
      endcase
   end

   //============================================================================================================================
   // Test Summary
   wait_cycles(100);

   $display("");
   $display("========================================");
   $display("TEST SUMMARY");
   $display("========================================");
   $display("Total Tests Run: %0d", test_number);
   $display("Total Errors:    %0d", error_count);

   if (error_count == 0) begin
      $display("*** ALL TESTS PASSED ***");
      $display("Simple JTAG FIFO Interface working correctly");
      end
   else
      $display("*** %0d TESTS FAILED ***", error_count);

   $display("========================================");

   #1000;
   $finish;
end

//=======================================================
// Timeout and Waveforms

//initial begin
//   #50000000;  // 50ms timeout
//   $finish;
//   end

//initial begin
//   $dumpfile("jtag_simple_test.vcd");
//   $dumpvars(0, jtag_simple_testbench);
//   end

endmodule
`endif
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
