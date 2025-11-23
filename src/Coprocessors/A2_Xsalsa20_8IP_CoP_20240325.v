/* ******************************************************************************************************************************

   Copyright © 2019..2022 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         :  CRYPTO Library

   Description          :  XOR SALSA 20/8 for Litecoin (LTC) Hash
                           Half Fixed memory version
                           32 cycle input
                           32 cycle output
                           FIFO style input/output protocol

*********************************************************************************************************************************
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*********************************************************************************************************************************
   NOTES:

   Clocks are in phase: Rising edge of interface fast_clock always coincident with a rising edge of the internal fast_clock

****************************************************************************************************************************** */

`define  FPGA
//`define  STANDALONE
`ifdef   STANDALONE
   `timescale  1ns/1ps
   `define  BMOSI_WIDTH  96
   `define  BMISO_WIDTH  96
   `define  SMOSI_WIDTH  272
   `define  SMISO_WIDTH  260
   `define  NMOSI_WIDTH  272
   `define  NMISO_WIDTH  260
   `define  TMOSI_WIDTH  66
   `define  TMISO_WIDTH  66
   `define  DMOSI_WIDTH  66
   `define  DMISO_WIDTH  34
   `define  IMOSI_WIDTH  48
   `define  IMISO_WIDTH  33
   `define  fastcycle    #0.040     // Half period
   `define  slowcycle    #0.120
   `define  min_delay    #0.050     // greater than fastcycle
   `define  tFCQ         #0.008
   `define  tFGQ         #0.010
   `define  tCQ          #0.020
   `define  tCYC         #0.120
   `define  tACC         #0.040
   `define  XSALSA_base    13'h1008      // Salsa_CoP
   `define  XSALSA_XBUF    13'h1008
   `define  XSALSA_XBBS    13'h1009
   `define  XSALSA_STOP    13'h100a
   `define  XSALSA_STRT    13'h100b
   `define  XSALSA_CTRL    13'h100c
   `define  TILEONE_WM_BASE      aXSALSA_base      // Base Address
   `define  TILEONE_ADDR_MSB)    16                // Most significant address bit for decode
   `define  WM_NUM_CLOCKS_READ   1                 // Access time in clock cycles     -- supports up to 7 wait states
   `define  WM_NUM_CLOCKS_CYCLE  1                 // RAM cycle time in clock cycles
   `include "A2_defines.vh"
`else
   `include "A2_project_settings.vh"
`endif

module   A2_Xsalsa20_8I_CoP #(
   parameter   [12:0]   BASE     = 13'h00000,
   parameter            NUM_REGS = 5
   )(
   output wire [`IMISO_WIDTH-1:0]   imiso,      // relative to slow_clock     Slave data out {               IOk, IOo[31:0]}
   input  wire [`IMOSI_WIDTH-1:0]   imosi,      // relative to slow_clock     Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   // SRAM Interface
   output wire [`SMOSI_WIDTH-1:0]   smosi,      // relative to fast_clock     Salsa Master output to  SRAM slave input  (WM)
   input  wire [`SMISO_WIDTH-1:0]   smiso,      //                            Salsa Master input from (WM)
   // Global
   output wire                      done,       // relative to slow_clock     Interrupt : Algorithm done
   output wire                      SalsaOn,    //
   input  wire                      fast_clock, // 12 GHz Pipeline  fast_clock
   input  wire                      slow_clock, //  4 GHz Interface slow_clock
   input  wire                      reset       //      relative to slow_clock -- synchronized to fast_clock internally
   );

// Locals -----------------------------------------------------------------------------------------------------------------------
genvar g;
// Buffer Register Files

reg   [31:0]   r0_X [0:31];
reg   [31:0]   r0_Y [0:31];                     // For Interpolation

reg   [31:0]   IN,OUT;
reg   [ 1:0]   CTRL;                            // Control Reg:   0: SalsaOn (WM off limits) 1: Add a Waitreate to r0_X reads
reg            slow_load, slow_read, slow_stop, slow_start, slow_fini;
reg            fast_load, fast_read, fast_stop, fast_start, fast_fini, fast_shift;

// Datapath Nets

wire [255:0]   Saved, MemData;                  // For interpolation

wire  [31:0]   r2_ADD,r3_ADD,r4_ADD,r5_ADD,     // Feedback registers for writes to r0_X[nn]
               b0_HA, b0_HB, b0_HC, b0_HD,      // Selected elements from HI buffer
               b0_LA, b0_LB, b0_LC, b0_LD,      // Selected elements from LO buffer
               b0_XA, b0_XB, b0_XC, b0_XD,      // XOR of memory and selected elements from HI buffer
               b0_YA, b0_YB, b0_YC, b0_YD,      // XOR of memory and selected elements from LO buffer
               b0_ZA, b0_ZB, b0_ZC, b0_ZD,      // XOR of XORs
               b1_DA, b2_DB, b3_DC, b0_DD;      // Digest Data

wire   [9:0]   b1_ADR;                          // Saved Memory Address

wire [255:0]   iWM;                             // Working Memory Write Data
wire [255:0]    WMo;                            // Working Memory Read  Data
wire [  3:0]    WMk;                            // Acknowledge Per bank

wire           freset;

integer  i;

// Synchronize 'reset' to fast clock --------------------------------------------------------------------------------------------

A2_reset  u_rst (.q(freset),.reset(reset),.clock(fast_clock));

// imosi/imosi Interface ========================================================================================================
// For correct operation and to prevent errors, reads and writes to the X buffer are only permitted when in the 'idle' state.
`include "NEWPORT_IO_addresses.vh"

localparam  LG2REGS  =  `LG2(NUM_REGS);

wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
wire           idle;
reg            RXk;

// Decollate imosi
assign   {cIO[2:0], iIO[31:0], aIO[12:0]} = imosi;

wire                 in_range = aIO[12:LG2REGS] == BASE[12:LG2REGS];
wire  [NUM_REGS-1:0]   decode = in_range << aIO[LG2REGS-1:0];
wire  [NUM_REGS-1:0]    write = {NUM_REGS{cIO==3'b110}} & decode;
wire  [NUM_REGS-1:0]     read = {NUM_REGS{cIO==3'b101}} & decode;
// Writes

wire     ld_X  = write[XSALSA_XBUF],        // Push Xbuf data
         ld_XB = write[XSALSA_XBBS],        // Push Xbuf data byte swapped
         stop  = write[XSALSA_STOP],        // Stop the co-processor and reset
         start = write[XSALSA_STRT_RDY],    // Start processing, look for ready
         ld_CR = write[XSALSA_CTRL];        // Load Control Register
// Reads
wire     rd_X  =  read[XSALSA_XBUF],        // Pop  Xbuf data
         rd_XB =  read[XSALSA_XBBS],        // Pop  Xbuf data byte-swapped
         fini  =  read[XSALSA_STRT_RDY];    // Read status:  if finished TRUE(0xffffffff) else FALSE(0x00000000)

wire [2:0]  sIOo  = {(rd_X  &  idle),       // We may select None and return all zeros (FALSE)
                     (rd_XB &  idle),
                      ld_CR | (fini  && (done | idle))};

A2_mux #(3,32) iMXo (.z(IOo[31:0]), .s(sIOo), .d({OUT[31:0], OUT[7:0],OUT[15:8],OUT[23:16],OUT[31:24], 32'hffffffff}));

wire     rXXB  =  rd_X | rd_XB;

always @(posedge slow_clock)  RXk <= `tCQ rXXB;

wire     rdXk  =  CTRL[1] ?  RXk & rXXB : rXXB;

wire     IOk   =  ld_X | ld_XB | stop | start | ld_CR
               |  rdXk | fini;
// Collate imiso
assign   imiso = {IOk,IOo};

// Control Registers ============================================================================================================

// Enable exclusive WM access
always @(posedge slow_clock)  if (ld_CR || reset)  CTRL  <= `tCQ reset ? 2'b00  : iIO[1:0];  // 1 : Waitstate 0: SalsaOn

assign SalsaOn = CTRL[0];

// Capture Write Data
always @(posedge slow_clock)  if (ld_X || ld_XB)   IN    <= `tCQ ld_X ? iIO     : {iIO[7:0],iIO[15:8],iIO[23:16],iIO[31:24]};

// Capture Read
always @(posedge slow_clock)  if (fini || rdXk )   OUT   <= `tCQ fini ? r0_X[0] : r0_X[1];  // 'fini' primes OUT from r0_X head

// Capture commands and move into fast fast_clock domain
// All these flip=flops operate in the same way.  An incoming command in the slow clock domain is first captured on a slow clock
// rising edge.  The output is then captured on the next rising edge of the fast clock.  This signal then, is used to reset the
// slow-clock domain flip-flop so on the next fast clock rising edge the signal is returned to low.  So a single fast-clock
// cycle version of incoming signal is produced.  This all works as the slow-clock rising edge is always coincident with a
// fast-clock rising edge.
//
always @(posedge slow_clock or posedge fast_load )
   if (fast_load)             slow_load   <= `tFCQ   1'b0;                    //                       _finish here
   else                       slow_load   <= `tFCQ   idle & (ld_X | ld_XB);   // Start here _        /
always @(posedge fast_clock)  fast_load   <= `tFCQ   slow_load;               //Steps..   // \_then here_/

always @(posedge slow_clock or posedge fast_read )
   if (fast_read)             slow_read   <= `tFCQ   1'b0;
   else                       slow_read   <= `tFCQ   idle & (rd_X | rd_XB);
always @(posedge fast_clock)  fast_read   <= `tFCQ   slow_read;               //Steps..

always @(posedge slow_clock or posedge fast_start)
   if (fast_start)            slow_start  <= `tFCQ   1'b0;
   else                       slow_start  <= `tFCQ   idle & start;
always @(posedge fast_clock)  fast_start  <= `tFCQ   slow_start;

always @(posedge slow_clock or posedge fast_stop )
   if (fast_stop)             slow_stop   <= `tFCQ   1'b0;
   else                       slow_stop   <= `tFCQ   stop;
always @(posedge fast_clock)  fast_stop   <= `tFCQ   slow_stop;

always @(posedge slow_clock or posedge fast_fini )
   if (fast_fini)             slow_fini   <= `tFCQ   1'b0;
   else                       slow_fini   <= `tFCQ   fini;
always @(posedge fast_clock)  fast_fini   <= `tFCQ   slow_fini;

always @(posedge fast_clock)  fast_shift  <= `tFCQ   slow_load | slow_read;

// Finite State Machine ---------------------------------------------------------------------------------------------------------
//*****[   FLIP-FLOPS    ]*****
reg   [10:0]  ITERATION;
reg    [5:0]  CYCLE;
reg    [2:0]  FSM;     localparam [2:0]  IDLE = 0, PREP = 1, LOW = 2, HIGH = 3, TAIL = 4,  REST = 5, DONE = 6;

// Control Lines
reg           RUN,  PRIME, INTERPOLATE, YSAVED;
reg           RH00, RH01, RH02, RH03;
reg           RL32, RL33, RL34, RL35, RL36, RL37, RL38, RL39, RL40;
reg           RH32, RH33, RH34, RH35, RH36, RH37, RH38, RH39, RH40;
reg           RL32u36,    RL33u37,    RL34u38,    RH32u36,    RH33u37,    RH34u38;
reg           RM00, RM01, RM02, RM03, MIX;
reg           X2Y0, X2Y1, X2Y2, X2Y3;
// Fanouts
reg  [16:0]   RLH0, RLH1, RLH2, RLH3;
reg  [32:0]   RL00, RL01, RL02, RL03;
reg           RL0x, RL0y, RL0z;
reg           RH0x, RH0y, RH0z;
reg           RLHx, RLHy, RLHz;

always @(posedge fast_clock) begin
   if (freset || fast_stop) begin
      FSM         <= `tFCQ IDLE;
      CYCLE       <= `tFCQ  6'd0;
      ITERATION   <= `tFCQ 11'd0;
      RUN         <= `tFCQ  1'b0;
      end
   else (* parallel_case *) case (FSM)
      IDLE  :  if ((CYCLE > 6) && fast_start) begin
                  FSM            <= `tFCQ PREP;
                  ITERATION      <= `tFCQ 11'd0;
                  CYCLE          <= `tFCQ  6'd0;
                  RUN            <= `tFCQ  1'b1;
                  end
               else CYCLE        <= `tFCQ (CYCLE == 6'd7) ? 6'd7 : CYCLE + 1'b1;
      PREP  :  if (CYCLE == 10) begin                       // first write to WM
                  FSM            <= `tFCQ LOW;
                  CYCLE          <= `tFCQ 6'd0;
                  end
               else  CYCLE       <= `tFCQ CYCLE + 1'b1;     // Keep going
      LOW   :  if (CYCLE == 37) begin                       // End of Low
                  FSM            <= `tFCQ HIGH;
                  CYCLE          <= `tFCQ 6'd0;
                  end
               else  CYCLE       <= `tFCQ CYCLE + 1'b1;     // Keep going
      HIGH  :  if (CYCLE == 31) begin                       // Prepare for Memory Access
                  if (!INTERPOLATE)
                     ITERATION   <= `tFCQ ITERATION + 1'd1; // Increment Iteration
                  CYCLE          <= `tFCQ CYCLE + 1'b1;     // Keep incrementing
                  end
               else if (CYCLE == 45) begin                  // End before Interpolate
                  FSM            <= `tFCQ LOW;              // To allow turnaound
                  CYCLE          <= `tFCQ 6'd0;
                  end
               else if (CYCLE == 37) begin                  // End of High
                  if (!INTERPOLATE) begin                   // If going to Interpolate keep going
                     if (ITERATION == 0)                    // Rolled over end of 1k iterations
                        FSM      <= `tFCQ TAIL;             // Finish up
                     else
                        FSM      <= `tFCQ LOW;              // Normal End
                     CYCLE       <= `tFCQ 6'd0;
                     end
                  else CYCLE     <= `tFCQ CYCLE + 1'b1;     // Keep going
                  end
               else  CYCLE       <= `tFCQ CYCLE + 1'b1;     // Keep going
      TAIL  :  if (CYCLE==4) begin
                  FSM            <= `tFCQ DONE;
                  CYCLE          <= `tFCQ 6'd0;
                  end
               else  CYCLE       <= `tFCQ CYCLE + 1'b1;
      DONE  :  if (fast_fini) begin
                  FSM            <= `tFCQ IDLE;
                  RUN            <= `tFCQ 1'b0;
                  end
      endcase
   end

localparam  EARLY =  3;

// Outputs back to Host
assign   done     =  (FSM==DONE);
assign   idle     =  (FSM==IDLE);
// Control Points
wire     prep     =  (FSM==PREP),
         low      =  (FSM==LOW ),
         high     =  (FSM==HIGH),

         gate     =  freset | RUN,   // Clock Gate
         // Early
         e_head_L = (prep &                                 (CYCLE==10-EARLY))
                  | (high & (ITERATION!=0) & (INTERPOLATE ? (CYCLE==45-EARLY)
                                                          : (CYCLE==37-EARLY))),
         e_head_M =  high & ITERATION[10]  & (INTERPOLATE ? (CYCLE==42-EARLY)
                                                          : (CYCLE==37-EARLY)),
         e_head_H =  low  & (CYCLE==37-EARLY),
         // Timely
         head_L   = (prep &                                 (CYCLE==10))
                  | (high & (ITERATION!=0) & (INTERPOLATE ? (CYCLE==45)
                                                          : (CYCLE==37))),
         head_M   =  high &  ITERATION[10] & (INTERPOLATE ? (CYCLE==42)
                                                          : (CYCLE==37)),       // Mix: Becomes first cycle of LOW
         head_H   =  low  & (CYCLE==37),

         tail     = (CYCLE==31),
         tail_L   =  low  & tail,
         tail_H   =  high & tail,
         e_tail   = (CYCLE==30),

         head_P   = (low | high) &  &(CYCLE[1:0])    & ~(CYCLE==35) & ~(CYCLE==43),  // pipeline controls
         e_head_P = (low | high) &   (CYCLE[1:0]==2) & ~(CYCLE==34) & ~(CYCLE==42),  // pipeline controls
         x_head_P = head_P ^ e_head_P,

         MX       =  ITERATION[10],                                          // Read Memory and Mix flag ELSE Write Memory flag

         head_eWM =  !MX                ? (head_L & ~ITERATION[0])
                  :         INTERPOLATE ? (high   &  (CYCLE==37))
                  :                       (high   &  (CYCLE==33)  & ~b1_ADR[0]),

         ld_wrADR =  !MX                &  high   &  (CYCLE==33)  | freset,
         ld_rdADR =   MX & !INTERPOLATE &  high   &  (CYCLE==33),

         read_Y   =   MX &  INTERPOLATE & 1'b0,
         save_X   =   MX &  INTERPOLATE & (CYCLE==38),
         load_X   =   MX &  INTERPOLATE & (CYCLE==41);

// Control bits -----------------------------------------------------------------------------------------------------------------

always @(posedge fast_clock) INTERPOLATE <= `tFCQ ~freset & ((ld_rdADR  & b1_ADR[0]) | (INTERPOLATE & ~(high & (CYCLE==33))));
always @(posedge fast_clock) YSAVED      <= `tFCQ ~freset & (save_X | YSAVED & ~(RL03 & ~INTERPOLATE));

//===============================================================================================================================
// Pipelined control bits that enable register loads

always @(posedge fast_clock) if (freset | RUN & ( RH0x    | e_head_H     ))  RH0x    <= `tFCQ ~(freset | RH0x );
always @(posedge fast_clock) if (freset | RUN & ( RH0y    | RH0x         ))  RH0y    <= `tFCQ ~(freset | RH0y );
always @(posedge fast_clock) if (freset | RUN & ( RH0z    | RH0y         ))  RH0z    <= `tFCQ ~(freset | RH0z );
always @(posedge fast_clock) if (freset | RUN & ( RH00    | RH0z         ))  RH00    <= `tFCQ ~(freset | RH00 );
always @(posedge fast_clock) if (freset | RUN & ( RH01    | RH00         ))  RH01    <= `tFCQ ~(freset | RH01 );
always @(posedge fast_clock) if (freset | RUN & ( RH02    | RH01         ))  RH02    <= `tFCQ ~(freset | RH02 );
always @(posedge fast_clock) if (freset | RUN & ( RH03    | RH02         ))  RH03    <= `tFCQ ~(freset | RH03 );

always @(posedge fast_clock) if (freset | RUN & ( RM00    | load_X       ))  RM00    <= `tFCQ ~(freset | RM00);
always @(posedge fast_clock) if (freset | RUN & ( RM01    | RM00         ))  RM01    <= `tFCQ ~(freset | RM01);
always @(posedge fast_clock) if (freset | RUN & ( RM02    | RM01         ))  RM02    <= `tFCQ ~(freset | RM02);
always @(posedge fast_clock) if (freset | RUN & ( RM03    | RM02         ))  RM03    <= `tFCQ ~(freset | RM03);

always @(posedge fast_clock) if (freset | RUN & ( RL32    | tail_L       ))  RL32    <= `tFCQ ~(freset | RL32);
always @(posedge fast_clock) if (freset | RUN & ( RL33    | RL32         ))  RL33    <= `tFCQ ~(freset | RL33);
always @(posedge fast_clock) if (freset | RUN & ( RL34    | RL33         ))  RL34    <= `tFCQ ~(freset | RL34);
always @(posedge fast_clock) if (freset | RUN & ( RL35    | RL34         ))  RL35    <= `tFCQ ~(freset | RL35);
always @(posedge fast_clock) if (freset | RUN & ( RL36    | RL35         ))  RL36    <= `tFCQ ~(freset | RL36);
always @(posedge fast_clock) if (freset | RUN & ( RL37    | RL36         ))  RL37    <= `tFCQ ~(freset | RL37);
always @(posedge fast_clock) if (freset | RUN & ( RL38    | RL37         ))  RL38    <= `tFCQ ~(freset | RL38);
always @(posedge fast_clock) if (freset | RUN & ( RL39    | RL38         ))  RL39    <= `tFCQ ~(freset | RL39);
always @(posedge fast_clock) if (freset | RUN & ( RL40    | RL39         ))  RL40    <= `tFCQ ~(freset | RL40);

always @(posedge fast_clock) if (freset | RUN & ( RH32    | tail_H       ))  RH32    <= `tFCQ ~(freset | RH32);
always @(posedge fast_clock) if (freset | RUN & ( RH33    | RH32         ))  RH33    <= `tFCQ ~(freset | RH33);
always @(posedge fast_clock) if (freset | RUN & ( RH34    | RH33         ))  RH34    <= `tFCQ ~(freset | RH34);
always @(posedge fast_clock) if (freset | RUN & ( RH35    | RH34         ))  RH35    <= `tFCQ ~(freset | RH35);
always @(posedge fast_clock) if (freset | RUN & ( RH36    | RH35         ))  RH36    <= `tFCQ ~(freset | RH36);
always @(posedge fast_clock) if (freset | RUN & ( RH37    | RH36         ))  RH37    <= `tFCQ ~(freset | RH37);
always @(posedge fast_clock) if (freset | RUN & ( RH38    | RH37         ))  RH38    <= `tFCQ ~(freset | RH38);
always @(posedge fast_clock) if (freset | RUN & ( RH39    | RH38         ))  RH39    <= `tFCQ ~(freset | RH39);
always @(posedge fast_clock) if (freset | RUN & ( RH40    | RH39         ))  RH40    <= `tFCQ ~(freset | RH40);

always @(posedge fast_clock) if (freset | RUN & ( RL32u36 | tail_L | RL35))  RL32u36 <= `tFCQ ~(freset | RL32u36);
always @(posedge fast_clock) if (freset | RUN & ( RL33u37 | RL32u36      ))  RL33u37 <= `tFCQ ~(freset | RL33u37);
always @(posedge fast_clock) if (freset | RUN & ( RL34u38 | RL33u37      ))  RL34u38 <= `tFCQ ~(freset | RL34u38);
always @(posedge fast_clock) if (freset | RUN & ( RH32u36 | tail_H | RH35))  RH32u36 <= `tFCQ ~(freset | RH32u36);
always @(posedge fast_clock) if (freset | RUN & ( RH33u37 | RH32u36      ))  RH33u37 <= `tFCQ ~(freset | RH33u37);
always @(posedge fast_clock) if (freset | RUN & ( RH34u38 | RH33u37      ))  RH34u38 <= `tFCQ ~(freset | RH34u38);

always @(posedge fast_clock) if (freset | RUN & (head_M | MIX  & RL03[32]))  MIX     <= `tFCQ ~(freset | MIX & RL03[32]);

//===============================================================================================================================
// Pipeline control bits that are selects for multiplexors with high fanouts

always @(posedge fast_clock) if (freset | RUN & ( RL0x | e_head_L))  RL0x <= `tFCQ ~( freset | RL0x );
always @(posedge fast_clock) if (freset | RUN & ( RL0y | RL0x    ))  RL0y <= `tFCQ ~( freset | RL0y );
always @(posedge fast_clock) if (freset | RUN & ( RL0z | RL0y    ))  RL0z <= `tFCQ ~( freset | RL0z );

`ifdef   FPGA
for (g=0; g<33; g=g+1) begin : rl0x
   always @(posedge fast_clock) if (freset || RUN && ( RL00[32] || RL0z     )) RL00[g]   <= `tCQ ~(freset | RL00[g]);
   always @(posedge fast_clock) if (freset || RUN && ( RL01[32] || RL00[32] )) RL01[g]   <= `tCQ ~(freset | RL01[g]);
   always @(posedge fast_clock) if (freset || RUN && ( RL02[32] || RL01[32] )) RL02[g]   <= `tCQ ~(freset | RL02[g]);
   always @(posedge fast_clock) if (freset || RUN && ( RL03[32] || RL02[32] )) RL03[g]   <= `tCQ ~(freset | RL03[g]);
   end
`else
wire  ck_RL00,ck_RL01,ck_RL02,ck_RL03;
preicg iCG0 (.ECK(ck_RL00), .CK(fast_clock), .E(freset | RUN & ( RL00[32] | RL0z     )), .SE(1'b0));
preicg iCG1 (.ECK(ck_RL01), .CK(fast_clock), .E(freset | RUN & ( RL01[32] | RL00[32] )), .SE(1'b0));
preicg iCG2 (.ECK(ck_RL02), .CK(fast_clock), .E(freset | RUN & ( RL02[32] | RL01[32] )), .SE(1'b0));
preicg iCG3 (.ECK(ck_RL03), .CK(fast_clock), .E(freset | RUN & ( RL03[32] | RL02[32] )), .SE(1'b0));

for (g=0; g<33; g=g+1) begin : rl0x
   DFFQ_X4N_A9PP84TSL_C14 iRL0 (.Q(RL00[g]), .CK(ck_RL00), .D(~(freset | RL00[g])));
   DFFQ_X4N_A9PP84TSL_C14 iRL1 (.Q(RL01[g]), .CK(ck_RL01), .D(~(freset | RL01[g])));
   DFFQ_X4N_A9PP84TSL_C14 iRL2 (.Q(RL02[g]), .CK(ck_RL02), .D(~(freset | RL02[g])));
   DFFQ_X4N_A9PP84TSL_C14 iRL3 (.Q(RL03[g]), .CK(ck_RL03), .D(~(freset | RL03[g])));
   end
`endif
//===============================================================================================================================
// Selects for b0_LA, b0_LB, b0_LC, b0_LD, b0_HA, b0_HB, b0_HC, b0_HD

always @(posedge fast_clock) if (freset | RUN & ( RLHx | e_head_L | e_head_H)) RLHx  <= `tFCQ ~( freset | RLHx );
always @(posedge fast_clock) if (freset | RUN & ( RLHy | RLHx               )) RLHy  <= `tFCQ ~( freset | RLHy );
always @(posedge fast_clock) if (freset | RUN & ( RLHz | RLHy               )) RLHz  <= `tFCQ ~( freset | RLHz );

`ifdef   FPGA
for (g=0; g<17; g=g+1) begin : rlhx
   always @(posedge fast_clock) if (freset || RUN && ( RLH0[16] || RLHz     )) RLH0[g]  <= `tCQ ~(freset | RLH0[g]);
   always @(posedge fast_clock) if (freset || RUN && ( RLH1[16] || RLH0[16] )) RLH1[g]  <= `tCQ ~(freset | RLH1[g]);
   always @(posedge fast_clock) if (freset || RUN && ( RLH2[16] || RLH1[16] )) RLH2[g]  <= `tCQ ~(freset | RLH2[g]);
   always @(posedge fast_clock) if (freset || RUN && ( RLH3[16] || RLH2[16] )) RLH3[g]  <= `tCQ ~(freset | RLH3[g]);
   end
`else
wire  ck_RLH0,ck_RLH1,ck_RLH2,ck_RLH3;
preicg iCG4 (.ECK(ck_RLH0), .CK(fast_clock), .E(freset | RUN & ( RLH0[16] | RLHz     )), .SE(1'b0));
preicg iCG5 (.ECK(ck_RLH1), .CK(fast_clock), .E(freset | RUN & ( RLH1[16] | RLH0[16] )), .SE(1'b0));
preicg iCG6 (.ECK(ck_RLH2), .CK(fast_clock), .E(freset | RUN & ( RLH2[16] | RLH1[16] )), .SE(1'b0));
preicg iCG7 (.ECK(ck_RLH3), .CK(fast_clock), .E(freset | RUN & ( RLH3[16] | RLH2[16] )), .SE(1'b0));

for (g=0; g<17; g=g+1) begin : rlhx
   DFFQ_X4N_A9PP84TSL_C14 iRL0 (.Q(RLH0[g]), .CK(ck_RLH0), .D(~(freset | RLH0[g])));
   DFFQ_X4N_A9PP84TSL_C14 iRL1 (.Q(RLH1[g]), .CK(ck_RLH1), .D(~(freset | RLH1[g])));
   DFFQ_X4N_A9PP84TSL_C14 iRL2 (.Q(RLH2[g]), .CK(ck_RLH2), .D(~(freset | RLH2[g])));
   DFFQ_X4N_A9PP84TSL_C14 iRL3 (.Q(RLH3[g]), .CK(ck_RLH3), .D(~(freset | RLH3[g])));
   end
`endif

//===============================================================================================================================

//(* siart MULTICYCLE *)
reg     SMIXs;

wire        en_SMIXs = e_head_M & YSAVED & ~INTERPOLATE | SMIXs & (CYCLE==5);

always @(posedge fast_clock) if (freset | RUN &  en_SMIXs ) SMIXs <= `tFCQ ~(freset | SMIXs );

//===============================================================================================================================
// Register Files
//===============================================================================================================================

always @(posedge fast_clock) begin
   // Move r0_X to r0_Y
   if (RM00)   {r0_Y[16], r0_Y[20], r0_Y[24], r0_Y[28], r0_Y[00], r0_Y[04], r0_Y[08], r0_Y[12]} <= `tFCQ
               {r0_X[16], r0_X[20], r0_X[24], r0_X[28], r0_X[00], r0_X[04], r0_X[08], r0_X[12]};
   if (RM01)   {r0_Y[21], r0_Y[25], r0_Y[29], r0_Y[17], r0_Y[05], r0_Y[09], r0_Y[13], r0_Y[01]} <= `tFCQ
               {r0_X[21], r0_X[25], r0_X[29], r0_X[17], r0_X[05], r0_X[09], r0_X[13], r0_X[01]};
   if (RM02)   {r0_Y[26], r0_Y[30], r0_Y[18], r0_Y[22], r0_Y[10], r0_Y[14], r0_Y[02], r0_Y[06]} <= `tFCQ
               {r0_X[26], r0_X[30], r0_X[18], r0_X[22], r0_X[10], r0_X[14], r0_X[02], r0_X[06]};
   if (RM03)   {r0_Y[31], r0_Y[19], r0_Y[23], r0_Y[27], r0_Y[15], r0_Y[03], r0_Y[07], r0_Y[11]} <= `tFCQ
               {r0_X[31], r0_X[19], r0_X[23], r0_X[27], r0_X[15], r0_X[03], r0_X[07], r0_X[11]};
   end

always @(posedge fast_clock) begin
   // Load Inputs \ Read Regs
   if (fast_shift)
      for (i=0; i<32; i=i+1)
         if (i==31)  r0_X[i] <= `tFCQ  fast_load ? IN[31:0] : r0_X[0];
         else        r0_X[i] <= `tFCQ  r0_X[i+1];
   // Load from Memory so we can interpolate
   if (RM00)   {r0_X[16], r0_X[20], r0_X[24], r0_X[28], r0_X[00], r0_X[04], r0_X[08], r0_X[12]} <= `tFCQ  WMo[255:000]; //fo=256
   if (RM01)   {r0_X[21], r0_X[25], r0_X[29], r0_X[17], r0_X[05], r0_X[09], r0_X[13], r0_X[01]} <= `tFCQ  WMo[255:000]; //fo=256
   if (RM02)   {r0_X[26], r0_X[30], r0_X[18], r0_X[22], r0_X[10], r0_X[14], r0_X[02], r0_X[06]} <= `tFCQ  WMo[255:000]; //fo=256
   if (RM03)   {r0_X[31], r0_X[19], r0_X[23], r0_X[27], r0_X[15], r0_X[03], r0_X[07], r0_X[11]} <= `tFCQ  WMo[255:000]; //fo=256
   // Load XOR results
   if (RL00[32])        {r0_X[00], r0_X[04], r0_X[08], r0_X[12]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
   if (RL01[32])        {r0_X[05], r0_X[09], r0_X[13], r0_X[01]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
   if (RL02[32])        {r0_X[10], r0_X[14], r0_X[02], r0_X[06]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
   if (RL03[32])        {r0_X[15], r0_X[03], r0_X[07], r0_X[11]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
   if (RH00)            {r0_X[16], r0_X[20], r0_X[24], r0_X[28]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
   if (RH01)            {r0_X[21], r0_X[25], r0_X[29], r0_X[17]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
   if (RH02)            {r0_X[26], r0_X[30], r0_X[18], r0_X[22]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
   if (RH03)            {r0_X[31], r0_X[19], r0_X[23], r0_X[27]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
   // Mixing
   if (RL00[32] && MIX) {r0_X[16], r0_X[20], r0_X[24], r0_X[28]} <= `tFCQ  {~b0_XA, ~b0_XB, ~b0_XC, ~b0_XD };           //fo=128
   if (RL01[32] && MIX) {r0_X[21], r0_X[25], r0_X[29], r0_X[17]} <= `tFCQ  {~b0_XA, ~b0_XB, ~b0_XC, ~b0_XD };           //fo=128
   if (RL02[32] && MIX) {r0_X[26], r0_X[30], r0_X[18], r0_X[22]} <= `tFCQ  {~b0_XA, ~b0_XB, ~b0_XC, ~b0_XD };           //fo=128
   if (RL03[32] && MIX) {r0_X[31], r0_X[19], r0_X[23], r0_X[27]} <= `tFCQ  {~b0_XA, ~b0_XB, ~b0_XC, ~b0_XD };           //fo=128
   // Load Low Salsa Results
   if (RL34)             r0_X[00]                                <= `tFCQ   r2_ADD;                                     //fo= 32
   if (RL35)            {r0_X[05], r0_X[04]                    } <= `tFCQ  {r2_ADD, r3_ADD                };            //fo= 64
   if (RL36)            {r0_X[10], r0_X[09], r0_X[08]          } <= `tFCQ  {r2_ADD, r3_ADD, r4_ADD        };            //fo= 96
   if (RL37)            {r0_X[15], r0_X[14], r0_X[13], r0_X[12]} <= `tFCQ  {r2_ADD, r3_ADD, r4_ADD, r5_ADD};            //fo=128
   if (RL38)            {          r0_X[03], r0_X[02], r0_X[01]} <= `tFCQ  {        r3_ADD, r4_ADD, r5_ADD};            //fo= 96
   if (RL39)            {                    r0_X[07], r0_X[06]} <= `tFCQ  {                r4_ADD, r5_ADD};            //fo= 64
   if (RL40)                                           r0_X[11]  <= `tFCQ                           r5_ADD;             //fo= 32
   // Load High Salsa Results
   if (RH34)             r0_X[16]                                <= `tFCQ   r2_ADD;                                     //fo= 32
   if (RH35)            {r0_X[21], r0_X[20]                    } <= `tFCQ  {r2_ADD, r3_ADD                };            //fo= 64
   if (RH36)            {r0_X[26], r0_X[25], r0_X[24]          } <= `tFCQ  {r2_ADD, r3_ADD, r4_ADD        };            //fo= 96
   if (RH37)            {r0_X[31], r0_X[30], r0_X[29], r0_X[28]} <= `tFCQ  {r2_ADD, r3_ADD, r4_ADD, r5_ADD};            //fo=128
   if (RH38)            {          r0_X[19], r0_X[18], r0_X[17]} <= `tFCQ  {        r3_ADD, r4_ADD, r5_ADD};            //fo= 96
   if (RH39)            {                    r0_X[23], r0_X[22]} <= `tFCQ  {                r4_ADD, r5_ADD};            //fo= 64
   if (RH40)                                           r0_X[27]  <= `tFCQ                           r5_ADD;             //fo= 32
   end

//===============================================================================================================================
// Main Datapath to execute pipeline

// Select Low  Buffer entries
assign   {b0_LA,b0_LB,b0_LC,b0_LD}  =
            {8{RLH0[15:0]}} & {r0_X[00], r0_X[04], r0_X[08], r0_X[12]}                                                  //fo=128
         |  {8{RLH1[15:0]}} & {r0_X[05], r0_X[09], r0_X[13], r0_X[01]}                                                  //fo=128
         |  {8{RLH2[15:0]}} & {r0_X[10], r0_X[14], r0_X[02], r0_X[06]}                                                  //fo=128
         |  {8{RLH3[15:0]}} & {r0_X[15], r0_X[03], r0_X[07], r0_X[11]};                                                 //fo=128

// Select High Buffer entries
assign   {b0_HA,b0_HB,b0_HC,b0_HD}  =
            {8{RLH0[15:0]}} & {r0_X[16], r0_X[20], r0_X[24], r0_X[28]}                                                  //fo=128
         |  {8{RLH1[15:0]}} & {r0_X[21], r0_X[25], r0_X[29], r0_X[17]}                                                  //fo=128
         |  {8{RLH2[15:0]}} & {r0_X[26], r0_X[30], r0_X[18], r0_X[22]}                                                  //fo=128
         |  {8{RLH3[15:0]}} & {r0_X[31], r0_X[19], r0_X[23], r0_X[27]};                                                 //fo=128

// As the next operation is an XOR (which is commutative) we do NOT swap sides again so, Y holds the saved value of X from
// the last iteration and X hold the interpolated value replacing the Memory data
assign   Saved[255:0] =
            {8{RL00[31:0]}} & {r0_Y[16], r0_Y[20], r0_Y[24], r0_Y[28], r0_Y[00], r0_Y[04], r0_Y[08], r0_Y[12]}          //fo=256
         |  {8{RL01[31:0]}} & {r0_Y[21], r0_Y[25], r0_Y[29], r0_Y[17], r0_Y[05], r0_Y[09], r0_Y[13], r0_Y[01]}          //fo=256
         |  {8{RL02[31:0]}} & {r0_Y[26], r0_Y[30], r0_Y[18], r0_Y[22], r0_Y[10], r0_Y[14], r0_Y[02], r0_Y[06]}          //fo=256
         |  {8{RL03[31:0]}} & {r0_Y[31], r0_Y[19], r0_Y[23], r0_Y[27], r0_Y[15], r0_Y[03], r0_Y[07], r0_Y[11]};         //fo=256

assign   MemData[255:0] =
           ~(SMIXs ?  Saved[255:0] :  WMo[255:000]);                                                                    //fo=256

// XOR against memory when mixing
assign   {b0_XA, b0_XB, b0_XC, b0_XD}  =  {b0_HA, b0_HB, b0_HC, b0_HD} ^  MemData[255:128];              // XOR high and mem
assign   {b0_YA, b0_YB, b0_YC, b0_YD}  =  {b0_LA, b0_LB, b0_LC, b0_LD} ^  MemData[127:000];              // XOR low  and mem
assign   {b0_ZA, b0_ZB, b0_ZC, b0_ZD}  =  {b0_XA, b0_XB, b0_XC, b0_XD} ^ {b0_YA, b0_YB, b0_YC, b0_YD};   // XOR High and Low

// Select buffer entry for digest update
assign   {b1_DA, b2_DB, b3_DC, b0_DD}  =
            {128{ RL32u36 }} & {r0_X[03], r0_X[02], r0_X[01], r0_X[00]}                                                 //fo=128
         |  {128{ RL33u37 }} & {r0_X[04], r0_X[07], r0_X[06], r0_X[05]}                                                 //fo=128
         |  {128{ RL34u38 }} & {r0_X[09], r0_X[08], r0_X[11], r0_X[10]}                                                 //fo=128
         |  {128{ RL35    }} & {r0_X[14], r0_X[13], r0_X[12], r0_X[15]}                                                 //fo=128
         |  {128{ RH32u36 }} & {r0_X[19], r0_X[18], r0_X[17], r0_X[16]}                                                 //fo=128
         |  {128{ RH33u37 }} & {r0_X[20], r0_X[23], r0_X[22], r0_X[21]}                                                 //fo=128
         |  {128{ RH34u38 }} & {r0_X[25], r0_X[24], r0_X[27], r0_X[26]}                                                 //fo=128
         |  {128{ RH35    }} & {r0_X[30], r0_X[29], r0_X[28], r0_X[31]};                                                //fo=128

//===============================================================================================================================
// Work Memory Interface
reg           rqWM;                             // Request Working Memory Request -- Highest Priority
reg            wWM;                             // Write   Working Memory
reg  [  3:0]   eWM;                             // Enable  Working Memory Per bank
reg  [ 15:7]   aWM;                             // Address Working Memory Read/Write
reg  [  3:0]   bWM;                             // Bank    Working Memory bank select

always @(posedge fast_clock) if (gate) rqWM     <= `tFCQ ~freset & (prep |  (high & (CYCLE==28) & (ITERATION[0]|MX)))   // set
                                                    |    ~freset &  rqWM & ~( low & (CYCLE==09));                       // reset
always @(posedge fast_clock) if (gate) wWM      <= `tFCQ ~MX    & ~ITERATION[0];
always @(posedge fast_clock) if (gate) eWM[3:0] <= `tFCQ {eWM[2:0], head_eWM};

always @(posedge fast_clock) if (gate && ld_wrADR) aWM[15:7] <= `tFCQ ITERATION[9:1];
                        else if (gate && ld_rdADR) aWM[15:7] <= `tFCQ    b1_ADR[9:1];

// Register File Outputs
assign   iWM[255:0]     =  {b0_HA,b0_HB,b0_HC,b0_HD, b0_LA,b0_LB,b0_LC,b0_LD};

// Collate                  270   269:266   265  264:256    255:0
assign   smosi[270:0]   =  {rqWM, eWM[3:0], wWM, aWM[15:7], iWM[255:0]};

// Decollate
assign   {WMk[3:0],WMo[255:0]}   = smiso[259:0];

// Prep for Execution Pipeline ==================================================================================================

reg E_PRIME, E_head_P, E_tail;      // Registered Early Control Signals for the pipeline

always @(posedge fast_clock) if (gate) E_PRIME  <= `tFCQ ~freset & (            (RL0y | RH0y)
                                                                 |  E_PRIME & ~((low  | high) & (CYCLE==2)));
always @(posedge fast_clock) if (gate) E_head_P <= `tFCQ  e_head_P;
always @(posedge fast_clock) if (gate) E_tail   <= `tFCQ  e_tail;

A2_Xsalsa20_8_pipe i_SPPL (
   // Data Output
   .b1_ADR     (b1_ADR[09:0]),
   .r2_ADD     (r2_ADD[31:0]), .r3_ADD (r3_ADD[31:0]), .r4_ADD (r4_ADD[31:0]), .r5_ADD (r5_ADD[31:0]),
   // Data Input
   .b0_ZA      (b0_ZA [31:0]), .b0_ZB  (b0_ZB [31:0]), .b0_ZC  (b0_ZC [31:0]), .b0_ZD  (b0_ZD [31:0]),
   .b1_DA      (b1_DA [31:0]), .b2_DB  (b2_DB [31:0]), .b3_DC  (b3_DC [31:0]), .b0_DD  (b0_DD [31:0]),
   // Control
   .E_PRIME    (E_PRIME),
   .E_head_P   (E_head_P),
   .E_tail     (E_tail),
   .fast_start (fast_start),
   // Global
   .gate       (gate),
   .fast_clock (fast_clock)
   );

endmodule

// Execution Pipeline ===========================================================================================================

module A2_Xsalsa20_8_pipe (
   output reg  [31:0]   r2_ADD,    r3_ADD,    r4_ADD,    r5_ADD,
   output wire [ 9:0]   b1_ADR,
   input  wire [31:0]   b0_ZA,     b0_ZB,     b0_ZC,     b0_ZD,
   input  wire [31:0]   b0_DD,     b1_DA,     b2_DB,     b3_DC,
   input  wire          E_head_P,
   input  wire          E_tail,
   input  wire          E_PRIME,
   input  wire          fast_start,
   input  wire          gate,
   input  wire          fast_clock
   );

// Control
reg   [39:32]  PP;
reg            PRIME;
reg            P00,  P01,  P02,  P03;
reg            P0P1,P0P1P2,P1P2P3;
reg   add2dgstA, add2dgstB, add2dgstC, add2dgstD, add2dgstD2;

always @(posedge fast_clock) if (gate) {P03, P02, P01, P00}    <= `tFCQ {P02,P01,P00,E_head_P};
always @(posedge fast_clock) if (gate)  P0P1                   <= `tFCQ  P00 |       E_head_P;
always @(posedge fast_clock) if (gate)  P0P1P2                 <= `tFCQ  P01 | P00 | E_head_P;
always @(posedge fast_clock) if (gate)  P1P2P3                 <= `tFCQ  P02 | P01 | P00;
always @(posedge fast_clock) if (gate)  PP[39:32]              <= `tFCQ {PP[38:32],E_tail};
always @(posedge fast_clock) if (gate)  PRIME                  <= `tFCQ  E_PRIME;

always @(posedge fast_clock) if (fast_start || gate &&  (E_tail || PP[35]))  add2dgstD  <= `tFCQ  E_tail;
always @(posedge fast_clock) if (fast_start || gate &&  (PP[32] || PP[36]))  add2dgstA  <= `tFCQ  PP[32];
always @(posedge fast_clock) if (fast_start || gate &&  (PP[33] || PP[37]))  add2dgstB  <= `tFCQ  PP[33];
always @(posedge fast_clock) if (fast_start || gate &&  (PP[34] || PP[38]))  add2dgstC  <= `tFCQ  PP[34];
always @(posedge fast_clock) if (fast_start || gate &&  (PP[35] || PP[39]))  add2dgstD2 <= `tFCQ  PP[35];

// Pipeline Registers   *****[ DYNAMIC LATCHES ]*****
reg   [31:0]   r1_A, r1_B, r1_C, r1_D, r1_E,
               r2_A, r2_B, r2_C, r2_D,
               r3_A, r3_B, r3_C, r3_D,
               r4_A, r4_B, r4_C, r4_D,
                     r5_B, r5_C, r5_D,
                           r6_C;

// Pipeline Arithmetic
wire  [31:0]   b1_ARX, b2_ARX, b3_ARX, b4_ARX,        // Results of ARX computations
               b1_ADD, b2_ADD, b3_ADD, b4_ADD;        // Results of ADDs

// Pipeline Input Multiplexing
wire  [31:0]   n0_B  =  !P03                   ?   b0_ZB // New input
                     :                             r5_D, // Registered feedback

               n0_C  =  !P1P2P3                ?   b0_ZC // New input
                     :   P03                   ?   r5_C  // Registered feedback
                     :                             r6_C, // Registered feedback

               n0_D  =  !add2dgstD && !P1P2P3  ?   b0_ZD // New input
                     :   add2dgstD             ?   b0_DD // Digest
                     :                             r5_B; // Registered feedback

//                                                                       Unregistered feedback :  higher mux
always @(posedge fast_clock) if (gate)      r1_A     <= `tFCQ !PRIME               ?  b4_ARX   :  b0_ZA;
always @(posedge fast_clock) if (gate)      r1_B     <= `tFCQ  P0P1P2              ?  b3_ARX   :  n0_B;
always @(posedge fast_clock) if (gate)      r1_C     <= `tFCQ  P0P1                ?  b2_ARX   :  n0_C;
always @(posedge fast_clock) if (gate)      r1_D     <= `tFCQ !add2dgstD &&  P00   ?  b1_ARX   :  n0_D;
always @(posedge fast_clock) if (gate && add2dgstD) r1_E     <= `tFCQ  P00         ?  b1_ARX   :  r5_B;

assign   b1_ADD[31:00]  =  r1_A[31:00] +  r1_D[31:00],
         b1_ARX[31:00]  =  r1_B[31:00] ^ {b1_ADD[24:00],b1_ADD[31:25]};

assign   b1_ADR[9:0]    =  b1_ADD[9:0];

// Pipeline Stage 2 -------------------------------------------------------------------------------------------------------------

always @(posedge fast_clock) if (gate && add2dgstA)   r2_ADD   <= `tFCQ  b1_ADD;
always @(posedge fast_clock) if (gate)                r2_B     <= `tFCQ  add2dgstA  ?  r1_B  :  b1_ARX;
always @(posedge fast_clock) if (gate)                r2_C     <= `tFCQ  r1_C;
always @(posedge fast_clock) if (gate)                r2_D     <= `tFCQ  add2dgstA  ?  r1_E  :  r1_D;
always @(posedge fast_clock) if (gate)                r2_A     <= `tFCQ  add2dgstA  ?  b1_DA :  r1_A;

assign   b2_ADD[31:00]  =  r2_A[31:00] +  r2_B[31:00],
         b2_ARX[31:00]  =  r2_C[31:00] ^ {b2_ADD[22:00],b2_ADD[31:23]};

// Pipeline Stage 3 -------------------------------------------------------------------------------------------------------------

always @(posedge fast_clock) if (gate && add2dgstB)   r3_ADD   <= `tFCQ  b2_ADD;
always @(posedge fast_clock) if (gate)                r3_C     <= `tFCQ  add2dgstB   ?  r2_C  :  b2_ARX;
always @(posedge fast_clock) if (gate)                r3_D     <= `tFCQ  r2_D;
always @(posedge fast_clock) if (gate)                r3_A     <= `tFCQ  r2_A;
always @(posedge fast_clock) if (gate)                r3_B     <= `tFCQ  add2dgstB   ?  b2_DB :  r2_B;

assign   b3_ADD[31:00]  =  r3_C[31:00] +  r3_B[31:00],
         b3_ARX[31:00]  =  r3_D[31:00] ^ {b3_ADD[18:00],b3_ADD[31:19]};

// Pipeline Stage 4 -------------------------------------------------------------------------------------------------------------

always @(posedge fast_clock) if (gate && add2dgstC)   r4_ADD   <= `tFCQ  b3_ADD;
always @(posedge fast_clock) if (gate)                r4_D     <= `tFCQ  add2dgstC   ?  r3_D  :  b3_ARX;
always @(posedge fast_clock) if (gate)                r4_A     <= `tFCQ  r3_A;
always @(posedge fast_clock) if (gate)                r4_B     <= `tFCQ  r3_B;
always @(posedge fast_clock) if (gate)                r4_C     <= `tFCQ  add2dgstC   ?  b3_DC :  r3_C;

assign   b4_ADD[31:00]  =  r4_D[31:00] +  r4_C[31:00],
         b4_ARX[31:00]  =  r4_A[31:00] ^ {b4_ADD[13:00],b4_ADD[31:14]};

// Pipeline Stage 5 -------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (gate && add2dgstD2)  r5_ADD   <= `tFCQ  b4_ADD;
always @(posedge fast_clock) if (gate && P0P1P2)      r5_B     <= `tFCQ  r4_B;
always @(posedge fast_clock) if (gate && P0P1  )      r5_C     <= `tFCQ  r4_C;
always @(posedge fast_clock) if (gate && P00   )      r5_D     <= `tFCQ  r4_D;

// Pipeline Stage 6 -------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (gate && P01   )      r6_C     <= `tFCQ  r5_C;

endmodule

// ==============================================================================================================================

`ifndef   PREICG
`define   PREICG
module preicg (
   output wire ECK,
   input  wire E, SE,
   input  wire CK
   );
reg    q;

always @* if (!CK) q = (E | SE);

assign ECK = CK & q;

endmodule
`endif
// ==============================================================================================================================

module DFFQ_X4N_A9PP84TSL_C14 (
   output reg  Q,
   input  wire D,
   input  wire CK
   );

always @(posedge CK) Q <= `tFCQ D;
endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_salsa20_8           // Add this as a Simulator command line define

`include "..//RTL//NEWPORT_IO_addresses.tbh"
module t;

// Stimulii
reg  [47:0]              imosi;
reg                      reset;
reg                      slow_clock;
reg                      fast_clock;
reg                      SalsaOn, MPPon;
reg  [`WMOSI_WIDTH -1:0] wmosi;         // Neural Net Port
reg  [`DMOSI_WIDTH -1:0] amosi;         // Tile Port
reg  [`DMOSI_WIDTH -1:0] bmosi;         // A2S Data Port
reg                      bacpt;
// Interconnect
wire [`SMOSI_WIDTH -1:0] smosi;
wire [`SMISO_WIDTH -1:0] smiso;
// Responses
wire [32:0]              imiso;
wire [`WMISO_WIDTH -1:0] wmiso;         // Neural Net Port   -- fast clock
wire [`DMISO_WIDTH+31:0] bmiso;         // Tile Port         -- slow clock, pre-decoded bmosi
wire [`DMISO_WIDTH -1:0] amiso;         // A2S Data Port     -- slow clock
wire                     done;


A2_Xsalsa20_8I_CoP i_Salsa (
   .imiso      (imiso),       // Slave data out {               IOk, IOo[31:0]}
   .imosi      (imosi),       // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   .done       (done ),       // Interrupt : Algorithm done
   // SRAM Interface
   .smosi      (smosi),       // Salsa Master output to  SRAM slave input  (WM: Working memory)
   .smiso      (smiso),       // Salsa Master input from SRAM slave output (WM: Working memory)
   // Global
   .SalsaOn    (SalsaOn),
   .reset      (reset),       // It takes 10 cycles to initalize from assertion of reset before first 'load' !!!
   .slow_clock (slow_clock),
   .fast_clock (fast_clock)
   );


wRAM #(
   .DEPTH( 2048),             // CoP DEPTH = 2048
   .WIDTH(  256),             // CoP WIDTH =  256
   .BASE (13'h1000)              // Base Address
// .tACC (`tACC)              // Access time in clock cycles     -- supports up to 7 wait states
// .tCYC (`tCYC)              // RAM cycle time in clock cycles
   ) mut (
   .smiso(smiso),             // Salsa Port : 260 {      sack[3:0],        sbrd[255:0]}
   .wmiso(wmiso),             // SIMD  Port : 130 {WMg,  WMk, WMo[127:0]}
   .amiso(amiso),             // RAM   Port :  34 {agnt, aack,              ardd[31:0]}
   .bmiso(bmiso),             // RAM   Port :  66 {bgnt, back, brd0[31:0],  brd1[31:0]}

   .smosi(smosi),             // Salsa Port : 272 {sreq, sen[3:0],swr, sadr[9:0], swrd[255:0]}
   .wmosi(wmosi),             // SIMD  Port : 290 {eWM,  wWM,  iWM[127:0], mWM[127:0], aWM[31:0]}
   .amosi(amosi),             // RAM   Port :  50 {areq, awr,  awrd[31:0], aadr[17:2]}
   .bmosi(bmosi),             // RAM   Port :  50 {breq, bwr,  bwrd[31:0], badr[17:2]}
   .bacpt(bacpt),
   .SalsaOn(SalsaOn),         // Salsa is active, all other accesses fail -- no acknowledge
   .reset(reset),
   .fast_clock(fast_clock),
   .tile_clock(slow_clock)
   );




// Test Stimulii
reg  [1023:0]  stim, resp, xpct;
reg  [  31:0]  flag;
reg            load;
reg            read;
reg            gate;
// Measurement
integer        cycles, i,j;

// Test Sequence ---------------------------------------------------------------------------------------------------------------

initial begin
   fast_clock = 1'b1;
   forever begin
      `fastcycle fast_clock = ~fast_clock;
      end
   end

initial begin
   slow_clock = 1'b1;
   forever begin
      `slowcycle slow_clock = ~slow_clock;
      end
   end

always @(posedge fast_clock)
   if (reset)     cycles = 0;
   else if (gate) cycles = cycles +1;

// Test Procedure
initial begin
   $display(" Register Based Execution Pipeline: NO CONTROL FAN-OUT");
   nmosi    = 260'h0;
   amosi    =  66'h0;
   bmosi    =  66'h0;
   MPPon    =  1'b0;
   bacpt    =  1'b0;
   reset    =  1'b0;
   gate     =  1'b0;
   read     =  1'b0;
   load     =  1'b0;
   stim     = {256'h0c809ebc35e7e5b9308736de918332c80c2eb1fcc348c7c76b5daa0ee44de053,
               256'h6d04b6621cb651959328032861663eabbf005a0ca327784bb813398e7dd42df6,
               256'h917498556eb884b7899f30bb956a5647fcb5076e2fc4f9e579f1e7fcda315878,
               256'h6155e74c3c2089d26ff087e9e883b1e55174c638fcff36fb0793d68d5d9ec5df};

   xpct     = {256'hb8dc6d17a70820da7795cdcfc42520c502c1f36a95dfe383b706642ab1c70f4d,
               256'hd3a36645b0bf693b869274126ab91d19f4a6bcfc588a674ad56c223817edfc52,
               256'hd60adf34d792e0662797a268c1eebae593a97f4b19c2fa4fea973768f4e6db64,
               256'h1a693b27379e14f043d980c101409b1bdd9e7aab2550adcdd87b8a02861550dc};
   // Reset
   repeat (10 ) @(posedge slow_clock); `tFCQ
   reset    =  1'b1;
   repeat (100) @(posedge slow_clock); `tFCQ
   reset    =  1'b0;
   repeat (10 ) @(posedge slow_clock); `tFCQ
   gate     =  1'b1;

   // Load job to Xsalsa
   // Enable WM access
   imosi    =  {3'b110,32'h00000001,  `aTILE_XSALSA_CTRL};     @(posedge slow_clock); `tFCQ;      // Enable Salsa

   imosi    =  {3'b000,32'h00000000,  `aTILE_XSALSA_XBUF};     @(posedge slow_clock); `tFCQ;      // No Access
   for (i=0; i<32; i=i+1) begin
      imosi =  {3'b110,stim[32*i+:32],`aTILE_XSALSA_XBUF};     @(posedge slow_clock); `tFCQ;      // Write to Xbuf
      end
   imosi    =  {3'b000,32'h00000000,  `aTILE_XSALSA_XBUF};     @(posedge slow_clock); `tFCQ;      // end write

   imosi    =  {3'b110,32'h00000000,  `aTILE_XSALSA_STRT_RDY}; @(posedge slow_clock); `tFCQ;      // Start

   imosi    =  {3'b101,32'h00000000,  `aTILE_XSALSA_STRT_RDY}; @(posedge slow_clock) flag <= `tFCQ imiso[31:0];  // Check flag
   imosi    =  {3'b000,32'h00000000,  `aTILE_XSALSA_XBUF};     @(posedge slow_clock); `tFCQ;

   // Wait for algorithm to send 'done' interrupt
   while ( !done ) begin
      @(posedge slow_clock); `tFCQ;
      end

   // Check the done flag
   @(posedge slow_clock); `tFCQ;
   imosi =     {3'b101,32'h00000000,`aTILE_XSALSA_STRT_RDY};   @(posedge slow_clock) flag <= `tFCQ imiso[31:0];  // Check flag
   imosi =     {3'b000,32'h00000000,`aTILE_XSALSA_XBUF};       @(posedge slow_clock); `tFCQ;

   // Read back the results
   for (i=0; i<32; i=i+1) begin
      imosi =  {3'b101,32'h00000000,`aTILE_XSALSA_XBUF};       @(posedge slow_clock) resp[32*i+:32] <= `tFCQ imiso[31:0];
      imosi =  {3'b000,32'h00000000,`aTILE_XSALSA_XBUF};       @(posedge slow_clock); `tFCQ;
      end
   imosi =     {3'b000,32'h00000000,`aTILE_XSALSA_XBUF};       @(posedge slow_clock); `tFCQ;

   // Turn off the clocks
   @(posedge slow_clock); `tFCQ
   gate      =  1'b0;
   @(posedge slow_clock); `tFCQ

   // Display and check the result
   $display("\n Response = %h", resp[1023:768]);
   $display(  "            %h", resp[ 767:512]);
   $display(  "            %h", resp[ 511:256]);
   $display(  "            %h", resp[ 255:0  ]);
   if (resp == xpct) $display("\n Algorithm is correct!!! \n");
   else              $display("\n ERROR - algorithm failed\n");
   $display(  " cycles = %d",cycles);
   $display("\n DONE  \n");
   $finish;
   end

endmodule

`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
