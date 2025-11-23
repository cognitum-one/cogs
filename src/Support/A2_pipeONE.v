//===============================================================================================================================
//
//    Copyright © 1996..2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name   : Pipeline
//
//    Description    : Stallable pipestage
//                     This is a pipestage module that when linked in a chain creates a FIFO that may ppl_stall in any stage
//                     This can be used as a FIFO or as a ppl_stallable pipeline with logic between each stage.
//
//                     This is not really a pipe.  It is a single register that after load sets its state machine to not
//                     accept and further writes unbtil it has been read-and-cleared.
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//==============================================================================================================================

`ifndef  tCQ
   `define  tCQ #1
`endif

module A2_pipeONE #(
   parameter               WIDTH = 8
   )(
   // Input
   input  wire [WIDTH-1:0] pdi,     // Data in
   input  wire             pwr,     // Like a FIFO push
   output wire             pir,     // Like a FIFO not full  (Input Ready)
   // Output
   output wire [WIDTH-1:0] pdo,     // Data out
   output wire             por,     // Like a FIFO not empty (Output Ready)
   input  wire             prd,     // Like a FIFO pop
   // Global
   input                   clock,
   input                   reset
   );

reg   [WIDTH-1:0]  Pipe;            // Pipeline register only one deep
reg                State;           // State Machine ( one bit! )

always @(posedge clock)
   if      (!State && pwr )   Pipe[WIDTH-1:0] <= `tCQ  pdi[WIDTH-1:0];

always @(posedge clock)
   if      (reset)            State <= `tCQ 1'b0;
   else if (!State && pwr)    State <= `tCQ 1'b1;
   else if ( State && prd)    State <= `tCQ 1'b0;

assign
   pir            = ~State,
   por            =  State,
   pdo[WIDTH-1:0] =  Pipe[WIDTH-1:0];

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Set a command line `define to "verify_A2_pipe" to do a standalone module test on this file
`ifdef   verify_A2_pipeINJ
`define  MODE_SELECTABLE

module t;

parameter   TICK  = 10;
parameter   WIDTH =  8;

reg   [WIDTH-1:0] tip0_data;
reg               tip0_give;

wire  [WIDTH-1:0] pipe1;
wire  [WIDTH-1:0] pipe2;
wire  [WIDTH-1:0] pipe3;

wire              p0p1_give;
wire              p1p2_give;
wire              p2to_give;

wire              tip0_take;
wire              p1p0_take;
reg               p2p1_take;

reg               stall_0_1;
reg               stall_1_2;
reg               ppl_stall3_4;

reg               top2_take;
reg   [WIDTH-1:0] check;
reg               clock;
reg               reset;

integer  errors;

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////


A2_pipe p0 (               //    ^    V
   .pdi     (tip0_data),   //    |    |
   .pwr     (tip0_give),   // <--|----+
   .pir     (tip0_take),   // ---+
                           //
   .pdo     (pipe1    ),   //
   .por     (p0p1_give),   // --------+
   .prd     (p1p0_take),   // <--+    |
                           //    |    |
   .clock   (clock),       //    |    |
   .reset   (reset)        //    |    give
   );                      //    |    |
                           //    take |
A2_pipeINJ p1 (            //    |    |
   .ppl_di   (pipe1    ),  //    |    |
   .ppl_wr   (p0p1_give),  // <--|----+
   .ppl_ir   (p1p0_take),  // ---+
                           //
   .ppl_do   (pipe2    ),  //
   .ppl_or   (p1p2_give),  // --------+
   .ppl_rd   (p2p1_take),  // <--+    |
                           //    take give
   .clock    (clock),      //    |    |
   .reset    (reset)       //    ^    V
   );


////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

initial begin
   clock = 1'b0;
   forever clock = #(TICK/2) ~clock;
   end

initial begin
   reset       =  1'b0;
   p2p1_take   =  1'b0;
   repeat (10)    @(posedge clock);
   reset    =  1'b1;
   repeat (10)    @(posedge clock);
   reset    =  1'b0;
   repeat (10)    @(posedge clock);

   // TEST 1: Load it all up and remove it all
   tip0_data    =  144;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  53;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  121;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  74;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   p2p1_take    =  1'b1;
   repeat (4)     @(posedge clock);
   p2p1_take    =  1'b0;
   repeat (40)    @(posedge clock);

   // TEST 2: Check for rejections
   tip0_data    =  205;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  5;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  109;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  63;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  63;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  63;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  12;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   p2p1_take    =  1'b1;
   repeat (5)     @(posedge clock);
   p2p1_take    =  1'b0;
   repeat (40)    @(posedge clock);

   // TEST 2: Check for other rejections
   tip0_data    =  105;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  105;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  105;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  105;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  79;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  31;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   tip0_data    =  253;
   tip0_give    =  1'b1;
   repeat (1)     @(posedge clock);
   tip0_give    =  1'b0;
   repeat (8)     @(posedge clock);
   p2p1_take    =  1'b1;
   repeat (4)     @(posedge clock);
   p2p1_take    =  1'b0;
   repeat (40)    @(posedge clock);

   $display("\n TEST COMPLETED \n");
   $stop();
   end

endmodule

`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

