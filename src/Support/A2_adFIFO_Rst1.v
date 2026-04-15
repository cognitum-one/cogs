/********************************************************************************************************************************
**                                                                                                                             **
**    Copyright © 2022 Advanced Architectures                                                                                  **
**                                                                                                                             **
**    All rights reserved                                                                                                      **
**    Confidential Information                                                                                                 **
**    Limited Distribution to Authorized Persons Only                                                                          **
**    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.                                        **
**                                                                                                                             **
**    Project Name         : Asynchronous FIFO                                                                                 **
**    Description          : Asynchronous FIFO with dipstick                                                                   **
**                                                                                                                             **
*********************************************************************************************************************************
**    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE      **
**    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS **
**    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED **
**    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED **
**    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)      **
**    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.                   **
*********************************************************************************************************************************
      Notes for Use and Synthesis

      This FIFO does NOT synchronize pointers or try to use gray counters.  Instead it counts reads and writes separately
      and transfers these counts to the opposite side by closed loop synchronizers to update dipsticks on either side.  This
      allows fast reads and writes but pays for it in latency and thus in overall FIFO depth.  It also benefits from the
      ability to handle a wide range of relative clock frequencies. To minimise the latency the fastest available clock on
      both the read and write side should be used and wAFF and rAFF then control the data rate.

      FALSE PATHS:
                        Wr_transfer to Rd_dipstick
                        Rd_transfer to Wr_dipstick
                        Wr_req to u_wr_req.d       // Synchronizer inputs
                        Wr_ack to u_wr_ack.d
                        Rd_req to u_rd_req.d
                        Rd_ack to u_rd_ack.d
                        resetn to u_rst_wr.d, u_rst_wr.resetn, u_rst_rd.d and u_rst_rd.resetn

********************************************************************************************************************************/

module  A2_adFIFO_Rst1  #(
   parameter   WIDTH    =  32,
   parameter   LG2DEPTH =   4             // DEPTH always a power of 2
   ) (
   // Write Side
   output wire    [LG2DEPTH:0]   AFFwc,   // Write word count
   output wire                   AFFir,   // Write Input Ready (not full)

   input  wire    [WIDTH-1 :0]   iAFF,    // Data In
   input  wire                   wAFF,    // Data Write        (push)
   input  wire                   wr_clk,  // Write Clock
   // Read Side
   output wire    [WIDTH-1 :0]   AFFo,    // Data Out
   output wire    [LG2DEPTH:0]   AFFrc,   // Read word count
   output wire                   AFFor,   // Read Output Ready (not empty)

   input  wire                   rAFF,    // Data Read         (pop)
   input  wire                   rd_clk,  // Read Clock

   input  wire                   reset
   );

localparam  DEPTH = 1 << LG2DEPTH;

// Locals =======================================================================================================================

wire [LG2DEPTH:0]    rd_wrxfr, wr_rdxfr;
wire                 rd_wrinc, wr_rddec;
wire                 rd_irdy,  wr_irdy;
wire                 rd_init,  wr_init;   // MAV (26Feb25) was rd_initn, wr_initn;
wire                 rd_ldxf,  wr_ldxf;

// Resets =======================================================================================================================
//
// Reset from either side must ensure that all registers and synchronizers are correctly reset to a quiescent state with the
// FIFO empty.  Resets also force input-ready and output-ready outputs to signal that the FIFO is not ready.
// Treat reset as asynchronous to both sides and ensure that the inactive edge is synchronized to both rd_clk and wr_clk
// active edges.
A2_sync_Rst1  u_rst_wr (.q(wr_init),.reset(reset),.d(reset),.clock(wr_clk));  // MAV (26Feb25) was A2_syncRst1  and .q(wr_initn),.resetn(resetn),.d(resetn),
A2_sync_Rst1  u_rst_rd (.q(rd_init),.reset(reset),.d(reset),.clock(rd_clk));  // MAV (26Feb25) was A2_syncRst1  and .q(rd_initn),.resetn(resetn),.d(resetn),

// Write Side ===================================================================================================================
// Write side counts incoming pushes and transfers that count to the read side.  This is done by saving the current count of
// pushes, clearing the counter and then sending a request to the read side through synchronisers.  When the request arrives the
// staticised count of pushes is added to the read side dipstick.  The read side immediately sends an acknowledge back to the
// write side to complete the transaction.   As this takes some time based on the clock cycle times of the read and write clocks
// the write counter (Wr_count) may have multiple writes to send over this interface.

// Write Clock Registers
reg  [LG2DEPTH  :0]   Wr_transfer;                    /* false path */
reg  [LG2DEPTH  :0]   Wr_dipstick;
reg  [LG2DEPTH  :0]   Wr_count;
reg  [LG2DEPTH-1:0]   Wr_ptr;

wire  wr_full  =  Wr_dipstick == DEPTH[LG2DEPTH:0];

wire  wr_push  =  wAFF & ~wr_full;

wire  wr_load  = (Wr_count != {LG2DEPTH+1{1'b0}});

wire  wr_send  =  wr_load  &  wr_irdy;

wire  wr_RAM   =  wr_push;

always @(posedge wr_clk or posedge wr_init) // MAV (26Feb25) added  or posedge wr_init
   if (wr_init) begin                       // MAV (26Feb25) was !wr_initn
      Wr_ptr      <= #1 0;
      Wr_count    <= #1 0;
      Wr_dipstick <= #1 0;
      end
   else begin
      if       ( wr_RAM               ) Wr_ptr       <= #1  Wr_ptr + 1;

      if       (!wr_push &&  wr_send  ) Wr_count     <= #1  0;
      else if  ( wr_push && !wr_send  ) Wr_count     <= #1  Wr_count + 1;
      else if  ( wr_push &&  wr_send  ) Wr_count     <= #1  1;

      if       (!wr_push &&  wr_rddec ) Wr_dipstick  <= #1  Wr_dipstick - wr_rdxfr;            /* false path */
      else if  ( wr_push && !wr_rddec ) Wr_dipstick  <= #1  Wr_dipstick + 1;
      else if  ( wr_push &&  wr_rddec ) Wr_dipstick  <= #1  Wr_dipstick - wr_rdxfr + 1;        /* false path */

      if       ( wr_ldxf              ) Wr_transfer  <= #1  Wr_count;
      end

// Cross clock domains ==========================================================================================================
// Closed loop synchronizers are used.  The write signal is turned into a level transition and passed through the synchronizer.
// It is then returned as an acknowledge back through synchronizers to the write clock domain.  It is also turned back into a
// pulse for use in the read clock domain.

assign   rd_wrxfr = Wr_transfer;                      /* false path */

A2_CDC_AsyncRst iWRT (    // MAV (4Mar25) added _AsyncRst
    .CDCor  (rd_wrinc),   // Output ready
    .CDCir  (wr_irdy),    // Input ready
    .CDCld  (wr_ldxf),    // Load transfer
   .wCDC    (wr_load),    // Write input data
   .rCDC    (rd_wrinc),   // Read output data
   .ireset  (wr_init),   // reset synchronized to iclock  // MAV (26Feb25) was .iresetn
   .iclock  (wr_clk),     // input  clock
   .oreset  (reset),     // reset synchronized to oclock  // MAV (26Feb25) was .oresetn
   .oclock  (rd_clk)      // output clock
   );

// Read Side ====================================================================================================================
// Read side is a little different so that a pop command always removes the current data (the one read) and fetches the next.
// This creates a fallthrough of one just like an A2_sFIFO.  To do this requires that when a first data item arrives it primes
// the pipe and reads the data from memory and increments the rd_pointer and the rd_dipstick before the first pop.
// Also when the FIFO is going empty the read pointer is NOT incremented.  This resets the state to the same as an empty FIFO.

// Read Clock Registers
reg [LG2DEPTH  :0]   Rd_transfer;                     /* false path */
reg [LG2DEPTH  :0]   Rd_dipstick;
reg [LG2DEPTH  :0]   Rd_count;
reg [LG2DEPTH-1:0]   Rd_ptr;

wire  rd_empty =  Rd_dipstick ==  {LG2DEPTH+1{1'b0}};
wire  rd_last  =  Rd_dipstick == {{LG2DEPTH{1'b0}},1'b1};

wire  rd_pop   =  rAFF & ~rd_empty;

wire  rd_load  = (Rd_count != {LG2DEPTH+1{1'b0}});
wire  rd_send  =  rd_load  &  rd_irdy;

wire  rd_ptrinc=  rd_empty &  rd_wrinc                // Prime
               |  rd_pop   & ~rd_last                 // Advance until only one left
               |  rd_pop   &  rd_last  &  rd_wrinc;   // Advance only if last & new transfer

wire  rd_RAM   =  rd_ptrinc;

always @(posedge rd_clk or posedge rd_init)  // MAV (26Feb25) added  or posedge wr_init
   if (rd_init) begin                        // MAV (26Feb25) was !rd_initn
      Rd_ptr      <= #1 0;
      Rd_count    <= #1 0;
      Rd_dipstick <= #1 0;
      end
   else begin
      if       ( rd_ptrinc            ) Rd_ptr       <= #1  Rd_ptr + 1;

      if       (!rd_pop  &&  rd_send  ) Rd_count     <= #1  0;
      else if  ( rd_pop  && !rd_send  ) Rd_count     <= #1  Rd_count + 1;
      else if  ( rd_pop  &&  rd_send  ) Rd_count     <= #1  1;

      if       (!rd_pop  &&  rd_wrinc ) Rd_dipstick  <= #1  Rd_dipstick + rd_wrxfr;       /* false path */
      else if  ( rd_pop  && !rd_wrinc ) Rd_dipstick  <= #1  Rd_dipstick - 1;
      else if  ( rd_pop  &&  rd_wrinc ) Rd_dipstick  <= #1  Rd_dipstick + rd_wrxfr -1;    /* false path */

      if       ( rd_ldxf              ) Rd_transfer  <= #1  Rd_count;
      end

// Cross clock domains ==========================================================================================================
// Closed loop synchronizers are used.  The read signal is turned into a level transition and passed through the synchronizer.
// It is then returned as an acknowledge back through synchronizers to the read clock domain.  It is also turned back into a
// pulse for use in the write clock domain.

assign   wr_rdxfr =  Rd_transfer;                     /* false path */

A2_CDC_AsyncRst iRDT ( // MAV (4Mar25) added _AsyncRst
    .CDCor  (wr_rddec),   // Output ready
    .CDCir  (rd_irdy),    // Input ready
    .CDCld  (rd_ldxf),    // Load transfer
   .wCDC    (rd_load),    // Write input data
   .rCDC    (wr_rddec),   // Read output data
   .ireset  (rd_init),   // reset synchronized to iclock   // MAV (26Feb25) was .iresetn
   .iclock  (rd_clk),     // input  clock
   .oreset  (reset),     // reset synchronized to oclock   // MAV (26Feb25) was .oresetn
   .oclock  (wr_clk)     // output clock
   );

// Two Port Synchronous SRAM ===================================================================================================
// FIFO memory
reg [WIDTH-1:0]   RAM   [0:DEPTH-1];
reg [WIDTH-1:0]   RAMo;

// Write side
always @(posedge wr_clk) begin
   if (wr_RAM)
      RAM[Wr_ptr]  <= #1  iAFF;
   end

// Read  side
always @(posedge rd_clk) begin
   if (rd_RAM)
      RAMo <= #1  RAM[Rd_ptr];
   end

// Outputs =====================================================================================================================

// Write side
assign   AFFir   =  Wr_dipstick != DEPTH;
assign   AFFwc   =  Wr_dipstick;

// Read side
assign   AFFor   =  Rd_dipstick > 7'd0;
assign   AFFrc   =  Rd_dipstick;
assign   AFFo    =  RAMo;

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Set a command line `define to "verify_A2_adFIFO" to do a standalone module test on this file
`ifdef   verify_A2_adFIFO
`ifdef   synthesis
`else

// synopsys  translate_off
// synthesis translate_off

`timescale 1ns/1ps
`define  tick 10

module t;

parameter   WIDTH       =  32;
parameter   LG2DEPTH    =   6;
parameter   DEPTH       =  1 << LG2DEPTH;
parameter   WTICK       =  17;
parameter   RTICK       =  13;

integer  i,j;

reg     [WIDTH-1    :0]    rdCS, wrCS;

wire    [WIDTH-1    :0]    AFFo;   // Data Out
wire    [LG2DEPTH   :0]    AFFwc;  // Write word count
wire    [LG2DEPTH   :0]    AFFrc;  // Read word count
wire                       AFFir;  // Write Input Ready (not full)
wire                       AFFor;  // Read Output Ready (not empty)

reg     [WIDTH-1    :0]    iAFF;    // Data In
reg                        wAFF;    // Data Write        (push)
reg                        rAFF;    // Data Read         (pop)

reg                        wr_clk;  // Write Clock
reg                        rd_clk;  // Read Clock
reg                        resetn;

A2_adFIFO  #(WIDTH,LG2DEPTH) mut (
   .AFFo    (AFFo),    // Data Out
   .AFFwc   (AFFwc),   // Write word count
   .AFFrc   (AFFrc),   // Read word count
   .AFFor   (AFFor),   // Read Output Ready (not empty)
   .AFFir   (AFFir),   // Write Input Ready (not full)

   .iAFF    (iAFF),    // Data In
   .wAFF    (wAFF),    // Data Write        (push)
   .rAFF    (rAFF),    // Data Read         (pop)

   .wr_clk  (wr_clk),  // Write Clock
   .rd_clk  (rd_clk),  // Read Clock
   .resetn  (resetn)
   );

initial begin
   wr_clk     = 0;
   forever begin
      #(WTICK/2); wr_clk = 0;
      #(WTICK/2); wr_clk = 1;
      end
   end

initial begin
   rd_clk     = 0;
   forever begin
      #(RTICK/2); rd_clk = 0;
      #(RTICK/2); rd_clk = 1;
      end
   end

always @(posedge wr_clk)
   if       (!mut.wr_initn) wrCS <= #1 0;
   else if  (AFFir && wAFF) wrCS <= #1 wrCS + iAFF;

always @(posedge rd_clk)
   if       (!mut.rd_initn) rdCS <= #1 0;
   else if  (AFFor && rAFF) rdCS <= #1 rdCS + AFFo;

initial begin
   resetn   =  1;
   iAFF     =  0;
   wAFF     =  0;
   rAFF     =  0;
   #100;
   resetn   = 0;
   #100;
   resetn   =  1;
   #100;
   fork
      // Write Side ------------------------------------
      begin : wr
      repeat (10) @(posedge wr_clk); #1;
      iAFF = 32'h55555555;
      wAFF = 1;
      repeat ( 1) @(posedge wr_clk); #1;
      iAFF = 32'h66666666;
      repeat ( 1) @(posedge wr_clk); #1;
      iAFF = 32'h77777777;
      repeat ( 1) @(posedge wr_clk); #1;
      iAFF = 32'h88888888;
      repeat ( 1) @(posedge wr_clk); #1;
      iAFF = 32'h99999999;
      repeat ( 1) @(posedge wr_clk); #1;
      iAFF = 32'h00000000;
      repeat ( 1) @(posedge wr_clk); #1;
      iAFF = 32'h11111111;
      repeat ( 1) @(posedge wr_clk); #1;
      iAFF = 32'h22222222;
      repeat ( 1) @(posedge wr_clk); #1;
      iAFF = 32'h33333333;
      repeat ( 1) @(posedge wr_clk); #1;
      wAFF = 0;
      repeat (20) @(posedge wr_clk); #1;
      iAFF = 32'hab08723e;
      wAFF = 1;
      for (i=0; i<(2*DEPTH); i=i+1) begin
         iAFF = {iAFF[30:0], iAFF[31]^iAFF[21]^iAFF[1]^iAFF[0]};
         while (!AFFir)
            repeat ( 1) @(posedge rd_clk); #1;
         repeat ( 1) @(posedge wr_clk); #1;
         end
      wAFF = 0;
      repeat (1000) @(posedge wr_clk); #1;
      iAFF = 32'h600df1d0;
      wAFF = 1;
      repeat ( 1) @(posedge wr_clk); #1;
      wAFF = 0;
      repeat (100) @(posedge wr_clk); #1;
      end
      // Read side -------------------------------------
      begin : rd
      repeat (10) @(posedge rd_clk); #1;
      while (AFFrc < 6)
         repeat ( 1) @(posedge rd_clk); #1;
      rAFF = 1;
      repeat (15)  @(posedge rd_clk); #1;
      rAFF = 0;
      repeat (100) @(posedge rd_clk); #1;
      rAFF = 1;
      for (j=0; j<(2*DEPTH); j=j+1) begin
         while (!AFFor)
            repeat ( 1) @(posedge rd_clk); #1;
         repeat ( 1) @(posedge rd_clk); #1;
         end
      rAFF = 0;
      repeat (100) @(posedge rd_clk); #1;
      rAFF = 1;
      while (!AFFor)
         repeat ( 1) @(posedge rd_clk); #1;
      repeat ( 1) @(posedge rd_clk); #1;


      end
      //------------------------------------------------
   join
   if (wrCS == rdCS) $display(" All tests run with verified checksums\n\n");
   else              $display(" All tests run checksums incorrect!!! \n\n");
   $stop();
   $finish();
   end


endmodule

// synthesis translate_on
// synopsys  translate_on
`endif
`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////



