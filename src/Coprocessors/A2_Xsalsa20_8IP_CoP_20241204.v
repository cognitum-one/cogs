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

`define  REORG

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
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_defines.vh"
`else
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/NEWPORT_defines.vh"
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

//(* SiART  fast_clock *)
reg   [31:0]   r0_X [0:31];
reg   [31:0]   r0_Y [0:31];                     // For Interpolation
reg            fast_load, fast_read, fast_stop, fast_start, fast_fini, fast_shift;

//(* SiART  slow_clock *)
reg   [31:0]   IN,OUT;
reg   [ 1:0]   CTRL; //Control Reg:   0: SalsaOn (WM off limits) 1: Add a Waitstate to r0_X reads
reg            slow_load, slow_read, slow_stop, slow_start, slow_fini;

// Datapath Nets

wire [255:0]   Saved,  MemData;                 // For interpolation
wire [127:0]   Readata,Restore;

wire  [31:0]   r2_ADD,r3_ADD,r4_ADD,r5_ADD,     // Feedback registers for writes to r0_X[nn]
               b0_HA, b0_HB, b0_HC, b0_HD,      // Selected elements from HI buffer
               b0_LA, b0_LB, b0_LC, b0_LD,      // Selected elements from LO buffer
               b0_UA, b0_UB, b0_UC, b0_UD,      // XOR of memory and selected elements from HI buffer
               b0_VA, b0_VB, b0_VC, b0_VD,      // XOR of memory and selected elements from LO buffer
               b0_XA, b0_XB, b0_XC, b0_XD,      // XOR of memory and selected elements from HI buffer
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
`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/NEWPORT_IO_addresses.vh"

localparam  LG2REGS  =  `LG2(NUM_REGS);

//(* SiART  slow_clock *)
wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
wire           idle;
reg            RXk;

// Decollate imosi
assign   {cIO[2:0], iIO[31:0], aIO[12:0]} = imosi;

//(* SiART  slow_clock *)
wire                 in_range = aIO[12:LG2REGS] == BASE[12:LG2REGS];
wire  [NUM_REGS-1:0]   decode = in_range << aIO[LG2REGS-1:0];
wire  [NUM_REGS-1:0]    write = {NUM_REGS{cIO==3'b110}} & decode;
wire  [NUM_REGS-1:0]     read = {NUM_REGS{cIO==3'b101}} & decode;
// Writes

//(* SiART  slow_clock *)
wire        ld_X  = write[XSALSA_XBUF],        // Push Xbuf data
            ld_XB = write[XSALSA_XBBS],        // Push Xbuf data byte swapped
            stop  = write[XSALSA_STOP],        // Stop the co-processor and reset
            start = write[XSALSA_STRT_RDY],    // Start processing, look for ready
            ld_CR = write[XSALSA_CTRL];        // Load Control Register
// Reads
//(* SiART  slow_clock *)
wire        rd_X  =  read[XSALSA_XBUF],        // Pop  Xbuf data
            rd_XB =  read[XSALSA_XBBS],        // Pop  Xbuf data byte-swapped
            fini  =  read[XSALSA_STRT_RDY];    // Read status:  if finished TRUE(0xffffffff) else FALSE(0x00000000)

//(* SiART  slow_clock *)
wire [2:0]  sIOo  = {(rd_X  &  idle),       // We may select None and return all zeros (FALSE)
                     (rd_XB &  idle),
                      ld_CR | (fini  && (done | idle))};

A2_mux #(3,32) iMXo (.z(IOo[31:0]), .s(sIOo), .d({OUT[31:0], OUT[7:0],OUT[15:8],OUT[23:16],OUT[31:24], 32'hffffffff}));

//(* SiART  slow_clock *)
wire        rXXB  =  rd_X | rd_XB;

always @(posedge slow_clock)  RXk <= `tCQ rXXB;

//(* SiART  slow_clock *)
wire        rdXk  =  CTRL[1] ?  RXk & rXXB : rXXB;
wire        IOk   =  ld_X | ld_XB | stop | start | ld_CR | rdXk | fini;

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

// Capture commands and move into fast fast_clock
// All these flip=flops operate in the same way.  An incoming command in the slow clock is first captured on a slow clock
// rising edge.  The output is then captured on the next rising edge of the fast clock.  This signal then, is used to reset the
// slow-clock flip-flop so on the next fast clock rising edge the signal is returned to low.  So a single fast-clock
// cycle version of incoming signal is produced.  This all works as the slow-clock rising edge is always coincident with a
// fast-clock rising edge.
//
always @(posedge slow_clock or posedge fast_load )
   if (fast_load)             slow_load   <= `tFCQ   1'b0;                    //                       _finish here
   else                       slow_load   <= `tFCQ   idle & (ld_X | ld_XB);   // Start here  _        /
always @(posedge fast_clock)  fast_load   <= `tFCQ   slow_load;               // Steps..   // \_then here_/

always @(posedge slow_clock or posedge fast_read )
   if (fast_read)             slow_read   <= `tFCQ   1'b0;
   else                       slow_read   <= `tFCQ   idle & (rd_X | rd_XB);
always @(posedge fast_clock)  fast_read   <= `tFCQ   slow_read;               // Steps..

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

//(* SiART -- fast_clock *)
reg          RUN,  PRIME, INTERPOLATE, YSAVED;
reg [10:0]   ITERATION;
reg  [5:0]   CYCLE;
reg  [2:0]   FSM;     localparam [2:0]  IDLE = 0, PREP = 1, LOW = 2, HIGH = 3, TAIL = 4,  DONE = 5;

always @(posedge fast_clock) begin
   if (freset || fast_stop) begin
      FSM         <= `tFCQ  IDLE;
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

         e_tail   = (CYCLE==30),
         tail     = (CYCLE==29),
         tail_L   =  low  & tail,
         tail_H   =  high & tail,

         e2head_P = (low | high) & (CYCLE[1:0]==1) & ~(CYCLE==33) & ~(CYCLE==41) & ~(CYCLE==37) & ~(CYCLE==45),
         e_head_P = (low | high) & (CYCLE[1:0]==2) & ~(CYCLE==34) & ~(CYCLE==42),  // pipeline controls
         head_P   = (low | high) & (CYCLE[1:0]==3) & ~(CYCLE==35) & ~(CYCLE==43),  // pipeline controls
         x_head_P = head_P ^ e_head_P,

         MX       =  ITERATION[10],                                          // Read Memory and Mix flag ELSE Write Memory flag

         head_eWM =  !MX                ? (head_L & ~ITERATION[0])
                  :         INTERPOLATE ? (high   &  (CYCLE==37))
                  :                       (high   &  (CYCLE==33)  & ~b1_ADR[0]),

         ld_wrADR =  !MX                &  high   &  (CYCLE==33)  | freset,
         ld_rdADR =   MX & !INTERPOLATE &  high   &  (CYCLE==33),

         save_X   =   MX &  INTERPOLATE & (CYCLE==38),
         e_load_X =   MX &  INTERPOLATE & (CYCLE==38),
         load_X   =   MX &  INTERPOLATE & (CYCLE==41);

//===============================================================================================================================
// Pipelined control bits that enable register loads
//===============================================================================================================================

// Registered Control Lines -----------------------------------------------------------------------------------------------------

`ifndef  REORG
//(* SiART  fast_clock *)
reg   RL32u36,    RL33u37,    RL34u38;
reg   RH32u36,    RH33u37,    RH34u38;
reg   MIX,        SMIXs;
reg   E_PRIME,    E_head_P,   E_tail;              // Registered Early Control Signals for the pipeline
reg   RLHx, RLHy, RLHz, RLH0, RLH1, RLH2, RLH3;
reg   RL0x, RL0y, RL0z, RL00, RL01, RL02, RL03;
reg   RH0x, RH0y, RH0z, RH00, RH01, RH02, RH03;
reg   RM0x, RM0y, RM0z, RM00, RM01, RM02, RM03;
reg   RL30, RL31, RL32, RL33, RL34, RL35, RL36, RL37, RL38, RL39, RL40;
reg   RH30, RH31, RH32, RH33, RH34, RH35, RH36, RH37, RH38, RH39, RH40;

`else
//(* SiART  fast_clock *)
reg         RL32u36,    RL33u37,    RL34u38;
reg         RH32u36,    RH33u37,    RH34u38;
reg         SMIXs;
reg         E_PRIME,    E_head_P,   E_tail;              // Registered Early Control Signals for the pipeline
reg         RLHx, RLHy, RLHz, RLH0, RLH1, RLH2, RLH3;

/*(* SiART  multicycle *)*/ reg MIX;

reg         RL0x, RL0y;
reg [01:00] RL0z;
reg [39:00] RL03, RL02, RL01, RL00;

reg         RH0x, RH0y;
reg [01:00] RH0z;
reg [19:00] RH03, RH02, RH01, RH00;

reg         RM0x,RM0y;
reg [01:00] RM0z;
reg [39:00] RM03, RM02, RM01, RM00;

reg         RL30, RL31, RL32, RL33;
reg [04:00] RL34, RL40;
reg [09:00] RL35, RL39;
reg [14:00] RL36, RL38;
reg [19:00] RL37;

reg         RH30, RH31, RH32, RH33;
reg [04:00] RH34, RH40;
reg [09:00] RH35, RH39;
reg [14:00] RH36, RH38;
reg [19:00] RH37;

reg [32:00] FS0;

`endif
// ------------------------------------------------------------------------------------------------------------------------------

`ifndef  REORG
//always @(posedge fast_clock) if (ld_rdADR && b1_ADR[0]                         )  INTERPOLATE   <= `tFCQ  1'b1;
//                        else if (freset || INTERPOLATE && (high && (CYCLE==33)))  INTERPOLATE   <= `tFCQ  1'b0;
//
//always @(posedge fast_clock) if (save_X                                        )  YSAVED  <= `tFCQ 1'b1;
//                        else if (freset || YSAVED  && (RL03 & ~INTERPOLATE)    )  YSAVED  <= `tFCQ 1'b0;
//
//always @(posedge fast_clock) if (freset || RUN && ( RL32u36 || RL31   || RL35 ))  RL32u36 <= `tFCQ ~(freset | RL32u36);
//always @(posedge fast_clock) if (freset || RUN && ( RL33u37 || RL32u36        ))  RL33u37 <= `tFCQ ~(freset | RL33u37);
//always @(posedge fast_clock) if (freset || RUN && ( RL34u38 || RL33u37        ))  RL34u38 <= `tFCQ ~(freset | RL34u38);
//always @(posedge fast_clock) if (freset || RUN && ( RH32u36 || RH31   || RH35 ))  RH32u36 <= `tFCQ ~(freset | RH32u36);
//always @(posedge fast_clock) if (freset || RUN && ( RH33u37 || RH32u36        ))  RH33u37 <= `tFCQ ~(freset | RH33u37);
//always @(posedge fast_clock) if (freset || RUN && ( RH34u38 || RH33u37        ))  RH34u38 <= `tFCQ ~(freset | RH34u38);
//
//always @(posedge fast_clock) if (freset || RUN && ( e_head_M  || MIX  && RL03   ))  MIX     <= `tFCQ ~(freset | MIX & RL03);
//
//always @(posedge fast_clock) if (freset || RUN && ( RLHx || e_head_L || e_head_H)) RLHx   <= `tFCQ ~(freset | RLHx);
//always @(posedge fast_clock) if (freset || RUN && ( RLHy || RLHx              ))  RLHy    <= `tFCQ ~(freset | RLHy);
//always @(posedge fast_clock) if (freset || RUN && ( RLHz || RLHy              ))  RLHz    <= `tFCQ ~(freset | RLHz);
//always @(posedge fast_clock) if (freset || RUN && ( RLH0 || RLHz              ))  RLH0    <= `tFCQ ~(freset | RLH0);
//always @(posedge fast_clock) if (freset || RUN && ( RLH1 || RLH0              ))  RLH1    <= `tFCQ ~(freset | RLH1);
//always @(posedge fast_clock) if (freset || RUN && ( RLH2 || RLH1              ))  RLH2    <= `tFCQ ~(freset | RLH2);
//always @(posedge fast_clock) if (freset || RUN && ( RLH3 || RLH2              ))  RLH3    <= `tFCQ ~(freset | RLH3);
//
//wire       en_SMIXs = e_head_M & YSAVED & ~INTERPOLATE | SMIXs & (CYCLE==5);
//
//always @(posedge fast_clock) if (freset || RUN &&  en_SMIXs                    )  SMIXs   <= `tFCQ ~(freset | SMIXs );
//always @(posedge fast_clock) if (freset || RL0y     || RH0y                    )  E_PRIME <= `tFCQ ~freset;
//                        else if (E_PRIME &&    (low || high) && (CYCLE==2)     )  E_PRIME <= `tFCQ  1'b0;
//always @(posedge fast_clock) if (freset || e_head_P || E_head_P                ) E_head_P <= `tFCQ  ~freset & e_head_P;
//always @(posedge fast_clock) if (freset || e_tail   || E_tail                  )  E_tail  <= `tFCQ  ~freset & e_tail;
//
//always @(posedge fast_clock) if (freset || RUN && ( RL0x || e_head_L ))  RL0x    <= `tFCQ ~(freset | RL0x);
//always @(posedge fast_clock) if (freset || RUN && ( RL0y || RL0x     ))  RL0y    <= `tFCQ ~(freset | RL0y);
//always @(posedge fast_clock) if (freset || RUN && ( RL0z || RL0y     ))  RL0z    <= `tFCQ ~(freset | RL0z);
//always @(posedge fast_clock) if (freset || RUN && ( RL00 || RL0z     ))  RL00    <= `tFCQ ~(freset | RL00);
//always @(posedge fast_clock) if (freset || RUN && ( RL01 || RL00     ))  RL01    <= `tFCQ ~(freset | RL01);
//always @(posedge fast_clock) if (freset || RUN && ( RL02 || RL01     ))  RL02    <= `tFCQ ~(freset | RL02);
//always @(posedge fast_clock) if (freset || RUN && ( RL03 || RL02     ))  RL03    <= `tFCQ ~(freset | RL03);
//
//always @(posedge fast_clock) if (freset || RUN && ( RH0x || e_head_H ))  RH0x    <= `tFCQ ~(freset | RH0x );
//always @(posedge fast_clock) if (freset || RUN && ( RH0y || RH0x     ))  RH0y    <= `tFCQ ~(freset | RH0y );
//always @(posedge fast_clock) if (freset || RUN && ( RH0z || RH0y     ))  RH0z    <= `tFCQ ~(freset | RH0z );
//always @(posedge fast_clock) if (freset || RUN && ( RH00 || RH0z     ))  RH00    <= `tFCQ ~(freset | RH00 );
//always @(posedge fast_clock) if (freset || RUN && ( RH01 || RH00     ))  RH01    <= `tFCQ ~(freset | RH01 );
//always @(posedge fast_clock) if (freset || RUN && ( RH02 || RH01     ))  RH02    <= `tFCQ ~(freset | RH02 );
//always @(posedge fast_clock) if (freset || RUN && ( RH03 || RH02     ))  RH03    <= `tFCQ ~(freset | RH03 );
//
//always @(posedge fast_clock) if (freset || RUN && ( RM0x || e_load_X ))  RM0x    <= `tFCQ ~(freset | RM0x);
//always @(posedge fast_clock) if (freset || RUN && ( RM0y || RM0x     ))  RM0y    <= `tFCQ ~(freset | RM0y);
//always @(posedge fast_clock) if (freset || RUN && ( RM0z || RM0y     ))  RM0z    <= `tFCQ ~(freset | RM0z);
//always @(posedge fast_clock) if (freset || RUN && ( RM00 || RM0z     ))  RM00    <= `tFCQ ~(freset | RM00);
//always @(posedge fast_clock) if (freset || RUN && ( RM01 || RM00     ))  RM01    <= `tFCQ ~(freset | RM01);
//always @(posedge fast_clock) if (freset || RUN && ( RM02 || RM01     ))  RM02    <= `tFCQ ~(freset | RM02);
//always @(posedge fast_clock) if (freset || RUN && ( RM03 || RM02     ))  RM03    <= `tFCQ ~(freset | RM03);
//
//always @(posedge fast_clock) if (freset || RUN && ( RL30 || tail_L   ))  RL30    <= `tFCQ ~(freset | RL30);
//always @(posedge fast_clock) if (freset || RUN && ( RL31 || RL30     ))  RL31    <= `tFCQ ~(freset | RL31);
//always @(posedge fast_clock) if (freset || RUN && ( RL32 || RL31     ))  RL32    <= `tFCQ ~(freset | RL32);
//always @(posedge fast_clock) if (freset || RUN && ( RL33 || RL32     ))  RL33    <= `tFCQ ~(freset | RL33);
//always @(posedge fast_clock) if (freset || RUN && ( RL34 || RL33     ))  RL34    <= `tFCQ ~(freset | RL34);
//always @(posedge fast_clock) if (freset || RUN && ( RL35 || RL34     ))  RL35    <= `tFCQ ~(freset | RL35);
//always @(posedge fast_clock) if (freset || RUN && ( RL36 || RL35     ))  RL36    <= `tFCQ ~(freset | RL36);
//always @(posedge fast_clock) if (freset || RUN && ( RL37 || RL36     ))  RL37    <= `tFCQ ~(freset | RL37);
//always @(posedge fast_clock) if (freset || RUN && ( RL38 || RL37     ))  RL38    <= `tFCQ ~(freset | RL38);
//always @(posedge fast_clock) if (freset || RUN && ( RL39 || RL38     ))  RL39    <= `tFCQ ~(freset | RL39);
//always @(posedge fast_clock) if (freset || RUN && ( RL40 || RL39     ))  RL40    <= `tFCQ ~(freset | RL40);
//
//always @(posedge fast_clock) if (freset || RUN && ( RH30 || tail_H   ))  RH30    <= `tFCQ ~(freset | RH30);
//always @(posedge fast_clock) if (freset || RUN && ( RH31 || RH30     ))  RH31    <= `tFCQ ~(freset | RH31);
//always @(posedge fast_clock) if (freset || RUN && ( RH32 || RH31     ))  RH32    <= `tFCQ ~(freset | RH32);
//always @(posedge fast_clock) if (freset || RUN && ( RH33 || RH32     ))  RH33    <= `tFCQ ~(freset | RH33);
//always @(posedge fast_clock) if (freset || RUN && ( RH34 || RH33     ))  RH34    <= `tFCQ ~(freset | RH34);
//always @(posedge fast_clock) if (freset || RUN && ( RH35 || RH34     ))  RH35    <= `tFCQ ~(freset | RH35);
//always @(posedge fast_clock) if (freset || RUN && ( RH36 || RH35     ))  RH36    <= `tFCQ ~(freset | RH36);
//always @(posedge fast_clock) if (freset || RUN && ( RH37 || RH36     ))  RH37    <= `tFCQ ~(freset | RH37);
//always @(posedge fast_clock) if (freset || RUN && ( RH38 || RH37     ))  RH38    <= `tFCQ ~(freset | RH38);
//always @(posedge fast_clock) if (freset || RUN && ( RH39 || RH38     ))  RH39    <= `tFCQ ~(freset | RH39);
//always @(posedge fast_clock) if (freset || RUN && ( RH40 || RH39     ))  RH40    <= `tFCQ ~(freset | RH40);
//
////===============================================================================================================================
//// Register Files
////===============================================================================================================================
//
//`define  SubV8(x,a,b,c,d,e,f,g,h)  {x[a],x[b],x[c],x[d],x[e],x[f],x[g],x[h]}  // 8 element sub-vector
//
//always @(posedge fast_clock) begin
//   // Move r0_X to r0_Y
//   if (RM00)   `SubV8(r0_Y,16,20,24,28,00,04,08,12) <= `tFCQ `SubV8(r0_X,16,20,24,28,00,04,08,12);
//   if (RM01)   `SubV8(r0_Y,21,25,29,17,05,09,13,01) <= `tFCQ `SubV8(r0_X,21,25,29,17,05,09,13,01);
//   if (RM02)   `SubV8(r0_Y,26,30,18,22,10,14,02,06) <= `tFCQ `SubV8(r0_X,26,30,18,22,10,14,02,06);
//   if (RM03)   `SubV8(r0_Y,31,19,23,27,15,03,07,11) <= `tFCQ `SubV8(r0_X,31,19,23,27,15,03,07,11);
//   end
//
//always @(posedge fast_clock) begin
//   // Load Inputs \ Read Regs
//   if (fast_shift)
//      for (i=0; i<32; i=i+1)
//         if (i==31)  r0_X[i] <= `tFCQ  fast_load ? IN[31:0] : r0_X[0];
//         else        r0_X[i] <= `tFCQ  r0_X[i+1];
//   // Load from Memory so we can interpolate
//   if (RM00)            `SubV8(r0_X,16,20,24,28,00,04,08,12)     <= `tFCQ  WMo[255:000]; //fo=256
//   if (RM01)            `SubV8(r0_X,21,25,29,17,05,09,13,01)     <= `tFCQ  WMo[255:000]; //fo=256
//   if (RM02)            `SubV8(r0_X,26,30,18,22,10,14,02,06)     <= `tFCQ  WMo[255:000]; //fo=256
//   if (RM03)            `SubV8(r0_X,31,19,23,27,15,03,07,11)     <= `tFCQ  WMo[255:000]; //fo=256
//   // Load XOR results
//   if (RL00)            {r0_X[00], r0_X[04], r0_X[08], r0_X[12]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
//   if (RL01)            {r0_X[05], r0_X[09], r0_X[13], r0_X[01]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
//   if (RL02)            {r0_X[10], r0_X[14], r0_X[02], r0_X[06]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
//   if (RL03)            {r0_X[15], r0_X[03], r0_X[07], r0_X[11]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
//   if (RH00)            {r0_X[16], r0_X[20], r0_X[24], r0_X[28]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
//   if (RH01)            {r0_X[21], r0_X[25], r0_X[29], r0_X[17]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
//   if (RH02)            {r0_X[26], r0_X[30], r0_X[18], r0_X[22]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
//   if (RH03)            {r0_X[31], r0_X[19], r0_X[23], r0_X[27]} <= `tFCQ  { b0_ZA,  b0_ZB,  b0_ZC,  b0_ZD };           //fo=128
//   // Mixing
//   if (RL00 && MIX)     {r0_X[16], r0_X[20], r0_X[24], r0_X[28]} <= `tFCQ  { b0_XA,  b0_XB,  b0_XC,  b0_XD };           //fo=128
//   if (RL01 && MIX)     {r0_X[21], r0_X[25], r0_X[29], r0_X[17]} <= `tFCQ  { b0_XA,  b0_XB,  b0_XC,  b0_XD };           //fo=128
//   if (RL02 && MIX)     {r0_X[26], r0_X[30], r0_X[18], r0_X[22]} <= `tFCQ  { b0_XA,  b0_XB,  b0_XC,  b0_XD };           //fo=128
//   if (RL03 && MIX)     {r0_X[31], r0_X[19], r0_X[23], r0_X[27]} <= `tFCQ  { b0_XA,  b0_XB,  b0_XC,  b0_XD };           //fo=128
//   // Load Low Salsa Results
//   if (RL34)             r0_X[00]                                <= `tFCQ   r2_ADD;                                     //fo= 32
//   if (RL35)            {r0_X[05], r0_X[04]                    } <= `tFCQ  {r2_ADD, r3_ADD                };            //fo= 64
//   if (RL36)            {r0_X[10], r0_X[09], r0_X[08]          } <= `tFCQ  {r2_ADD, r3_ADD, r4_ADD        };            //fo= 96
//   if (RL37)            {r0_X[15], r0_X[14], r0_X[13], r0_X[12]} <= `tFCQ  {r2_ADD, r3_ADD, r4_ADD, r5_ADD};            //fo=128
//   if (RL38)            {          r0_X[03], r0_X[02], r0_X[01]} <= `tFCQ  {        r3_ADD, r4_ADD, r5_ADD};            //fo= 96
//   if (RL39)            {                    r0_X[07], r0_X[06]} <= `tFCQ  {                r4_ADD, r5_ADD};            //fo= 64
//   if (RL40)                                           r0_X[11]  <= `tFCQ                           r5_ADD;             //fo= 32
//   // Load High Salsa Results
//   if (RH34)             r0_X[16]                                <= `tFCQ   r2_ADD;                                     //fo= 32
//   if (RH35)            {r0_X[21], r0_X[20]                    } <= `tFCQ  {r2_ADD, r3_ADD                };            //fo= 64
//   if (RH36)            {r0_X[26], r0_X[25], r0_X[24]          } <= `tFCQ  {r2_ADD, r3_ADD, r4_ADD        };            //fo= 96
//   if (RH37)            {r0_X[31], r0_X[30], r0_X[29], r0_X[28]} <= `tFCQ  {r2_ADD, r3_ADD, r4_ADD, r5_ADD};            //fo=128
//   if (RH38)            {          r0_X[19], r0_X[18], r0_X[17]} <= `tFCQ  {        r3_ADD, r4_ADD, r5_ADD};            //fo= 96
//   if (RH39)            {                    r0_X[23], r0_X[22]} <= `tFCQ  {                r4_ADD, r5_ADD};            //fo= 64
//   if (RH40)                                           r0_X[27]  <= `tFCQ                           r5_ADD;             //fo= 32
//   end

`else    // REORG is defined -v-v-v-v-v-v-v

//-------------------------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (ld_rdADR && b1_ADR[0]                           )  INTERPOLATE   <= `tFCQ  1'b1;
                        else if (freset || INTERPOLATE && (high && (CYCLE==33))  )  INTERPOLATE   <= `tFCQ  1'b0;

always @(posedge fast_clock) if (save_X                                          )  YSAVED  <= `tFCQ 1'b1;
                        else if (freset || YSAVED  && (RL03[4] && !INTERPOLATE)  )  YSAVED  <= `tFCQ 1'b0;

always @(posedge fast_clock) if (freset || RUN && ( RL32u36 || RL31 || RL35[9]   )) RL32u36 <= `tFCQ ~(freset | RL32u36);
always @(posedge fast_clock) if (freset || RUN && ( RL33u37 || RL32u36           )) RL33u37 <= `tFCQ ~(freset | RL33u37);
always @(posedge fast_clock) if (freset || RUN && ( RL34u38 || RL33u37           )) RL34u38 <= `tFCQ ~(freset | RL34u38);
always @(posedge fast_clock) if (freset || RUN && ( RH32u36 || RH31 || RH35[9]   )) RH32u36 <= `tFCQ ~(freset | RH32u36);
always @(posedge fast_clock) if (freset || RUN && ( RH33u37 || RH32u36           )) RH33u37 <= `tFCQ ~(freset | RH33u37);
always @(posedge fast_clock) if (freset || RUN && ( RH34u38 || RH33u37           )) RH34u38 <= `tFCQ ~(freset | RH34u38);

always @(posedge fast_clock) if (freset || RUN && (e_head_M || MIX  && RL03[9]   )) MIX     <= `tFCQ ~(freset | MIX & RL03[14]);

always @(posedge fast_clock) if (freset || RUN && ( RLHx || e_head_L || e_head_H )) RLHx    <= `tFCQ ~(freset | RLHx);
always @(posedge fast_clock) if (freset || RUN && ( RLHy || RLHx                 )) RLHy    <= `tFCQ ~(freset | RLHy);
always @(posedge fast_clock) if (freset || RUN && ( RLHz || RLHy                 )) RLHz    <= `tFCQ ~(freset | RLHz);
always @(posedge fast_clock) if (freset || RUN && ( RLH0 || RLHz                 )) RLH0    <= `tFCQ ~(freset | RLH0);
always @(posedge fast_clock) if (freset || RUN && ( RLH1 || RLH0                 )) RLH1    <= `tFCQ ~(freset | RLH1);
always @(posedge fast_clock) if (freset || RUN && ( RLH2 || RLH1                 )) RLH2    <= `tFCQ ~(freset | RLH2);
always @(posedge fast_clock) if (freset || RUN && ( RLH3 || RLH2                 )) RLH3    <= `tFCQ ~(freset | RLH3);

wire   en_SMIXs = e_head_M & YSAVED & ~INTERPOLATE  | SMIXs  &  (CYCLE==5);

always @(posedge fast_clock) if (freset || RUN &&  en_SMIXs                      )  SMIXs   <= `tFCQ ~(freset | SMIXs );
always @(posedge fast_clock) if (freset || RL0y     || RH0y                      )  E_PRIME <= `tFCQ ~freset;
                        else if (E_PRIME &&    (low || high) && (CYCLE==2)       )  E_PRIME <= `tFCQ  1'b0;
always @(posedge fast_clock) if (freset || e_head_P || E_head_P                  ) E_head_P <= `tFCQ  ~freset & e_head_P;
always @(posedge fast_clock) if (freset || e_tail   || E_tail                    )  E_tail  <= `tFCQ  ~freset & e_tail;


always @(posedge fast_clock) if (freset || RUN && (RH0x     || e_load_X))  RH0x        <= `tFCQ ~(freset | RH0x);
always @(posedge fast_clock) if (freset || RUN && (RH0y     || RH0x    ))  RH0y        <= `tFCQ ~(freset | RH0y);
always @(posedge fast_clock) if (freset || RUN && (RH0z[1]  || RH0y    ))  RH0z[01:00] <= `tFCQ   freset ? 5'b0 : {5{~RH0z[1]}};
always @(posedge fast_clock) if (freset || RUN && (RH00[19] || RH0z[00]))  RH00[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RH00[19]}};
always @(posedge fast_clock) if (freset || RUN && (RH00[14] || RH0z[00]))  RH00[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RH00[14]}};
always @(posedge fast_clock) if (freset || RUN && (RH00[09] || RH0z[00]))  RH00[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RH00[09]}};
always @(posedge fast_clock) if (freset || RUN && (RH00[04] || RH0z[00]))  RH00[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RH00[04]}};
always @(posedge fast_clock) if (freset || RUN && (RH01[19] || RH00[19]))  RH01[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RH01[19]}};
always @(posedge fast_clock) if (freset || RUN && (RH01[14] || RH00[14]))  RH01[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RH01[14]}};
always @(posedge fast_clock) if (freset || RUN && (RH01[09] || RH00[09]))  RH01[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RH01[09]}};
always @(posedge fast_clock) if (freset || RUN && (RH01[04] || RH00[04]))  RH01[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RH01[04]}};
always @(posedge fast_clock) if (freset || RUN && (RH02[19] || RH01[19]))  RH02[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RH02[19]}};
always @(posedge fast_clock) if (freset || RUN && (RH02[14] || RH01[14]))  RH02[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RH02[14]}};
always @(posedge fast_clock) if (freset || RUN && (RH02[09] || RH01[09]))  RH02[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RH02[09]}};
always @(posedge fast_clock) if (freset || RUN && (RH02[04] || RH01[04]))  RH02[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RH02[04]}};
always @(posedge fast_clock) if (freset || RUN && (RH03[19] || RH02[19]))  RH03[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RH03[19]}};
always @(posedge fast_clock) if (freset || RUN && (RH03[14] || RH02[14]))  RH03[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RH03[14]}};
always @(posedge fast_clock) if (freset || RUN && (RH03[09] || RH02[09]))  RH03[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RH03[09]}};
always @(posedge fast_clock) if (freset || RUN && (RH03[04] || RH02[04]))  RH03[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RH03[04]}};
//-------------------------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (freset || RUN && (RL0x     || e_load_X))  RL0x        <= `tFCQ ~(freset | RL0x);
always @(posedge fast_clock) if (freset || RUN && (RL0y     || RL0x    ))  RL0y        <= `tFCQ ~(freset | RL0y);
always @(posedge fast_clock) if (freset || RUN && (RL0z[1]  || RL0y    ))  RL0z[01:00] <= `tFCQ   freset ? 5'b0 : {5{~RL0z[1]}};
always @(posedge fast_clock) if (freset || RUN && (RL00[39] || RL0z[00]))  RL00[39:35] <= `tFCQ   freset ? 5'b0 : {5{~RL00[39]}};
always @(posedge fast_clock) if (freset || RUN && (RL00[34] || RL0z[00]))  RL00[34:30] <= `tFCQ   freset ? 5'b0 : {5{~RL00[34]}};
always @(posedge fast_clock) if (freset || RUN && (RL00[29] || RL0z[00]))  RL00[29:25] <= `tFCQ   freset ? 5'b0 : {5{~RL00[29]}};
always @(posedge fast_clock) if (freset || RUN && (RL00[24] || RL0z[00]))  RL00[24:20] <= `tFCQ   freset ? 5'b0 : {5{~RL00[24]}};
always @(posedge fast_clock) if (freset || RUN && (RL00[19] || RL0z[00]))  RL00[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RL00[19]}};
always @(posedge fast_clock) if (freset || RUN && (RL00[14] || RL0z[00]))  RL00[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RL00[14]}};
always @(posedge fast_clock) if (freset || RUN && (RL00[09] || RL0z[00]))  RL00[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RL00[09]}};
always @(posedge fast_clock) if (freset || RUN && (RL00[04] || RL0z[00]))  RL00[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RL00[04]}};
always @(posedge fast_clock) if (freset || RUN && (RL01[39] || RL00[39]))  RL01[39:35] <= `tFCQ   freset ? 5'b0 : {5{~RL01[39]}};
always @(posedge fast_clock) if (freset || RUN && (RL01[34] || RL00[34]))  RL01[34:30] <= `tFCQ   freset ? 5'b0 : {5{~RL01[34]}};
always @(posedge fast_clock) if (freset || RUN && (RL01[29] || RL00[29]))  RL01[29:25] <= `tFCQ   freset ? 5'b0 : {5{~RL01[29]}};
always @(posedge fast_clock) if (freset || RUN && (RL01[24] || RL00[24]))  RL01[24:20] <= `tFCQ   freset ? 5'b0 : {5{~RL01[24]}};
always @(posedge fast_clock) if (freset || RUN && (RL01[19] || RL00[19]))  RL01[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RL01[19]}};
always @(posedge fast_clock) if (freset || RUN && (RL01[14] || RL00[14]))  RL01[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RL01[14]}};
always @(posedge fast_clock) if (freset || RUN && (RL01[09] || RL00[09]))  RL01[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RL01[09]}};
always @(posedge fast_clock) if (freset || RUN && (RL01[04] || RL00[04]))  RL01[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RL01[04]}};
always @(posedge fast_clock) if (freset || RUN && (RL02[39] || RL01[39]))  RL02[39:35] <= `tFCQ   freset ? 5'b0 : {5{~RL02[39]}};
always @(posedge fast_clock) if (freset || RUN && (RL02[34] || RL01[34]))  RL02[34:30] <= `tFCQ   freset ? 5'b0 : {5{~RL02[34]}};
always @(posedge fast_clock) if (freset || RUN && (RL02[29] || RL01[29]))  RL02[29:25] <= `tFCQ   freset ? 5'b0 : {5{~RL02[29]}};
always @(posedge fast_clock) if (freset || RUN && (RL02[24] || RL01[24]))  RL02[24:20] <= `tFCQ   freset ? 5'b0 : {5{~RL02[24]}};
always @(posedge fast_clock) if (freset || RUN && (RL02[19] || RL01[19]))  RL02[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RL02[19]}};
always @(posedge fast_clock) if (freset || RUN && (RL02[14] || RL01[14]))  RL02[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RL02[14]}};
always @(posedge fast_clock) if (freset || RUN && (RL02[09] || RL01[09]))  RL02[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RL02[09]}};
always @(posedge fast_clock) if (freset || RUN && (RL02[04] || RL01[04]))  RL02[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RL02[04]}};
always @(posedge fast_clock) if (freset || RUN && (RL03[39] || RL02[39]))  RL03[39:35] <= `tFCQ   freset ? 5'b0 : {5{~RL03[39]}};
always @(posedge fast_clock) if (freset || RUN && (RL03[34] || RL02[34]))  RL03[34:30] <= `tFCQ   freset ? 5'b0 : {5{~RL03[34]}};
always @(posedge fast_clock) if (freset || RUN && (RL03[29] || RL02[29]))  RL03[29:25] <= `tFCQ   freset ? 5'b0 : {5{~RL03[29]}};
always @(posedge fast_clock) if (freset || RUN && (RL03[24] || RL02[24]))  RL03[24:20] <= `tFCQ   freset ? 5'b0 : {5{~RL03[24]}};
always @(posedge fast_clock) if (freset || RUN && (RL03[19] || RL02[19]))  RL03[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RL03[19]}};
always @(posedge fast_clock) if (freset || RUN && (RL03[14] || RL02[14]))  RL03[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RL03[14]}};
always @(posedge fast_clock) if (freset || RUN && (RL03[09] || RL02[09]))  RL03[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RL03[09]}};
always @(posedge fast_clock) if (freset || RUN && (RL03[04] || RL02[04]))  RL03[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RL03[04]}};
//-----------------------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (freset || RUN && (RM0x     || e_load_X))  RM0x        <= `tFCQ ~(freset | RM0x);
always @(posedge fast_clock) if (freset || RUN && (RM0y     || RM0x    ))  RM0y        <= `tFCQ ~(freset | RM0y);
always @(posedge fast_clock) if (freset || RUN && (RM0z[1]  || RM0y    ))  RM0z[1:0]   <= `tFCQ   freset ? 5'b0 : {5{~RM0z[1]}};
always @(posedge fast_clock) if (freset || RUN && (RM00[39] || RM0z[00]))  RM00[39:35] <= `tFCQ   freset ? 5'b0 : {5{~RM03[39]}};
always @(posedge fast_clock) if (freset || RUN && (RM00[34] || RM0z[00]))  RM00[34:30] <= `tFCQ   freset ? 5'b0 : {5{~RM03[34]}};
always @(posedge fast_clock) if (freset || RUN && (RM00[29] || RM0z[00]))  RM00[29:25] <= `tFCQ   freset ? 5'b0 : {5{~RM03[29]}};
always @(posedge fast_clock) if (freset || RUN && (RM00[24] || RM0z[00]))  RM00[24:20] <= `tFCQ   freset ? 5'b0 : {5{~RM03[24]}};
always @(posedge fast_clock) if (freset || RUN && (RM00[19] || RM0z[00]))  RM00[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RM03[19]}};
always @(posedge fast_clock) if (freset || RUN && (RM00[14] || RM0z[00]))  RM00[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RM03[14]}};
always @(posedge fast_clock) if (freset || RUN && (RM00[09] || RM0z[00]))  RM00[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RM03[09]}};
always @(posedge fast_clock) if (freset || RUN && (RM00[04] || RM0z[00]))  RM00[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RM03[04]}};
always @(posedge fast_clock) if (freset || RUN && (RM01[39] || RM00[39]))  RM01[39:35] <= `tFCQ   freset ? 5'b0 : {5{~RM03[39]}};
always @(posedge fast_clock) if (freset || RUN && (RM01[34] || RM00[34]))  RM01[34:30] <= `tFCQ   freset ? 5'b0 : {5{~RM03[34]}};
always @(posedge fast_clock) if (freset || RUN && (RM01[29] || RM00[29]))  RM01[29:25] <= `tFCQ   freset ? 5'b0 : {5{~RM03[29]}};
always @(posedge fast_clock) if (freset || RUN && (RM01[24] || RM00[24]))  RM01[24:20] <= `tFCQ   freset ? 5'b0 : {5{~RM03[24]}};
always @(posedge fast_clock) if (freset || RUN && (RM01[19] || RM00[19]))  RM01[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RM03[19]}};
always @(posedge fast_clock) if (freset || RUN && (RM01[14] || RM00[14]))  RM01[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RM03[14]}};
always @(posedge fast_clock) if (freset || RUN && (RM01[09] || RM00[09]))  RM01[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RM03[09]}};
always @(posedge fast_clock) if (freset || RUN && (RM01[04] || RM00[04]))  RM01[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RM03[04]}};
always @(posedge fast_clock) if (freset || RUN && (RM02[39] || RM01[39]))  RM02[39:35] <= `tFCQ   freset ? 5'b0 : {5{~RM03[39]}};
always @(posedge fast_clock) if (freset || RUN && (RM02[34] || RM01[34]))  RM02[34:30] <= `tFCQ   freset ? 5'b0 : {5{~RM03[34]}};
always @(posedge fast_clock) if (freset || RUN && (RM02[29] || RM01[29]))  RM02[29:25] <= `tFCQ   freset ? 5'b0 : {5{~RM03[29]}};
always @(posedge fast_clock) if (freset || RUN && (RM02[24] || RM01[24]))  RM02[24:20] <= `tFCQ   freset ? 5'b0 : {5{~RM03[24]}};
always @(posedge fast_clock) if (freset || RUN && (RM02[19] || RM01[19]))  RM02[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RM03[19]}};
always @(posedge fast_clock) if (freset || RUN && (RM02[14] || RM01[14]))  RM02[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RM03[14]}};
always @(posedge fast_clock) if (freset || RUN && (RM02[09] || RM01[09]))  RM02[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RM03[09]}};
always @(posedge fast_clock) if (freset || RUN && (RM02[04] || RM01[04]))  RM02[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RM03[04]}};
always @(posedge fast_clock) if (freset || RUN && (RM03[39] || RM02[39]))  RM03[39:35] <= `tFCQ   freset ? 5'b0 : {5{~RM03[39]}};
always @(posedge fast_clock) if (freset || RUN && (RM03[34] || RM02[34]))  RM03[34:30] <= `tFCQ   freset ? 5'b0 : {5{~RM03[34]}};
always @(posedge fast_clock) if (freset || RUN && (RM03[29] || RM02[29]))  RM03[29:25] <= `tFCQ   freset ? 5'b0 : {5{~RM03[29]}};
always @(posedge fast_clock) if (freset || RUN && (RM03[24] || RM02[24]))  RM03[24:20] <= `tFCQ   freset ? 5'b0 : {5{~RM03[24]}};
always @(posedge fast_clock) if (freset || RUN && (RM03[19] || RM02[19]))  RM03[19:15] <= `tFCQ   freset ? 5'b0 : {5{~RM03[19]}};
always @(posedge fast_clock) if (freset || RUN && (RM03[14] || RM02[14]))  RM03[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RM03[14]}};
always @(posedge fast_clock) if (freset || RUN && (RM03[09] || RM02[09]))  RM03[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RM03[09]}};
always @(posedge fast_clock) if (freset || RUN && (RM03[04] || RM02[04]))  RM03[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RM03[04]}};
//-----------------------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (freset || RUN && (RL30     || tail_L  ))  RL30        <= `tFCQ   freset ? 1'b0 :    ~RL30;
always @(posedge fast_clock) if (freset || RUN && (RL31     || RL30    ))  RL31        <= `tFCQ   freset ? 1'b0 :    ~RL31;
always @(posedge fast_clock) if (freset || RUN && (RL32     || RL31    ))  RL32        <= `tFCQ   freset ? 1'b0 :    ~RL32;
always @(posedge fast_clock) if (freset || RUN && (RL33     || RL32    ))  RL33        <= `tFCQ   freset ? 1'b0 :    ~RL33;
always @(posedge fast_clock) if (freset || RUN && (RL34[04] || RL33    ))  RL34[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RL34[04]}};     //fo= 32   4  +1
always @(posedge fast_clock) if (freset || RUN && (RL35[09] || RL34[00]))  RL35[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RL35[09]}};     //fo= 64   8  +1
always @(posedge fast_clock) if (freset || RUN && (RL35[04] || RL34[00]))  RL35[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RL35[04]}};     //fo= 64   8  +1
always @(posedge fast_clock) if (freset || RUN && (RL36[14] || RL35[09]))  RL36[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RL36[14]}};     //fo= 96  12  14 13:10
always @(posedge fast_clock) if (freset || RUN && (RL36[09] || RL35[09]))  RL36[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RL36[09]}};     //fo= 96  12  09 08:05
always @(posedge fast_clock) if (freset || RUN && (RL36[04] || RL35[04]))  RL36[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RL36[04]}};     //fo= 96  12  04 03:00
always @(posedge fast_clock) if (freset || RUN && (RL37[19] || RL36[14]))  RL37[19:15] <= `tFCQ   freset ? 1'b0 : {5{~RL37[19]}};     //fo=128  16  19 18:15
always @(posedge fast_clock) if (freset || RUN && (RL37[14] || RL36[14]))  RL37[14:10] <= `tFCQ   freset ? 1'b0 : {5{~RL37[14]}};     //fo=128  16  14 13:10
always @(posedge fast_clock) if (freset || RUN && (RL37[09] || RL36[09]))  RL37[09:05] <= `tFCQ   freset ? 1'b0 : {5{~RL37[09]}};     //fo=128  16  09 08:05
always @(posedge fast_clock) if (freset || RUN && (RL37[04] || RL36[04]))  RL37[04:00] <= `tFCQ   freset ? 1'b0 : {5{~RL37[04]}};     //fo=128  16  04 03:00
always @(posedge fast_clock) if (freset || RUN && (RL38[14] || RL37[19]))  RL38[14:10] <= `tFCQ   freset ? 1'b0 : {5{~RL38[14]}};     //fo= 96  12  14 13:10
always @(posedge fast_clock) if (freset || RUN && (RL38[09] || RL37[14]))  RL38[09:05] <= `tFCQ   freset ? 1'b0 : {5{~RL38[09]}};     //fo= 96  12  09 08:05
always @(posedge fast_clock) if (freset || RUN && (RL38[04] || RL37[09]))  RL38[04:00] <= `tFCQ   freset ? 1'b0 : {5{~RL38[04]}};     //fo= 96  12  04 03:00
always @(posedge fast_clock) if (freset || RUN && (RL39[09] || RL38[14]))  RL39[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RL39[09]}};     //fo= 64   8  09 08:05
always @(posedge fast_clock) if (freset || RUN && (RL39[04] || RL38[09]))  RL39[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RL39[04]}};     //fo= 64   8  04 03:00
always @(posedge fast_clock) if (freset || RUN && (RL40[04] || RL39[08]))  RL40[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RL40[04]}};     //fo= 32   4  04 03:00
//-----------------------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (freset || RUN && (RH30     || tail_L  ))  RH30        <= `tFCQ   freset ? 1'b0 :    ~RH30;
always @(posedge fast_clock) if (freset || RUN && (RH31     || RH30    ))  RH31        <= `tFCQ   freset ? 1'b0 :    ~RH31;
always @(posedge fast_clock) if (freset || RUN && (RH32     || RH31    ))  RH32        <= `tFCQ   freset ? 1'b0 :    ~RH32;
always @(posedge fast_clock) if (freset || RUN && (RH33     || RH32    ))  RH33        <= `tFCQ   freset ? 1'b0 :    ~RH33;
always @(posedge fast_clock) if (freset || RUN && (RH34[04] || RH33    ))  RH34[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RH34[04]}};     //fo= 32   4  +1
always @(posedge fast_clock) if (freset || RUN && (RH35[09] || RH34[00]))  RH35[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RH35[09]}};     //fo= 64   8  +1
always @(posedge fast_clock) if (freset || RUN && (RH35[04] || RH34[00]))  RH35[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RH35[04]}};     //fo= 64   8  +1
always @(posedge fast_clock) if (freset || RUN && (RH36[14] || RH35[09]))  RH36[14:10] <= `tFCQ   freset ? 5'b0 : {5{~RH36[14]}};     //fo= 96  12  14 13:10
always @(posedge fast_clock) if (freset || RUN && (RH36[09] || RH35[09]))  RH36[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RH36[09]}};     //fo= 96  12  09 08:05
always @(posedge fast_clock) if (freset || RUN && (RH36[04] || RH35[04]))  RH36[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RH36[04]}};     //fo= 96  12  04 03:00
always @(posedge fast_clock) if (freset || RUN && (RH37[19] || RH36[14]))  RH37[19:15] <= `tFCQ   freset ? 1'b0 : {5{~RH37[19]}};     //fo=128  16  19 18:15
always @(posedge fast_clock) if (freset || RUN && (RH37[14] || RH36[14]))  RH37[14:10] <= `tFCQ   freset ? 1'b0 : {5{~RH37[14]}};     //fo=128  16  14 13:10
always @(posedge fast_clock) if (freset || RUN && (RH37[09] || RH36[09]))  RH37[09:05] <= `tFCQ   freset ? 1'b0 : {5{~RH37[09]}};     //fo=128  16  09 08:05
always @(posedge fast_clock) if (freset || RUN && (RH37[04] || RH36[04]))  RH37[04:00] <= `tFCQ   freset ? 1'b0 : {5{~RH37[04]}};     //fo=128  16  04 03:00
always @(posedge fast_clock) if (freset || RUN && (RH38[14] || RH37[19]))  RH38[14:10] <= `tFCQ   freset ? 1'b0 : {5{~RH38[14]}};     //fo= 96  12  14 13:10
always @(posedge fast_clock) if (freset || RUN && (RH38[09] || RH37[14]))  RH38[09:05] <= `tFCQ   freset ? 1'b0 : {5{~RH38[09]}};     //fo= 96  12  09 08:05
always @(posedge fast_clock) if (freset || RUN && (RH38[04] || RH37[09]))  RH38[04:00] <= `tFCQ   freset ? 1'b0 : {5{~RH38[04]}};     //fo= 96  12  04 03:00
always @(posedge fast_clock) if (freset || RUN && (RH39[09] || RH38[14]))  RH39[09:05] <= `tFCQ   freset ? 5'b0 : {5{~RH39[09]}};     //fo= 64   8  09 08:05
always @(posedge fast_clock) if (freset || RUN && (RH39[04] || RH38[09]))  RH39[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RH39[04]}};     //fo= 64   8  04 03:00
always @(posedge fast_clock) if (freset || RUN && (RH40[04] || RH39[08]))  RH40[04:00] <= `tFCQ   freset ? 5'b0 : {5{~RH40[04]}};     //fo= 32   4  04 03:00
//-----------------------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (freset || RUN && ( FS0[32] || fast_shift)) FS0[32:00] <= `tFCQ   freset ? 32'b0 :{32{~FS0[32]}};     //fo= 1024

wire  [31:0] ri = fast_load ? IN[31:0] : r0_X[0];

// r0_Y loaded from r0_X
//    inst    output    enable       input     clock
always @(posedge fast_clock) if (RM00[18])   r0_Y[00][31:24] <= `tFCQ  r0_X[00][31:24];
always @(posedge fast_clock) if (RM00[17])   r0_Y[00][23:16] <= `tFCQ  r0_X[00][23:16];
always @(posedge fast_clock) if (RM00[16])   r0_Y[00][15:08] <= `tFCQ  r0_X[00][15:08];
always @(posedge fast_clock) if (RM00[15])   r0_Y[00][07:00] <= `tFCQ  r0_X[00][07:00];
always @(posedge fast_clock) if (RM01[03])   r0_Y[01][31:24] <= `tFCQ  r0_X[01][31:24];
always @(posedge fast_clock) if (RM01[02])   r0_Y[01][23:16] <= `tFCQ  r0_X[01][23:16];
always @(posedge fast_clock) if (RM01[01])   r0_Y[01][15:08] <= `tFCQ  r0_X[01][15:08];
always @(posedge fast_clock) if (RM01[00])   r0_Y[01][07:00] <= `tFCQ  r0_X[01][07:00];
always @(posedge fast_clock) if (RM02[08])   r0_Y[02][31:24] <= `tFCQ  r0_X[02][31:24];
always @(posedge fast_clock) if (RM02[07])   r0_Y[02][23:16] <= `tFCQ  r0_X[02][23:16];
always @(posedge fast_clock) if (RM02[06])   r0_Y[02][15:08] <= `tFCQ  r0_X[02][15:08];
always @(posedge fast_clock) if (RM02[05])   r0_Y[02][07:00] <= `tFCQ  r0_X[02][07:00];
always @(posedge fast_clock) if (RM03[13])   r0_Y[03][31:24] <= `tFCQ  r0_X[03][31:24];
always @(posedge fast_clock) if (RM03[12])   r0_Y[03][23:16] <= `tFCQ  r0_X[03][23:16];
always @(posedge fast_clock) if (RM03[11])   r0_Y[03][15:08] <= `tFCQ  r0_X[03][15:08];
always @(posedge fast_clock) if (RM03[10])   r0_Y[03][07:00] <= `tFCQ  r0_X[03][07:00];
always @(posedge fast_clock) if (RM00[13])   r0_Y[04][31:24] <= `tFCQ  r0_X[04][31:24];
always @(posedge fast_clock) if (RM00[12])   r0_Y[04][23:16] <= `tFCQ  r0_X[04][23:16];
always @(posedge fast_clock) if (RM00[11])   r0_Y[04][15:08] <= `tFCQ  r0_X[04][15:08];
always @(posedge fast_clock) if (RM00[10])   r0_Y[04][07:00] <= `tFCQ  r0_X[04][07:00];
always @(posedge fast_clock) if (RM01[18])   r0_Y[05][31:24] <= `tFCQ  r0_X[05][31:24];
always @(posedge fast_clock) if (RM01[17])   r0_Y[05][23:16] <= `tFCQ  r0_X[05][23:16];
always @(posedge fast_clock) if (RM01[16])   r0_Y[05][15:08] <= `tFCQ  r0_X[05][15:08];
always @(posedge fast_clock) if (RM01[15])   r0_Y[05][07:00] <= `tFCQ  r0_X[05][07:00];
always @(posedge fast_clock) if (RM02[03])   r0_Y[06][31:24] <= `tFCQ  r0_X[06][31:24];
always @(posedge fast_clock) if (RM02[02])   r0_Y[06][23:16] <= `tFCQ  r0_X[06][23:16];
always @(posedge fast_clock) if (RM02[01])   r0_Y[06][15:08] <= `tFCQ  r0_X[06][15:08];
always @(posedge fast_clock) if (RM02[00])   r0_Y[06][07:00] <= `tFCQ  r0_X[06][07:00];
always @(posedge fast_clock) if (RM03[08])   r0_Y[07][31:24] <= `tFCQ  r0_X[07][31:24];
always @(posedge fast_clock) if (RM03[07])   r0_Y[07][23:16] <= `tFCQ  r0_X[07][23:16];
always @(posedge fast_clock) if (RM03[06])   r0_Y[07][15:08] <= `tFCQ  r0_X[07][15:08];
always @(posedge fast_clock) if (RM03[05])   r0_Y[07][07:00] <= `tFCQ  r0_X[07][07:00];
always @(posedge fast_clock) if (RM00[08])   r0_Y[08][31:24] <= `tFCQ  r0_X[08][31:24];
always @(posedge fast_clock) if (RM00[07])   r0_Y[08][23:16] <= `tFCQ  r0_X[08][23:16];
always @(posedge fast_clock) if (RM00[06])   r0_Y[08][15:08] <= `tFCQ  r0_X[08][15:08];
always @(posedge fast_clock) if (RM00[05])   r0_Y[08][07:00] <= `tFCQ  r0_X[08][07:00];
always @(posedge fast_clock) if (RM01[13])   r0_Y[09][31:24] <= `tFCQ  r0_X[09][31:24];
always @(posedge fast_clock) if (RM01[12])   r0_Y[09][23:16] <= `tFCQ  r0_X[09][23:16];
always @(posedge fast_clock) if (RM01[11])   r0_Y[09][15:08] <= `tFCQ  r0_X[09][15:08];
always @(posedge fast_clock) if (RM01[10])   r0_Y[09][07:00] <= `tFCQ  r0_X[09][07:00];
always @(posedge fast_clock) if (RM00[18])   r0_Y[10][31:24] <= `tFCQ  r0_X[10][31:24];
always @(posedge fast_clock) if (RM00[17])   r0_Y[10][23:16] <= `tFCQ  r0_X[10][23:16];
always @(posedge fast_clock) if (RM00[16])   r0_Y[10][15:08] <= `tFCQ  r0_X[10][15:08];
always @(posedge fast_clock) if (RM00[15])   r0_Y[10][07:00] <= `tFCQ  r0_X[10][07:00];
always @(posedge fast_clock) if (RM03[03])   r0_Y[11][31:24] <= `tFCQ  r0_X[11][31:24];
always @(posedge fast_clock) if (RM03[02])   r0_Y[11][23:16] <= `tFCQ  r0_X[11][23:16];
always @(posedge fast_clock) if (RM03[01])   r0_Y[11][15:08] <= `tFCQ  r0_X[11][15:08];
always @(posedge fast_clock) if (RM03[00])   r0_Y[11][07:00] <= `tFCQ  r0_X[11][07:00];
always @(posedge fast_clock) if (RM00[03])   r0_Y[12][31:24] <= `tFCQ  r0_X[12][31:24];
always @(posedge fast_clock) if (RM00[02])   r0_Y[12][23:16] <= `tFCQ  r0_X[12][23:16];
always @(posedge fast_clock) if (RM00[01])   r0_Y[12][15:08] <= `tFCQ  r0_X[12][15:08];
always @(posedge fast_clock) if (RM00[00])   r0_Y[12][07:00] <= `tFCQ  r0_X[12][07:00];
always @(posedge fast_clock) if (RM01[08])   r0_Y[13][31:24] <= `tFCQ  r0_X[13][31:24];
always @(posedge fast_clock) if (RM01[07])   r0_Y[13][23:16] <= `tFCQ  r0_X[13][23:16];
always @(posedge fast_clock) if (RM01[06])   r0_Y[13][15:08] <= `tFCQ  r0_X[13][15:08];
always @(posedge fast_clock) if (RM01[05])   r0_Y[13][07:00] <= `tFCQ  r0_X[13][07:00];
always @(posedge fast_clock) if (RM02[13])   r0_Y[14][31:24] <= `tFCQ  r0_X[14][31:24];
always @(posedge fast_clock) if (RM02[12])   r0_Y[14][23:16] <= `tFCQ  r0_X[14][23:16];
always @(posedge fast_clock) if (RM02[11])   r0_Y[14][15:08] <= `tFCQ  r0_X[14][15:08];
always @(posedge fast_clock) if (RM02[10])   r0_Y[14][07:00] <= `tFCQ  r0_X[14][07:00];
always @(posedge fast_clock) if (RM03[18])   r0_Y[15][31:24] <= `tFCQ  r0_X[15][31:24];
always @(posedge fast_clock) if (RM03[17])   r0_Y[15][23:16] <= `tFCQ  r0_X[15][23:16];
always @(posedge fast_clock) if (RM03[16])   r0_Y[15][15:08] <= `tFCQ  r0_X[15][15:08];
always @(posedge fast_clock) if (RM03[15])   r0_Y[15][07:00] <= `tFCQ  r0_X[15][07:00];
always @(posedge fast_clock) if (RM00[38])   r0_Y[16][31:24] <= `tFCQ  r0_X[16][31:24];
always @(posedge fast_clock) if (RM00[37])   r0_Y[16][23:16] <= `tFCQ  r0_X[16][23:16];
always @(posedge fast_clock) if (RM00[36])   r0_Y[16][15:08] <= `tFCQ  r0_X[16][15:08];
always @(posedge fast_clock) if (RM00[35])   r0_Y[16][07:00] <= `tFCQ  r0_X[16][07:00];
always @(posedge fast_clock) if (RM01[23])   r0_Y[17][31:24] <= `tFCQ  r0_X[17][31:24];
always @(posedge fast_clock) if (RM01[22])   r0_Y[17][23:16] <= `tFCQ  r0_X[17][23:16];
always @(posedge fast_clock) if (RM01[21])   r0_Y[17][15:08] <= `tFCQ  r0_X[17][15:08];
always @(posedge fast_clock) if (RM01[20])   r0_Y[17][07:00] <= `tFCQ  r0_X[17][07:00];
always @(posedge fast_clock) if (RM02[28])   r0_Y[18][31:24] <= `tFCQ  r0_X[18][31:24];
always @(posedge fast_clock) if (RM02[27])   r0_Y[18][23:16] <= `tFCQ  r0_X[18][23:16];
always @(posedge fast_clock) if (RM02[26])   r0_Y[18][15:08] <= `tFCQ  r0_X[18][15:08];
always @(posedge fast_clock) if (RM02[25])   r0_Y[18][07:00] <= `tFCQ  r0_X[18][07:00];
always @(posedge fast_clock) if (RM03[33])   r0_Y[19][31:24] <= `tFCQ  r0_X[19][31:24];
always @(posedge fast_clock) if (RM03[32])   r0_Y[19][23:16] <= `tFCQ  r0_X[19][23:16];
always @(posedge fast_clock) if (RM03[31])   r0_Y[19][15:08] <= `tFCQ  r0_X[19][15:08];
always @(posedge fast_clock) if (RM03[30])   r0_Y[19][07:00] <= `tFCQ  r0_X[19][07:00];
always @(posedge fast_clock) if (RM00[33])   r0_Y[20][31:24] <= `tFCQ  r0_X[20][31:24];
always @(posedge fast_clock) if (RM00[32])   r0_Y[20][23:16] <= `tFCQ  r0_X[20][23:16];
always @(posedge fast_clock) if (RM00[31])   r0_Y[20][15:08] <= `tFCQ  r0_X[20][15:08];
always @(posedge fast_clock) if (RM00[30])   r0_Y[20][07:00] <= `tFCQ  r0_X[20][07:00];
always @(posedge fast_clock) if (RM01[38])   r0_Y[21][31:24] <= `tFCQ  r0_X[21][31:24];
always @(posedge fast_clock) if (RM01[37])   r0_Y[21][23:16] <= `tFCQ  r0_X[21][23:16];
always @(posedge fast_clock) if (RM01[36])   r0_Y[21][15:08] <= `tFCQ  r0_X[21][15:08];
always @(posedge fast_clock) if (RM01[35])   r0_Y[21][07:00] <= `tFCQ  r0_X[21][07:00];
always @(posedge fast_clock) if (RM02[23])   r0_Y[22][31:24] <= `tFCQ  r0_X[22][31:24];
always @(posedge fast_clock) if (RM02[22])   r0_Y[22][23:16] <= `tFCQ  r0_X[22][23:16];
always @(posedge fast_clock) if (RM02[21])   r0_Y[22][15:08] <= `tFCQ  r0_X[22][15:08];
always @(posedge fast_clock) if (RM02[20])   r0_Y[22][07:00] <= `tFCQ  r0_X[22][07:00];
always @(posedge fast_clock) if (RM03[28])   r0_Y[23][31:24] <= `tFCQ  r0_X[23][31:24];
always @(posedge fast_clock) if (RM03[27])   r0_Y[23][23:16] <= `tFCQ  r0_X[23][23:16];
always @(posedge fast_clock) if (RM03[26])   r0_Y[23][15:08] <= `tFCQ  r0_X[23][15:08];
always @(posedge fast_clock) if (RM03[25])   r0_Y[23][07:00] <= `tFCQ  r0_X[23][07:00];
always @(posedge fast_clock) if (RM00[28])   r0_Y[24][31:24] <= `tFCQ  r0_X[24][31:24];
always @(posedge fast_clock) if (RM00[27])   r0_Y[24][23:16] <= `tFCQ  r0_X[24][23:16];
always @(posedge fast_clock) if (RM00[26])   r0_Y[24][15:08] <= `tFCQ  r0_X[24][15:08];
always @(posedge fast_clock) if (RM00[25])   r0_Y[24][07:00] <= `tFCQ  r0_X[24][07:00];
always @(posedge fast_clock) if (RM01[33])   r0_Y[25][31:24] <= `tFCQ  r0_X[25][31:24];
always @(posedge fast_clock) if (RM01[32])   r0_Y[25][23:16] <= `tFCQ  r0_X[25][23:16];
always @(posedge fast_clock) if (RM01[31])   r0_Y[25][15:08] <= `tFCQ  r0_X[25][15:08];
always @(posedge fast_clock) if (RM01[30])   r0_Y[25][07:00] <= `tFCQ  r0_X[25][07:00];
always @(posedge fast_clock) if (RM02[38])   r0_Y[26][31:24] <= `tFCQ  r0_X[26][31:24];
always @(posedge fast_clock) if (RM02[37])   r0_Y[26][23:16] <= `tFCQ  r0_X[26][23:16];
always @(posedge fast_clock) if (RM02[36])   r0_Y[26][15:08] <= `tFCQ  r0_X[26][15:08];
always @(posedge fast_clock) if (RM02[35])   r0_Y[26][07:00] <= `tFCQ  r0_X[26][07:00];
always @(posedge fast_clock) if (RM03[23])   r0_Y[27][31:24] <= `tFCQ  r0_X[27][31:24];
always @(posedge fast_clock) if (RM03[22])   r0_Y[27][23:16] <= `tFCQ  r0_X[27][23:16];
always @(posedge fast_clock) if (RM03[21])   r0_Y[27][15:08] <= `tFCQ  r0_X[27][15:08];
always @(posedge fast_clock) if (RM03[20])   r0_Y[27][07:00] <= `tFCQ  r0_X[27][07:00];
always @(posedge fast_clock) if (RM00[23])   r0_Y[28][31:24] <= `tFCQ  r0_X[28][31:24];
always @(posedge fast_clock) if (RM00[22])   r0_Y[28][23:16] <= `tFCQ  r0_X[28][23:16];
always @(posedge fast_clock) if (RM00[21])   r0_Y[28][15:08] <= `tFCQ  r0_X[28][15:08];
always @(posedge fast_clock) if (RM00[20])   r0_Y[28][07:00] <= `tFCQ  r0_X[28][07:00];
always @(posedge fast_clock) if (RM01[28])   r0_Y[29][31:24] <= `tFCQ  r0_X[29][31:24];
always @(posedge fast_clock) if (RM01[27])   r0_Y[29][23:16] <= `tFCQ  r0_X[29][23:16];
always @(posedge fast_clock) if (RM01[26])   r0_Y[29][15:08] <= `tFCQ  r0_X[29][15:08];
always @(posedge fast_clock) if (RM01[25])   r0_Y[29][07:00] <= `tFCQ  r0_X[29][07:00];
always @(posedge fast_clock) if (RM02[33])   r0_Y[30][31:24] <= `tFCQ  r0_X[30][31:24];
always @(posedge fast_clock) if (RM02[32])   r0_Y[30][23:16] <= `tFCQ  r0_X[30][23:16];
always @(posedge fast_clock) if (RM02[31])   r0_Y[30][15:08] <= `tFCQ  r0_X[30][15:08];
always @(posedge fast_clock) if (RM02[30])   r0_Y[30][07:00] <= `tFCQ  r0_X[30][07:00];
always @(posedge fast_clock) if (RM03[38])   r0_Y[31][31:24] <= `tFCQ  r0_X[31][31:24];
always @(posedge fast_clock) if (RM03[37])   r0_Y[31][23:16] <= `tFCQ  r0_X[31][23:16];
always @(posedge fast_clock) if (RM03[36])   r0_Y[31][15:08] <= `tFCQ  r0_X[31][15:08];
always @(posedge fast_clock) if (RM03[35])   r0_Y[31][07:00] <= `tFCQ  r0_X[31][07:00];

// r0_X Register Array 4 input ports
//    inst    output    enable3      enable2      enable1      enable0       input3        input2 input1  input0    clock
regX4 i_rx01(.q(r0_X[00][31:0]),.e3(RM00[03:00]),.e2(RL00[03:00]),.e1(RL34[03:00]),.e0({4{FS0[00]}}),.d3(WMo[127:096]),.d2(b0_ZA),.d1(r2_ADD),.d0(r0_X[01]),.clock(fast_clock));
regX4 i_rx02(.q(r0_X[01][31:0]),.e3(RM01[03:00]),.e2(RL01[03:00]),.e1(RL38[03:00]),.e0({4{FS0[01]}}),.d3(WMo[031:000]),.d2(b0_ZD),.d1(r5_ADD),.d0(r0_X[02]),.clock(fast_clock));
regX4 i_rx03(.q(r0_X[02][31:0]),.e3(RM02[03:00]),.e2(RL02[08:05]),.e1(RL38[08:05]),.e0({4{FS0[02]}}),.d3(WMo[063:032]),.d2(b0_ZC),.d1(r4_ADD),.d0(r0_X[08]),.clock(fast_clock));
regX4 i_rx04(.q(r0_X[03][31:0]),.e3(RM03[03:00]),.e2(RL03[13:10]),.e1(RL38[13:10]),.e0({4{FS0[03]}}),.d3(WMo[095:064]),.d2(b0_ZB),.d1(r3_ADD),.d0(r0_X[04]),.clock(fast_clock));
regX4 i_rx05(.q(r0_X[04][31:0]),.e3(RM00[08:05]),.e2(RL00[08:05]),.e1(RL35[03:00]),.e0({4{FS0[04]}}),.d3(WMo[095:064]),.d2(b0_ZB),.d1(r3_ADD),.d0(r0_X[05]),.clock(fast_clock));
regX4 i_rx06(.q(r0_X[05][31:0]),.e3(RM01[08:05]),.e2(RL01[13:10]),.e1(RL35[08:05]),.e0({4{FS0[05]}}),.d3(WMo[127:096]),.d2(b0_ZA),.d1(r2_ADD),.d0(r0_X[06]),.clock(fast_clock));
regX4 i_rx07(.q(r0_X[06][31:0]),.e3(RM02[08:05]),.e2(RL02[03:00]),.e1(RL39[03:00]),.e0({4{FS0[06]}}),.d3(WMo[031:000]),.d2(b0_ZD),.d1(r5_ADD),.d0(r0_X[12]),.clock(fast_clock));
regX4 i_rx08(.q(r0_X[07][31:0]),.e3(RM03[08:05]),.e2(RL03[08:05]),.e1(RL39[08:05]),.e0({4{FS0[07]}}),.d3(WMo[063:032]),.d2(b0_ZC),.d1(r4_ADD),.d0(r0_X[08]),.clock(fast_clock));
regX4 i_rx09(.q(r0_X[08][31:0]),.e3(RM00[13:10]),.e2(RL00[13:10]),.e1(RL36[03:00]),.e0({4{FS0[08]}}),.d3(WMo[063:032]),.d2(b0_ZC),.d1(r4_ADD),.d0(r0_X[09]),.clock(fast_clock));
regX4 i_rx10(.q(r0_X[09][31:0]),.e3(RM01[13:10]),.e2(RL01[08:05]),.e1(RL36[08:05]),.e0({4{FS0[09]}}),.d3(WMo[095:064]),.d2(b0_ZB),.d1(r3_ADD),.d0(r0_X[10]),.clock(fast_clock));
regX4 i_rx11(.q(r0_X[10][31:0]),.e3(RM02[13:10]),.e2(RL02[13:10]),.e1(RL36[13:10]),.e0({4{FS0[10]}}),.d3(WMo[127:096]),.d2(b0_ZA),.d1(r2_ADD),.d0(r0_X[16]),.clock(fast_clock));
regX4 i_rx12(.q(r0_X[11][31:0]),.e3(RM03[13:10]),.e2(RL03[03:00]),.e1(RL40[03:00]),.e0({4{FS0[11]}}),.d3(WMo[031:000]),.d2(b0_ZD),.d1(r5_ADD),.d0(r0_X[12]),.clock(fast_clock));
regX4 i_rx13(.q(r0_X[12][31:0]),.e3(RM00[18:15]),.e2(RL00[18:15]),.e1(RL37[03:00]),.e0({4{FS0[12]}}),.d3(WMo[031:000]),.d2(b0_ZD),.d1(r5_ADD),.d0(r0_X[13]),.clock(fast_clock));
regX4 i_rx14(.q(r0_X[13][31:0]),.e3(RM01[18:15]),.e2(RL01[18:15]),.e1(RL37[08:05]),.e0({4{FS0[13]}}),.d3(WMo[063:032]),.d2(b0_ZC),.d1(r4_ADD),.d0(r0_X[14]),.clock(fast_clock));
regX4 i_rx15(.q(r0_X[14][31:0]),.e3(RM02[18:15]),.e2(RL02[18:15]),.e1(RL37[13:10]),.e0({4{FS0[14]}}),.d3(WMo[095:064]),.d2(b0_ZB),.d1(r3_ADD),.d0(r0_X[04]),.clock(fast_clock));
regX4 i_rx16(.q(r0_X[15][31:0]),.e3(RM03[18:15]),.e2(RL03[18:15]),.e1(RL37[18:15]),.e0({4{FS0[15]}}),.d3(WMo[127:096]),.d2(b0_ZA),.d1(r2_ADD),.d0(r0_X[16]),.clock(fast_clock));

// r0_X Register Array 5 input ports
//    inst    output    enable4      enable3      enable2                 enable1      enable0       input4        input3 input2 input1  input0    clock
regX5 i_rx17(r0_X[16], RM00[23:20], RH00[03:00], RL00[23:20] & {4{MIX}}, RH34[03:00], {4{FS0[16]}}, WMo[255:224], b0_ZA, b0_XA, r2_ADD, r0_X[17], fast_clock);
regX5 i_rx18(r0_X[17], RM01[23:20], RH01[03:00], RL01[23:20] & {4{MIX}}, RH38[03:00], {4{FS0[17]}}, WMo[159:128], b0_ZD, b0_XD, r5_ADD, r0_X[18], fast_clock);
regX5 i_rx19(r0_X[18], RM02[23:20], RH02[08:05], RL02[28:25] & {4{MIX}}, RH38[08:05], {4{FS0[18]}}, WMo[191:160], b0_ZC, b0_XC, r4_ADD, r0_X[24], fast_clock);
regX5 i_rx20(r0_X[19], RM03[23:20], RH03[13:10], RL03[33:30] & {4{MIX}}, RH38[13:10], {4{FS0[19]}}, WMo[223:192], b0_ZB, b0_XB, r3_ADD, r0_X[20], fast_clock);
regX5 i_rx21(r0_X[20], RM00[28:25], RH00[08:05], RL00[28:25] & {4{MIX}}, RH35[03:00], {4{FS0[20]}}, WMo[223:192], b0_ZB, b0_XB, r3_ADD, r0_X[21], fast_clock);
regX5 i_rx22(r0_X[21], RM01[28:25], RH01[13:10], RL01[33:30] & {4{MIX}}, RH35[08:05], {4{FS0[21]}}, WMo[255:224], b0_ZA, b0_XA, r2_ADD, r0_X[22], fast_clock);
regX5 i_rx23(r0_X[22], RM02[28:25], RH02[03:00], RL02[23:20] & {4{MIX}}, RH39[03:00], {4{FS0[22]}}, WMo[159:128], b0_ZD, b0_XD, r5_ADD, r0_X[28], fast_clock);
regX5 i_rx24(r0_X[23], RM03[28:25], RH03[08:05], RL03[28:25] & {4{MIX}}, RH39[08:05], {4{FS0[23]}}, WMo[191:160], b0_ZC, b0_XC, r4_ADD, r0_X[24], fast_clock);
regX5 i_rx25(r0_X[24], RM00[33:30], RH00[13:10], RL00[33:30] & {4{MIX}}, RH36[03:00], {4{FS0[24]}}, WMo[191:160], b0_ZC, b0_XC, r4_ADD, r0_X[25], fast_clock);
regX5 i_rx26(r0_X[25], RM01[33:30], RH01[08:05], RL01[28:25] & {4{MIX}}, RH36[08:05], {4{FS0[25]}}, WMo[223:192], b0_ZB, b0_XB, r3_ADD, r0_X[26], fast_clock);
regX5 i_rx27(r0_X[26], RM02[33:30], RH02[13:10], RL02[33:30] & {4{MIX}}, RH36[13:10], {4{FS0[26]}}, WMo[255:224], b0_ZA, b0_XA, r2_ADD, r0_X[27], fast_clock);
regX5 i_rx28(r0_X[27], RM03[33:30], RH03[03:00], RL03[23:20] & {4{MIX}}, RH40[03:00], {4{FS0[27]}}, WMo[159:128], b0_ZD, b0_XD, r5_ADD, r0_X[28], fast_clock);
regX5 i_rx29(r0_X[28], RM00[38:35], RH00[18:15], RL00[38:35] & {4{MIX}}, RH37[03:00], {4{FS0[28]}}, WMo[159:128], b0_ZD, b0_XD, r5_ADD, r0_X[29], fast_clock);
regX5 i_rx30(r0_X[29], RM01[38:35], RH01[18:15], RL01[38:35] & {4{MIX}}, RH37[08:05], {4{FS0[29]}}, WMo[191:160], b0_ZC, b0_XC, r4_ADD, r0_X[30], fast_clock);
regX5 i_rx31(r0_X[30], RM02[38:35], RH02[18:15], RL02[38:35] & {4{MIX}}, RH37[13:10], {4{FS0[30]}}, WMo[223:192], b0_ZB, b0_XB, r3_ADD, r0_X[20], fast_clock);
regX5 i_rx32(r0_X[31], RM03[38:35], RH03[18:15], RL03[38:35] & {4{MIX}}, RH37[18:15], {4{FS0[31]}}, WMo[255:224], b0_ZA, b0_XA, r2_ADD, ri[31:0], fast_clock);

`endif  // REORG

//===============================================================================================================================
// Main Datapath to execute pipeline

// Select Low  Buffer entries
assign   {b0_LA,b0_LB,b0_LC,b0_LD}  =
            {128{RLH0}} & {r0_X[00], r0_X[04], r0_X[08], r0_X[12]}                                                      //fo=128
         |  {128{RLH1}} & {r0_X[05], r0_X[09], r0_X[13], r0_X[01]}                                                      //fo=128
         |  {128{RLH2}} & {r0_X[10], r0_X[14], r0_X[02], r0_X[06]}                                                      //fo=128
         |  {128{RLH3}} & {r0_X[15], r0_X[03], r0_X[07], r0_X[11]};                                                     //fo=128

// Select High Buffer entries
assign   {b0_HA,b0_HB,b0_HC,b0_HD}  =
            {128{RLH0}} & {r0_X[16], r0_X[20], r0_X[24], r0_X[28]}                                                      //fo=128
         |  {128{RLH1}} & {r0_X[21], r0_X[25], r0_X[29], r0_X[17]}                                                      //fo=128
         |  {128{RLH2}} & {r0_X[26], r0_X[30], r0_X[18], r0_X[22]}                                                      //fo=128
         |  {128{RLH3}} & {r0_X[31], r0_X[19], r0_X[23], r0_X[27]};                                                     //fo=128

// As the next operation is an XOR (which is commutative) we do NOT swap sides again so, Y holds the saved value of X from
// the last iteration and X hold the interpolated value replacing the Memory data
assign   Saved[255:0] =
            {256{RL00}} & {r0_Y[16], r0_Y[20], r0_Y[24], r0_Y[28], r0_Y[00], r0_Y[04], r0_Y[08], r0_Y[12]}              //fo=256
         |  {256{RL01}} & {r0_Y[21], r0_Y[25], r0_Y[29], r0_Y[17], r0_Y[05], r0_Y[09], r0_Y[13], r0_Y[01]}              //fo=256
         |  {256{RL02}} & {r0_Y[26], r0_Y[30], r0_Y[18], r0_Y[22], r0_Y[10], r0_Y[14], r0_Y[02], r0_Y[06]}              //fo=256
         |  {256{RL03}} & {r0_Y[31], r0_Y[19], r0_Y[23], r0_Y[27], r0_Y[15], r0_Y[03], r0_Y[07], r0_Y[11]};             //fo=256

assign   Readata[127:0] =    WMo[255:128] ^   WMo[127:0],
         Restore[127:0] =  Saved[255:128] ^ Saved[127:0],
         MemData[127:0] =  (SMIXs ?  Saved[255:128] :  WMo[255:128]);                                                   //fo=256

assign   {b0_VA, b0_VB, b0_VC, b0_VD}  =   SMIXs ?  Restore[127:0] :  Readata[127:0];

// XOR against memory when mixing
assign   {b0_UA, b0_UB, b0_UC, b0_UD}  =  {b0_HA, b0_HB, b0_HC, b0_HD} ^ {b0_LA, b0_LB, b0_LC, b0_LD};   // XOR high and low
assign   {b0_ZA, b0_ZB, b0_ZC, b0_ZD}  =  {b0_UA, b0_UB, b0_UC, b0_UD} ^ {b0_VA, b0_VB, b0_VC, b0_VD};   // XOR U & V
assign   {b0_XA, b0_XB, b0_XC, b0_XD}  =  {b0_HA, b0_HB, b0_HC, b0_HD} ^  MemData[127:0];                // XOR high and mem

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
// Work Memory Interface ========================================================================================================
//===============================================================================================================================
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

//===============================================================================================================================
// Execution Pipeline ===========================================================================================================
//===============================================================================================================================

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
   input  wire          gate,
   input  wire          fast_clock
   );

// Control
//(* SiART:  All these are physical flip-flops and should be 'CLONED' and 'RETIMED'  *)

/*(* SiART: CLONE and RETIME *)*/ reg   [39:32]  PP;
/*(* SiART: CLONE and RETIME *)*/ reg            PRIME;
/*(* SiART: CLONE and RETIME *)*/ reg            P00,  P01,  P02,  P03;
/*(* SiART: CLONE and RETIME *)*/ reg            P0P1,P0P1P2,P1P2P3;
/*(* SiART: CLONE and RETIME *)*/ reg            add2dgstA, add2dgstB, add2dgstC, add2dgstD, add2dgstD2;

always @(posedge fast_clock) if (E_head_P || P00      )  P00         <= `tFCQ  E_head_P;
always @(posedge fast_clock) if (P00      || P01      )  P01         <= `tFCQ  P00;
always @(posedge fast_clock) if (P01      || P02      )  P02         <= `tFCQ  P01;
always @(posedge fast_clock) if (P02      || P03      )  P03         <= `tFCQ  P02;

always @(posedge fast_clock) if (E_head_P || P01      )  P0P1        <= `tFCQ  E_head_P;
always @(posedge fast_clock) if (E_head_P || P02      )  P0P1P2      <= `tFCQ  E_head_P;
always @(posedge fast_clock) if (P00      || P03      )  P1P2P3      <= `tFCQ  P00;

always @(posedge fast_clock) if (E_tail   || PP[32]   )  PP[32]      <= `tFCQ E_tail;
always @(posedge fast_clock) if (PP[32]   || PP[33]   )  PP[33]      <= `tFCQ PP[32];
always @(posedge fast_clock) if (PP[33]   || PP[34]   )  PP[34]      <= `tFCQ PP[33];
always @(posedge fast_clock) if (PP[34]   || PP[35]   )  PP[35]      <= `tFCQ PP[34];
always @(posedge fast_clock) if (PP[35]   || PP[36]   )  PP[36]      <= `tFCQ PP[35];
always @(posedge fast_clock) if (PP[36]   || PP[37]   )  PP[37]      <= `tFCQ PP[36];
always @(posedge fast_clock) if (PP[37]   || PP[38]   )  PP[38]      <= `tFCQ PP[37];
always @(posedge fast_clock) if (PP[38]   || PP[39]   )  PP[39]      <= `tFCQ PP[38];

always @(posedge fast_clock) if (E_PRIME  || PRIME    )  PRIME       <= `tFCQ E_PRIME;

always @(posedge fast_clock) if (E_tail   || PP[35]   )  add2dgstD   <= `tFCQ  E_tail;
always @(posedge fast_clock) if (PP[32]   || PP[36]   )  add2dgstA   <= `tFCQ  PP[32];
always @(posedge fast_clock) if (PP[33]   || PP[37]   )  add2dgstB   <= `tFCQ  PP[33];
always @(posedge fast_clock) if (PP[34]   || PP[38]   )  add2dgstC   <= `tFCQ  PP[34];
always @(posedge fast_clock) if (PP[35]   || PP[39]   )  add2dgstD2  <= `tFCQ  PP[35];

// Pipeline Registers   *****[ DYNAMIC LATCHES ]*****
reg   [31:0]   r1_A, r1_B, r1_C, r1_D, r1_E,
               r2_A, r2_B, r2_C, r2_D,
               r3_A, r3_B, r3_C, r3_D,
               r4_A, r4_B, r4_C, r4_D,
                     r5_B, r5_C, r5_D,
                           r6_C;

// Pipeline Arithmetic
wire  [31:0]   b1_ARX, b2_ARX, b3_ARX, b4_ARX,              // Results of ARX computations
               b1_ADD, b2_ADD, b3_ADD, b4_ADD;              // Results of ADDs

// Pipeline Input Multiplexing
wire  [31:0]   n0_B  =  !P03                   ?   b0_ZB    // New input
                     :                             r5_D,    // Registered feedback

               n0_C  =  !P1P2P3                ?   b0_ZC    // New input
                     :   P03                   ?   r5_C     // Registered feedback
                     :                             r6_C,    // Registered feedback

               n0_D  =  !add2dgstD && !P1P2P3  ?   b0_ZD    // New input
                     :   add2dgstD             ?   b0_DD    // Digest
                     :                             r5_B;    // Registered feedback

// Pipeline Stage 1 -------------------------------------------------------------- Unregistered feedback :  higher mux ----------
always @(posedge fast_clock) if (gate)                r1_A     <= `tFCQ !PRIME            ?  b4_ARX   :  b0_ZA;
always @(posedge fast_clock) if (gate)                r1_B     <= `tFCQ  P0P1P2           ?  b3_ARX   :  n0_B;
always @(posedge fast_clock) if (gate)                r1_C     <= `tFCQ  P0P1             ?  b2_ARX   :  n0_C;
always @(posedge fast_clock) if (gate)                r1_D     <= `tFCQ !add2dgstD && P00 ?  b1_ARX   :  n0_D;
always @(posedge fast_clock) if (gate && add2dgstD)   r1_E     <= `tFCQ  P00              ?  b1_ARX   :  r5_B;

`define RR(x,y)  {x[y-1:0],x[31:y]}

assign {b1_ADD[31:0], `RR(b1_ARX,7)} = A2_ARX32 ( r1_A[31:0], r1_D[31:0], `RR(r1_B,7) ); //{r1_B[6:0],r1_B[31:7]});

assign  b1_ADR[9:0]    =  b1_ADD[9:0];

// Pipeline Stage 2 -------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (gate && add2dgstA)   r2_ADD   <= `tFCQ  b1_ADD;
always @(posedge fast_clock) if (gate)                r2_B     <= `tFCQ  add2dgstA  ?  r1_B  :  b1_ARX;
always @(posedge fast_clock) if (gate)                r2_C     <= `tFCQ  r1_C;
always @(posedge fast_clock) if (gate)                r2_D     <= `tFCQ  add2dgstA  ?  r1_E  :  r1_D;
always @(posedge fast_clock) if (gate)                r2_A     <= `tFCQ  add2dgstA  ?  b1_DA :  r1_A;

assign {b2_ADD[31:0], `RR(b2_ARX,9)} = A2_ARX32 ( r2_A[31:0], r2_B[31:0], `RR(r2_C,9) );

// Pipeline Stage 3 -------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (gate && add2dgstB)   r3_ADD   <= `tFCQ  b2_ADD;
always @(posedge fast_clock) if (gate)                r3_C     <= `tFCQ  add2dgstB   ?  r2_C  :  b2_ARX;
always @(posedge fast_clock) if (gate)                r3_D     <= `tFCQ  r2_D;
always @(posedge fast_clock) if (gate)                r3_A     <= `tFCQ  r2_A;
always @(posedge fast_clock) if (gate)                r3_B     <= `tFCQ  add2dgstB   ?  b2_DB :  r2_B;

assign {b3_ADD[31:0], `RR(b3_ARX,13)} = A2_ARX32 ( r3_C[31:0], r3_B[31:0], `RR(r3_D,13) );

// Pipeline Stage 4 -------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (gate && add2dgstC)   r4_ADD   <= `tFCQ  b3_ADD;
always @(posedge fast_clock) if (gate)                r4_D     <= `tFCQ  add2dgstC   ?  r3_D  :  b3_ARX;
always @(posedge fast_clock) if (gate)                r4_A     <= `tFCQ  r3_A;
always @(posedge fast_clock) if (gate)                r4_B     <= `tFCQ  r3_B;
always @(posedge fast_clock) if (gate)                r4_C     <= `tFCQ  add2dgstC   ?  b3_DC :  r3_C;

assign {b4_ADD[31:0], `RR(b4_ARX,18)} = A2_ARX32 ( r4_D[31:0], r4_C[31:0], `RR(r4_A,18) );

// Pipeline Stage 5 -------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (gate && add2dgstD2)  r5_ADD   <= `tFCQ  b4_ADD;
always @(posedge fast_clock) if (gate && P0P1P2)      r5_B     <= `tFCQ  r4_B;
always @(posedge fast_clock) if (gate && P0P1  )      r5_C     <= `tFCQ  r4_C;
always @(posedge fast_clock) if (gate && P00   )      r5_D     <= `tFCQ  r4_D;

// Pipeline Stage 6 -------------------------------------------------------------------------------------------------------------
always @(posedge fast_clock) if (gate && P01   )      r6_C     <= `tFCQ  r5_C;

// Support functions ============================================================================================================
// Combined adder, rotator and XOR as a single function with the XOR buried in the half-sum path of the adder.  The rotate is
// actually in the function call.  We need a rotate left, so to be sneaky we rotate right on the way in and rotates the result
// out, to get the same effect!!

function [63:00]  A2_ARX32;   // {  sum[31:00],arx[31:00] };
   input [31:00]  a,b,c;
   reg   [27:00]  g0;
   reg   [27:01]  p0;
   reg   [13:00]  g1;
   reg   [13:01]  p1;
   reg   [06:00]  g2;
   reg   [06:01]  p2;
   reg   [06:00]  g3;
   reg   [06:02]  p3;
   reg   [06:00]  g4;
   reg   [06:04]  p4;
   reg   [06:00]  c5;

begin
   g0[27:0] = ~(a[27:0] & b[27:0]);
   p0[27:1] = ~(a[27:1] | b[27:1]);

   g1[13]   = ~g0[27]  | (~p0[27] & ~g0[26]);
   p1[13]   =             ~p0[27] & ~p0[26];
   g1[12]   = ~g0[25]  | (~p0[25] & ~g0[24]);
   p1[12]   =             ~p0[25] & ~p0[24];
   g1[11]   = ~g0[23]  | (~p0[23] & ~g0[22]);
   p1[11]   =             ~p0[23] & ~p0[22];
   g1[10]   = ~g0[21]  | (~p0[21] & ~g0[20]);
   p1[10]   =             ~p0[21] & ~p0[20];
   g1[09]   = ~g0[19]  | (~p0[19] & ~g0[18]);
   p1[09]   =             ~p0[19] & ~p0[18];
   g1[08]   = ~g0[17]  | (~p0[17] & ~g0[16]);
   p1[08]   =             ~p0[17] & ~p0[16];
   g1[07]   = ~g0[15]  | (~p0[15] & ~g0[14]);
   p1[07]   =             ~p0[15] & ~p0[14];
   g1[06]   = ~g0[13]  | (~p0[13] & ~g0[12]);
   p1[06]   =             ~p0[13] & ~p0[12];
   g1[05]   = ~g0[11]  | (~p0[11] & ~g0[10]);
   p1[05]   =             ~p0[11] & ~p0[10];
   g1[04]   = ~g0[09]  | (~p0[09] & ~g0[08]);
   p1[04]   =             ~p0[09] & ~p0[08];
   g1[03]   = ~g0[07]  | (~p0[07] & ~g0[06]);
   p1[03]   =             ~p0[07] & ~p0[06];
   g1[02]   = ~g0[05]  | (~p0[05] & ~g0[04]);
   p1[02]   =             ~p0[05] & ~p0[04];
   g1[01]   = ~g0[03]  | (~p0[03] & ~g0[02]);
   p1[01]   =             ~p0[03] & ~p0[02];
   g1[00]   = ~g0[01]  | (~p0[01] & ~g0[00]);

   g2[06]   = ~(g1[13] |   p1[13] & g1[12]);
   p2[06]   =            ~(p1[13] & p1[12]);
   g2[05]   = ~(g1[11] |   p1[11] & g1[10]);
   p2[05]   =            ~(p1[11] & p1[10]);
   g2[04]   = ~(g1[09] |   p1[09] & g1[08]);
   p2[04]   =            ~(p1[09] & p1[08]);
   g2[03]   = ~(g1[07] |   p1[07] & g1[06]);
   p2[03]   =            ~(p1[07] & p1[06]);
   g2[02]   = ~(g1[05] |   p1[05] & g1[04]);
   p2[02]   =            ~(p1[05] & p1[04]);
   g2[01]   = ~(g1[03] |   p1[03] & g1[02]);
   p2[01]   =            ~(p1[03] & p1[02]);
   g2[00]   = ~(g1[01] |   p1[01] & g1[00]);

   g3[06]   = ~g2[06]  | (~p2[06] & ~g2[05]);
   p3[06]   =             ~p2[06] & ~p2[05];
   g3[05]   = ~g2[05]  | (~p2[05] & ~g2[04]);
   p3[05]   =             ~p2[05] & ~p2[04];
   g3[04]   = ~g2[04]  | (~p2[04] & ~g2[03]);
   p3[04]   =             ~p2[04] & ~p2[03];
   g3[03]   = ~g2[03]  | (~p2[03] & ~g2[02]);
   p3[03]   =             ~p2[03] & ~p2[02];
   g3[02]   = ~g2[02]  | (~p2[02] & ~g2[01]);
   p3[02]   =             ~p2[02] & ~p2[01];
   g3[01]   = ~g2[01]  |  ~p2[01] & ~g2[00];
   g3[00]   = ~g2[00];

   g4[06]   = ~(g3[06] |   p3[06] &  g3[04]);
   p4[06]   =            ~(p3[06] &  p3[04]);
   g4[05]   = ~(g3[05] |   p3[05] &  g3[03]);
   p4[05]   =            ~(p3[05] &  p3[03]);
   g4[04]   = ~(g3[04] |   p3[04] &  g3[02]);
   p4[04]   =            ~(p3[04] &  p3[02]);
   g4[03]   = ~(g3[03] |   p3[03] &  g3[01]);
   g4[02]   = ~(g3[02] |   p3[02] &  g3[00]);
   g4[01]   =  ~g3[01];
   g4[00]   =  ~g3[00];

   c5[06]   = ~g4[06]  |  ~p4[06] & ~g4[02];
   c5[05]   = ~g4[05]  |  ~p4[05] & ~g4[01];
   c5[04]   = ~g4[04]  |  ~p4[04] & ~g4[00];
   c5[03]   = ~g4[03];
   c5[02]   = ~g4[02];
   c5[01]   = ~g4[01];
   c5[00]   = ~g4[00];

  {A2_ARX32[63:60], A2_ARX32[31:28]} = A2_arx4( a[31:28], b[31:28], c[31:28], c5[06]);
  {A2_ARX32[59:56], A2_ARX32[27:24]} = A2_arx4( a[27:24], b[27:24], c[27:24], c5[05]);
  {A2_ARX32[55:52], A2_ARX32[23:20]} = A2_arx4( a[23:20], b[23:20], c[23:20], c5[04]);
  {A2_ARX32[51:48], A2_ARX32[19:16]} = A2_arx4( a[19:16], b[19:16], c[19:16], c5[03]);
  {A2_ARX32[47:44], A2_ARX32[15:12]} = A2_arx4( a[15:12], b[15:12], c[15:12], c5[02]);
  {A2_ARX32[43:40], A2_ARX32[11:08]} = A2_arx4( a[11:08], b[11:08], c[11:08], c5[01]);
  {A2_ARX32[39:36], A2_ARX32[07:04]} = A2_arx4( a[07:04], b[07:04], c[07:04], c5[00]);
  {A2_ARX32[35:32], A2_ARX32[03:00]} = A2_arx4( a[03:00], b[03:00], c[03:00],  1'b0 );

  end
endfunction

function  [ 7:0] A2_arx4;  // { s[3:0],x[3:0]  }
   input  [ 3:0] a,b,c;
   input         ci;
   reg    [3:1]  cz,co;
   reg    [3:0]  h, k, sz, so, xz, xo, s, x;
begin
   h        =   a ^ b;
   k        =   h ^ c;
   cz[1]    =   a[0] & b[0];
   cz[2]    =  ~h[1] ? b[1] : cz[1];
   cz[3]    =  ~h[2] ? b[2] : cz[2];
   co[1]    =   a[0] | b[0];
   co[2]    =  ~h[1] ? b[1] : co[1];
   co[3]    =  ~h[2] ? b[2] : co[2];

   sz       = {(h[3:1] ^ cz[3:1]),  h[0]};
   so       = {(h[3:1] ^ co[3:1]), ~h[0]};

   xz       = {(k[3:1] ^ cz[3:1]),  k[0]};
   xo       = {(k[3:1] ^ co[3:1]), ~k[0]};

   s        = ci ? so : sz;
   x        = ci ? xo : xz;
   A2_arx4  = {s,x};
end
endfunction

endmodule

// Support modules ==============================================================================================================

module   regX4 (
   output reg  [31:0]   q,
   input  wire [ 3:0]   e3,
   input  wire [ 3:0]   e2,
   input  wire [ 3:0]   e1,
   input  wire [ 3:0]   e0,
   input  wire [31:0]   d3,
   input  wire [31:0]   d2,
   input  wire [31:0]   d1,
   input  wire [31:0]   d0,
   input  wire          clock
   );
always @(posedge clock) if (e0[3] | e1[3] | e2[3] | e3[3]) q[31:24] <= `tFCQ {8{e0[3]}} & d0[31:24] | {8{e1[3]}} & d1[31:24] | {8{e2[3]}} & d2[31:24] | {8{e3[3]}} & d3[31:24];
always @(posedge clock) if (e0[2] | e1[2] | e2[2] | e3[2]) q[23:16] <= `tFCQ {8{e0[2]}} & d0[23:16] | {8{e1[2]}} & d1[23:16] | {8{e2[2]}} & d2[23:16] | {8{e3[2]}} & d3[23:16];
always @(posedge clock) if (e0[1] | e1[1] | e2[1] | e3[1]) q[15:08] <= `tFCQ {8{e0[1]}} & d0[15:08] | {8{e1[1]}} & d1[15:08] | {8{e2[1]}} & d2[15:08] | {8{e3[1]}} & d3[15:08];
always @(posedge clock) if (e0[0] | e1[0] | e2[0] | e3[0]) q[07:00] <= `tFCQ {8{e0[0]}} & d0[07:00] | {8{e1[0]}} & d1[07:00] | {8{e2[0]}} & d2[07:00] | {8{e3[0]}} & d3[07:00];
endmodule

module   regX5 (
   output reg  [31:0]   q,
   input  wire [ 3:0]   e4,
   input  wire [ 3:0]   e3,
   input  wire [ 3:0]   e2,
   input  wire [ 3:0]   e1,
   input  wire [ 3:0]   e0,
   input  wire [31:0]   d4,
   input  wire [31:0]   d3,
   input  wire [31:0]   d2,
   input  wire [31:0]   d1,
   input  wire [31:0]   d0,
   input  wire          clock
   );
always @(posedge clock)   if (e4[3])  q[31:24] <= `tFCQ d4[31:24];
always @(posedge clock)   if (e3[3])  q[31:24] <= `tFCQ d3[31:24];
always @(posedge clock)   if (e2[3])  q[31:24] <= `tFCQ d2[31:24];
always @(posedge clock)   if (e1[3])  q[31:24] <= `tFCQ d1[31:24];
always @(posedge clock)   if (e0[3])  q[31:24] <= `tFCQ d0[31:24];

always @(posedge clock)   if (e4[2])  q[23:16] <= `tFCQ d4[23:16];
always @(posedge clock)   if (e3[2])  q[23:16] <= `tFCQ d3[23:16];
always @(posedge clock)   if (e2[2])  q[23:16] <= `tFCQ d2[23:16];
always @(posedge clock)   if (e1[2])  q[23:16] <= `tFCQ d1[23:16];
always @(posedge clock)   if (e0[2])  q[23:16] <= `tFCQ d0[23:16];

always @(posedge clock)   if (e4[1])  q[15:08] <= `tFCQ d4[15:08];
always @(posedge clock)   if (e3[1])  q[15:08] <= `tFCQ d3[15:08];
always @(posedge clock)   if (e2[1])  q[15:08] <= `tFCQ d2[15:08];
always @(posedge clock)   if (e1[1])  q[15:08] <= `tFCQ d1[15:08];
always @(posedge clock)   if (e0[1])  q[15:08] <= `tFCQ d0[15:08];

always @(posedge clock)   if (e4[0])  q[07:00] <= `tFCQ d4[07:00];
always @(posedge clock)   if (e3[0])  q[07:00] <= `tFCQ d3[07:00];
always @(posedge clock)   if (e2[0])  q[07:00] <= `tFCQ d2[07:00];
always @(posedge clock)   if (e1[0])  q[07:00] <= `tFCQ d1[07:00];
always @(posedge clock)   if (e0[0])  q[07:00] <= `tFCQ d0[07:00];

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

`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/NEWPORT_IO_addresses.tbh"
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
   .WIDTH(  256)              // CoP WIDTH =  256
// .TYPE (2'b10)              // Base Address
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
      #10 fast_clock = ~fast_clock;
      end
   end

initial begin
   slow_clock = 1'b1;
   forever begin
      #40 slow_clock = ~slow_clock;
      end
   end

always @(posedge fast_clock)
   if (reset)     cycles = 0;
   else if (gate) cycles = cycles +1;

// Test Procedure
initial begin
   $display(" Register Based Execution Pipeline: NO CONTROL FAN-OUT");
// nmosi    = 260'h0;
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
