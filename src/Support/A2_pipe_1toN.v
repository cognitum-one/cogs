//===============================================================================================================================
//
//    Copyright © 1996..2012 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name   : Pipeline 1 to N expansion buffer
//
//    Author         : RTT
//    Creation Date  : 5/26/2004
//
//    Description    : Stallable pipestage
//                     This is a pipestage module that when linked in a chain creates a FIFO that may stall in any stage
//                     This can be used as a FIFO or as a stallable pipeline with logic between each stage.
//                     This module expands an incoming width by a factor of N (i.e. N=4: 8-bits in -> 32-bits out)
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================

//`include "A2_defines.vh" //Joel fix include path 2_26_25
`ifdef synthesis //JLPL
    `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
    `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_defines.vh" //Joel fix include path 2_26_25
`else
    `include "A2_project_settings.vh"
    `include "A2_defines.vh" //Joel fix include path 2_26_25
`endif

module A2_pipe_1toN  #(
   parameter   WIDTH    =  8,                      // Width of INPUT
   parameter   FACTOR   =  8,                      // Expansion Factor - Number of times output is wider than input
   parameter   ENDIAN   =  0                       // 0: little endian  1: Big endian
   )(
   // Output Interface
   output wire                      por,           // Output Ready:  Like a FIFO not empty
   output wire [FACTOR*WIDTH-1:0]   pdo,           // Output data
   input  wire                      prd,           // Read:  Like a FIFO pop  (active)
   // Input Interface
   output wire                      pir,           // Input Ready:   Like a FIFO not full
   input  wire        [WIDTH-1:0]   pdi,           // Input data
   input  wire                      pwr,           // Write: Like a FIFO push (active)
   // Global
   input  wire                      clock,
   input  wire                      flush,         // Instantly flush the pipe and load the current data to stage 0
   input  wire                      reset
   );

localparam  LX = `LG2(FACTOR);
localparam  L  = 1'b 0;
localparam  H  = 1'b 1;
localparam  x  = 1'b x;

// Flip-flops ------------------------------------------------------------------------------------------------------------------

reg  [FACTOR*WIDTH-1:0] r0_pipe  = 0;            // Pipeline register -- big shift register
reg         [WIDTH-1:0] r0_skid  = 0;            // Slave register for two deep
reg              [LX:0] r0_count = 0;            // Count of available words for output

// Local Nets ------------------------------------------------------------------------------------------------------------------
reg                     s0_ld_skid,  s0_ld_pipe;
reg                     s0_inc_by1,  s0_dec_byN, s0_dec_byNm1;
reg                     s0_sel;

wire             [LX:0] s0_count;
wire             [LX:0] n0_count;

// Pipeline Registers ----------------------------------------------------------------------------------------------------------

always @(posedge clock)
   if ( s0_ld_skid ) r0_skid  <= `tCQ  pdi[WIDTH-1:0];

generate begin
   if (ENDIAN == 1) begin : B
      always @(posedge clock) begin            //Joel 3_1_25 - Replace header referenced `ACTIVE_EDGE with posedge
         if ( s0_ld_pipe )
            r0_pipe[FACTOR*WIDTH-1:0] <= `tCQ {r0_pipe[(FACTOR-1)*WIDTH-1:0],(s0_sel ? pdi[WIDTH-1:0] : r0_skid[WIDTH-1:0])};
         end
      end
   else begin : LL
      always @(posedge clock) begin  //Joel 3_1_25 - Replace header referenced `ACTIVE_EDGE with posedge
         if ( s0_ld_pipe )
            r0_pipe[FACTOR*WIDTH-1:0] <= `tCQ {(s0_sel ? pdi[WIDTH-1:0] : r0_skid[WIDTH-1:0]),r0_pipe[FACTOR*WIDTH-1:WIDTH]};
         end
      end
   end endgenerate

// State machine ---------------------------------------------------------------------------------------------------------------

always @* begin
   if (s0_count[LX:0] <  FACTOR[LX:0])
      casez ({pwr, prd})
`define  TT  { s0_ld_skid, s0_ld_pipe, s0_sel, s0_inc_by1, s0_dec_byN, s0_dec_byNm1}
//                       \     |      /  ______/          /           /
//                        \    |     /  / _______________/           /
//                         \   |    /  / / _________________________/
//                          \  |   /  / / /
         2'b 1_? :   `TT = { L,H, H, H,L,L };
         default :   `TT = { L,L, x, L,L,L };
         endcase
   else if (s0_count[LX:0] == FACTOR[LX:0])
      casez ({pwr, prd})
         2'b 0_1 :   `TT = { L,L, x, L,H,L };
         2'b 1_0 :   `TT = { H,L, x, H,L,L };
         2'b 1_1 :   `TT = { L,H, H, L,L,H };
         default :   `TT = { L,L, x, L,L,L };
         endcase
   else if (s0_count[LX:0] >  FACTOR[LX:0])
      casez ({pwr, prd})
         2'b 0_1 :   `TT = { L,H, L, L,H,L };
         2'b 1_0 :   `TT = { L,L, x, L,L,L };
         2'b 1_1 :   `TT = { H,H, L, L,H,L };
         default :   `TT = { L,L, x, L,L,L };
         endcase
   else              `TT = { L,L, x, L,L,L };
   end

assign   s0_count[LX:0]  =   flush         ?  {LX+1{1'b 0}}  : r0_count [LX:0];
assign   n0_count[LX:0]  = ( s0_inc_by1    ?  s0_count[LX:0] + 1'b 1
                           : s0_dec_byN    ?  s0_count[LX:0] - FACTOR[LX:0]
                           : s0_dec_byNm1  ?  s0_count[LX:0] - FACTOR[LX:0] + 1'b 1
                           :                  s0_count[LX:0]);

always @(posedge clock)
   if (reset)  r0_count[LX:0] <= `tCQ  {LX+1{1'b 0}};
   else        r0_count[LX:0] <= `tCQ  n0_count[LX:0];

// Outputs ----------------------------------------------------------------------------------------------------------------------

assign   pdo[FACTOR*WIDTH-1:0]   =  r0_pipe[FACTOR*WIDTH-1:0];
assign   por                     =  s0_count > (FACTOR-1);      // Only output when there are N items available
assign   pir                     =  s0_count < (FACTOR+1);      // There's room until all pipe and skid registers are full!

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Set a command line `define to "verify_A2_pipe1toN" to do a standalone module test on this file
`ifdef   verify_A2_pipe1toN

module t;

parameter   WIDTH    = 32;
parameter   FACTOR   =  8;
parameter   TICK     = 10;

wire  [FACTOR*WIDTH-1:0]   pdo;
wire                       por;       // Like a FIFO not empty (Output Ready)
wire                       pir;       // Like a FIFO not full (Input Ready)

reg   [WIDTH-1:0]          pdi;
reg                        pwr;       // Like a FIFO pwr (active)
reg                        prd;        // Like a FIFO prd  (active)
reg                        clock;
reg                        flush;
reg                        reset;

A2_pipe1toN #(
   .WIDTH   (WIDTH),
   .FACTOR  (FACTOR)
   ) mut (
   .pdi     (pdi),
   .pwr     (pwr),       // Like a FIFO pwr (active)
   .pir     (pir),       // Like a FIFO not full (Input Ready)

   .pdo     (pdo),
   .por     (por),       // Like a FIFO not empty (Output Ready)
   .prd     (prd),        // Like a FIFO prd  (active)

   .clock   (clock),
   .flush   (flush),
   .reset   (reset)
   );

initial begin
   #1;
   clock     = 1;
   forever begin
      #(TICK/2) clock = 0;
      #(TICK/2) clock = 1;
      end
   end

reg   [31:0]    testdata;

always @(posedge clock) begin
   if (reset) begin
      testdata =  32'h 03020100;
      #1;
      pdi      =  testdata;
      end
   else if (pwr && pir) begin
      testdata = testdata + 32'h 04040404;
      #1;
      pdi      =  testdata;
      end
   end

initial begin
   $display (" FACTOR = %d, LX = %d, WIDTH = %d", FACTOR, mut.LX, WIDTH);
      pdi      =  0;
      pwr      =  0;
      prd      =  0;
      flush    =  0;
      reset    =  0;
      testdata =  0;
   @(posedge clock);
   #1 reset    =  1;
   #(TICK * 3);
      reset    =  0;
   #(TICK*10);            // test
      prd      =  1;      // test
   #(TICK*10);
      pwr      =  1;
      prd      =  1;
   #(TICK*9);
      prd      =  0;
   #(TICK*10);
      prd      =  1;
   #(TICK*10);
   repeat (8) begin
      pwr      =  0;
      #(TICK*4);
      pwr      =  1;
      #(TICK*4);
      end
   #(TICK*2);
      pwr      =  1;
      prd      =  1;
   #(TICK);
      flush    =  1;
   #(TICK);
      flush    =  0;
   #(TICK*10);
      prd      =  0;
   #(TICK*10);

   $finish;
   end

endmodule

`endif
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
