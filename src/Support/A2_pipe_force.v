//===============================================================================================================================
//
//    Copyright © 1996..2023 Advanced Architectures
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
//                     This is a pipestage module that when linked in a chain creates a FIFO that may stall in any stage
//                     This can be used as a FIFO or as a stallable pipeline with logic between each stage.
//
//                     This version has a force signal that overrides write and input ready
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//==============================================================================================================================

`ifdef  synthesis //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`timescale     1ns/1ps //JLPL
//`ifndef  tCQ
//`define  tCQ   #0.005
//`endif

module A2_pipe_force  #(
   parameter    WIDTH   =  8
   )(
   // Outputs
   output wire [WIDTH-1:0] pdo,           // Output data
   output wire             por,           // Like a FIFO not empty (Output Ready)
   output wire             pir,           // Like a FIFO not full  (Input Ready)
   // Inputs
   input  wire [WIDTH-1:0] pdi,           // Input data
   input  wire             pwr,           // Like a FIFO push      (write)
   input  wire             prd,           // Like a FIFO pop       (read)
   input  wire             pfc,           // Force an input into pipe and set the state machine to ONE

   input  wire             clock,
   input  wire             reset
   );

localparam  MPT   =  0,                   // Pipestage is empty
            ONE   =  1,                   // Pipestage has one word  (in 'pipe')
            TWO   =  2;                   // Pipestage has two words (one in 'skid' one in 'pipe')

reg   [WIDTH-1:0]  Pipe;                  // Pipeline register normally one deep
reg   [WIDTH-1:0]  Skid;                  // Slave register for two deep
reg         [2:0]  FSM;                   // Finite FSM Machine

always @(posedge clock) begin
   if      ( pwr && !prd &&  FSM[ONE]) Skid[WIDTH-1:0] <= `tCQ   pdi[WIDTH-1:0];
   end

always @(posedge clock) begin
   if      ( pfc
          || pwr &&          FSM[MPT]
          || pwr &&  prd &&  FSM[ONE]) Pipe[WIDTH-1:0] <= `tCQ   pdi[WIDTH-1:0];
   else if ( prd &&          FSM[TWO]) Pipe[WIDTH-1:0] <= `tCQ  Skid[WIDTH-1:0];
   end

always @(posedge clock) begin
   if (reset)                       FSM[2:0] <= `tCQ 3'd1 << MPT;
   else if (pfc)                    FSM[2:0] <= `tCQ 3'd1 << ONE;
   else case (1'b 1)
      FSM[MPT]:  if ( pwr         ) FSM[2:0] <= `tCQ 3'd1 << ONE;
      FSM[ONE]:  if ( pwr && !prd ) FSM[2:0] <= `tCQ 3'd1 << TWO;
            else if (!pwr &&  prd ) FSM[2:0] <= `tCQ 3'd1 << MPT;
      FSM[TWO]:  if (         prd ) FSM[2:0] <= `tCQ 3'd1 << ONE;
      default                       FSM[2:0] <= `tCQ 3'bx;
      endcase
   end

assign   pir            = ~FSM[TWO];
assign   por            = ~FSM[MPT];
assign   pdo[WIDTH-1:0] =  Pipe[WIDTH-1:0];

endmodule



////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Set a command line `define to "verify_A2_pipe" to do a standalone module test on this file
`ifdef   verify_pipe_force

module t;

parameter   TICK  = 10;
parameter   WIDTH =  9;

reg               p0_give;
reg   [WIDTH-1:0] p0_data;
wire              p0_take;

wire  [WIDTH-1:0] p0p1_data, p1p2_data, p2p3_data;
wire              p0p1_give, p1p2_give, p2p3_give;
wire              p1p0_take, p2p1_take;
reg                                     p3p2_take;

reg               clock;
reg               reset;

integer  errors;

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

wire  p0_force = p0_give & (p0_data == 9'b101010101);

A2_pipe_force #(9) ip0 (
   // Input
   .pir     (p0_take),                 // out
   .pdi     (p0_data),                 // in
   .pwr     (p0_give),                 // in
   .pfc     (p0_force),                // in
   // Output
   .pdo     (p0p1_data),               // out
   .por     (p0p1_give),               // out
   .prd     (p1p0_take),               // in

   .clock   (clock),
   .reset   (reset)
   );

wire  p0p1_force = p0p1_give & p0p1_data == 9'b101010101;

A2_pipe_force #(9) ip1 (
   // Input
   .pir     (p1p0_take),                 // out
   .pdi     (p0p1_data),                 // in
   .pwr     (p0p1_give),                 // in
   .pfc     (p0p1_force),                // in
   // Output
   .pdo     (p1p2_data),               // out
   .por     (p1p2_give),               // out
   .prd     (p2p1_take),               // in

   .clock   (clock),
   .reset   (reset)
   );

wire  p1p2_force = p0p1_give & p0p1_data == 9'b101010101;

A2_pipe_force #(9) ip2 (
   // Input
   .pir     (p2p1_take),                 // out
   .pdi     (p1p2_data),                 // in
   .pwr     (p1p2_give),                 // in
   .pfc     (p1p2_force),                // in
   // Output
   .pdo     (p2p3_data),               // out
   .por     (p2p3_give),               // out
   .prd     (p3p2_take),               // in

   .clock   (clock),
   .reset   (reset)
   );

wire  p2p3_force = p0p1_give & p0p1_data == 9'b101010101;

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

initial begin
   clock                      =  1;
   forever  #(TICK/2)   clock = ~clock;
   end

initial begin
   errors      =  0;
   reset       =  0;
   @(negedge clock);
   reset       =  1;
   repeat (10) @(negedge clock);
   reset       =  0;
   repeat (1) @(negedge clock);

   // First send normal data
   p3p2_take   =  1;

   p0_give     =  1;
   p0_data     =  9'h055;
   repeat (1) @(negedge clock);
   p0_give     =  1;
   p0_data     =  9'h0aa;
   repeat (1) @(negedge clock);
   p0_give     =  0;
   repeat (10) @(negedge clock);

   // Allow back pressure
   p3p2_take   =  0;

   p0_give     =  1;
   p0_data     =  9'h011;
   repeat (1) @(negedge clock);
   p0_give     =  1;
   p0_data     =  9'h022;
   repeat (1) @(negedge clock);
   p0_give     =  1;
   p0_data     =  9'h033;
   repeat (1) @(negedge clock);
   p0_give     =  1;
   p0_data     =  9'h044;
   repeat (1) @(negedge clock);
   p0_give     =  0;
   repeat (2) @(negedge clock);
   p0_give     =  1;
   p0_data     =  9'h155;
   repeat (1) @(negedge clock);
   p0_give     =  0;
   while (p2p3_data != 9'b101010101) @(negedge clock);
   repeat (1) @(negedge clock);
   p3p2_take   =  1;
   repeat (1) @(negedge clock);
   p0_give     =  1;
   p0_data     =  9'h 66;
   repeat (1) @(negedge clock);
   p0_give     =  1;
   p0_data     =  9'h 77;
   repeat (10) @(negedge clock);

   $display("\n TEST COMPLETED WITH %2d ERRORS \n",errors);
   $stop();
   $finish();
   end

endmodule

`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

