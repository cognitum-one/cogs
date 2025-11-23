/* ==============================================================================================================================

   Copyright © 1996..2025 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : Clocked Square Sparrow Prefix Arbiter that outputs first 2 winners

   Creation Date        : 5/15/1999

   Added global enable  : 20250319 RTT

=================================================================================================================================
      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
=================================================================================================================================

   For a four "request" example the module will generate, for each request:

   win[n]  = request[n] & grant[(n+0)%4] & ~request[(n+1)%4] & ~request[(n+2)%4] & ~request[(n+3)%4]
           | request[n] & grant[(n+1)%4] & ~request[(n+2)%4] & ~request[(n+3)%4]
           | request[n] & grant[(n+2)%4] & ~request[(n+3)%4]
           | request[n] & grant[(n+3)%4]

   For Square-Sparrow operation:    The grant signal should be the winner of the previous access

===============================================================================================================================*/

`ifdef  synthesis //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_defines.vh"
`else
   `include "A2_project_settings.vh"
   `include "A2_defines.vh"
`endif

//`ifndef  tCQ
//`define  tCQ   #1
//`endif
//`include "A2_defines.vh"

module A2_arbiter_for2 #(
   parameter N = 8  // Number of requestors (can be any value)
   )(
   output reg  [N-1:0]  winner1,    // First granted request (one-hot)
   output reg  [N-1:0]  winner2,    // Second granted request (one-hot)
   input  wire [N-1:0]  request,    // Request vector (one-hot)
   input  wire          advance,    // Advance priority control
   input  wire          enable,     // Enable arbitration
   input  wire          clock,
   input  wire          reset       // Synchronous
   );

localparam  LN = `LG2(N);

integer i,k;
reg [LN-1:0] found, last;                    // Holds last winner for next cycle
reg          flag1, flag2;

always @(posedge clock)
   if      ( reset ) last <= `tCQ  0;        // Reset priority to lowest bit
   else if (advance) last <= `tCQ  found;    // Store last winner as next priority

always @* begin
   winner1  = {N{1'b0}};
   winner2  = {N{1'b0}};
   flag1    =    1'b0;
   flag2    =    1'b0;
   found    =  last;
   for (i=0;i<N;i=i+1) begin
      k = (last + i + 1) %N;
      if (enable && !flag1 && request[k]) begin
         winner1[k]  = 1'b1;
         flag1       = 1'b1;
         found       = k;
         end
      else if (enable && flag1 && !flag2 && request[k]) begin
         winner2[k]  = 1'b1;
         flag2       = 1'b1;
         found       = k;
         end
      end
   end

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
`ifdef   verify_arb_for_2
`timescale 1ns / 1ps

module t;
    parameter N = 8; // Test with an 8-input arbiter
    reg          clk, rst;
    reg          enable, advance;
    reg  [N-1:0] req;
    wire [N-1:0] grant1, grant2;

    // Instantiate the Square Sparrow arbiter
    A2_arbiter_for2 #(.N(N)) dut (
        .clock  (clk),
        .reset  (rst),
        .enable (enable),
        .advance(advance),
        .request(req),
        .winner1(grant1),
        .winner2(grant2)
    );

    // Clock generation
    always #5 clk = ~clk;

    initial begin
        $display("Starting Square Sparrow Arbiter Test...");
        clk = 0;
        rst = 1;
        req = 0;
        enable    = 1;
        advance   = 1;
        @(posedge clk); #1;
        // Apply reset
        rst = 0;

        @(posedge clk); #1;
        // Test 1: Single request active
        req = 8'b00000001;
        #5 $display("Last: %h | Req: %b | Grant1: %b | Grant2: %b", dut.found, req, grant1, grant2);

        @(posedge clk); #1;
        // Test 2: Two adjacent requests
        req = 8'b00000011;
        #5 $display("Last: %h | Req: %b | Grant1: %b | Grant2: %b", dut.found, req, grant1, grant2);

        @(posedge clk); #1;
        // Test 3: Random multiple requests
        req = 8'b10101010;
        #5 $display("Last: %h | Req: %b | Grant1: %b | Grant2: %b", dut.found, req, grant1, grant2);

        @(posedge clk); #1;
        // Test 4: All requests active
        req = 8'b11111111;
        #5 $display("Last: %h | Req: %b | Grant1: %b | Grant2: %b", dut.found, req, grant1, grant2);

        @(posedge clk); #1;
        // Test 5: Sparse requests
        req = 8'b10001000;
        #5 $display("Last: %h | Req: %b | Grant1: %b | Grant2: %b", dut.found, req, grant1, grant2);

        @(posedge clk); #1;
        // Test 6: Reset during operation
        rst = 1;
        req = 8'b00001100;
        #5 $display("Last: %h | Req: %b | Grant1: %b | Grant2: %b (After Reset)", dut.found, req, grant1, grant2);
        @(posedge clk); #1;
        #10 rst = 0;

        // Finish test
        #20 $finish;
    end
endmodule

`endif

