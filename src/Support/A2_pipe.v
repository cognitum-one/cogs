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
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//==============================================================================================================================

`ifdef  synthesis //JLSW //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

//`timescale     1ns/1ps //JLPL
//`define  tCQ   #0.005

module A2_pipe #(
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
   if      ( pwr &&          FSM[MPT]
          || pwr &&  prd &&  FSM[ONE]) Pipe[WIDTH-1:0] <= `tCQ   pdi[WIDTH-1:0];
   else if ( prd &&          FSM[TWO]) Pipe[WIDTH-1:0] <= `tCQ  Skid[WIDTH-1:0];
   end

always @(posedge clock) begin
   if (reset)                       FSM[2:0] <= `tCQ 3'd1 << MPT;
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
`ifdef   verify_A2_pipe

module t;

parameter   TICK  = 250;
parameter   WIDTH =   8;

reg               tip0_give, top2_take;
reg   [WIDTH-1:0] tip0_data, check;

wire  [WIDTH-1:0] p0p1_data;
wire  [WIDTH-1:0] p1p2_data;
wire  [WIDTH-1:0] p2to_data;

wire                         p0ti_take;
wire              p0p1_give, p1p0_take;
wire              p1p2_give, p2p1_take;
wire              p2to_give;

reg               stall_0_1;
reg               stall_1_2;
reg               stall_2_3;

reg               clock;
reg               reset;

integer  errors;

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

A2_pipe #(8) p0 (
   .pdo  (p0p1_data),                 // out
   .por (p0p1_give),                 // out
   .pir (p0ti_take),                 // out
   .pdi  (tip0_data),                 // in
   .pwr  (tip0_give),                 // in
   .prd  (p1p0_take & ~stall_0_1),    // in
   .clock (clock),
   .reset (reset)
   );

A2_pipe #(8) p1 (
   .pdo  (p1p2_data),                 // out
   .por (p1p2_give),                 // out
   .pir (p1p0_take),                 // out
   .pdi  (p0p1_data),                 // in
   .pwr  (p0p1_give & ~stall_0_1),    // in
   .prd  (p2p1_take & ~stall_1_2),    // in
   .clock (clock),
   .reset (reset)
   );

A2_pipe #(8) p2 (
   .pdo  (p2to_data),                 // out
   .por (p2to_give),                 // out
   .pir (p2p1_take),                 // out
   .pdi  (p1p2_data),                 // in
   .pwr  (p1p2_give & ~stall_1_2),    // in
   .prd  (top2_take & ~stall_2_3),    // in
   .clock (clock),
   .reset (reset)
   );

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

initial begin
   clock                      =  1;
   forever  #(TICK/2)   clock = ~clock;
   end

always @(posedge clock) begin
   if (reset) begin
      tip0_data <= 0;
      end
   else if (tip0_give && p0ti_take) begin
      tip0_data <= tip0_data +1;
      end
   end

always @(posedge clock) begin
   if (reset) begin
      check <= 0;
      end
   else if (p2to_give && top2_take && !stall_2_3) begin
      check <= check +1;
      end
   end

always @(negedge clock) begin
   if (p2to_give && top2_take) begin
      if (p2to_data !== check) begin
         $display ("ERROR: Expected %h, monitored %h", check, p2to_data);
         errors = errors +1;
         end
      end
   if (errors > 10) begin
      $display("\n TOO MANY ERRORS : TEST FAILED \n");
      $stop();
      $finish();
      end
   end

initial begin
   errors      =  0;
   reset       =  0;
   tip0_give   =  0;
   top2_take   =  0;
   stall_0_1   =  0;
   stall_1_2   =  0;
   stall_2_3   =  0;
   @(negedge clock);
   reset       =  1;
   repeat (3) @(negedge clock);
   reset       =  0;
   repeat (3) @(negedge clock);

   // TEST 1: Load it all up and remove it all
   tip0_give    =  1;
   repeat (8) @(negedge clock);
   tip0_give    =  0;
   top2_take    =  1;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   repeat (32) @(negedge clock);

   // TEST 2: Run both ends together
   tip0_give    =  1;
   top2_take    =  1;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   tip0_give    =  0;
   repeat (8) @(negedge clock);
   tip0_give    =  1;
   top2_take    =  1;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   tip0_give    =  0;
   repeat (8) @(negedge clock);
   tip0_give    =  1;
   top2_take    =  1;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   tip0_give    =  0;
   repeat (32) @(negedge clock);

   // TEST 3: Pull all data through
   top2_take    =  1;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   repeat (32) @(negedge clock);

   // TEST 4: Set the pipe running and try a ppl_stall
   tip0_give    =  1;
   top2_take    =  1;
   repeat (8) @(negedge clock);
   stall_0_1 =  1;
   repeat (4) @(negedge clock);
   stall_0_1 =  0;
   repeat (8) @(negedge clock);
   tip0_give    =  0;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   repeat (32) @(negedge clock);

   // TEST 5: Try a different ppl_stall
   tip0_give    =  1;
   top2_take    =  1;
   repeat (8) @(negedge clock);
   stall_1_2 =  1;
   repeat (4) @(negedge clock);
   stall_1_2 =  0;
   repeat (8) @(negedge clock);
   tip0_give    =  0;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   repeat (32) @(negedge clock);

   // TEST 6: Try concurrent ppl_stalls
   tip0_give    =  1;
   top2_take    =  1;
   repeat (8) @(negedge clock);
   stall_0_1 =  1;
   stall_1_2 =  1;
   repeat (4) @(negedge clock);
   stall_0_1 =  0;
   stall_1_2 =  0;
   repeat (8) @(negedge clock);
   tip0_give    =  0;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   repeat (32) @(negedge clock);

   // TEST 7: Try skewed concurrent ppl_stalls
   tip0_give    =  1;
   top2_take    =  1;
   repeat (8) @(negedge clock);
   stall_0_1 =  1;
   repeat (1) @(negedge clock);
   stall_1_2 =  1;
   repeat (4) @(negedge clock);
   stall_0_1 =  0;
   repeat (1) @(negedge clock);
   stall_1_2 =  0;
   repeat (8) @(negedge clock);
   tip0_give    =  0;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   repeat (32) @(negedge clock);

   // TEST 8: Try nested concurrent ppl_stalls
   tip0_give    =  1;
   top2_take    =  1;
   repeat (8) @(negedge clock);
   stall_0_1 =  1;
   repeat (1) @(negedge clock);
   stall_1_2 =  1;
   repeat (1) @(negedge clock);
   stall_1_2 =  0;
   repeat (1) @(negedge clock);
   stall_0_1 =  0;
   repeat (8) @(negedge clock);
   tip0_give    =  0;
   repeat (8) @(negedge clock);
   top2_take    =  0;
   repeat (32) @(negedge clock);

   // TEST 10: Try single steps etc..
   tip0_give    =  1;
   top2_take    =  0;
   repeat (10) @(negedge clock);
   repeat (6)  begin
      top2_take    =  1;
                  @(negedge clock);
      top2_take    =  0;
      repeat (4)  @(negedge clock);
      end
   tip0_give    =  0;
   top2_take    =  1;
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

