/********************************************************************************************************************************

   Copyright © 2012 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         :  A2 Library

   Description          :  Radix-2 Serial Multiplier-Divider.

*********************************************************************************************************************************

   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

********************************************************************************************************************************

   NOTES:
   Multiply:   Signed Double length result of signed left * signed right operands
   Divide:     Signed Quotient in lower half of result and Signed Remainder in upper half of result of
               Signed left / Signed Right operands

********************************************************************************************************************************/

`ifdef  synthesis //JLPL
//   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
//   `include "A2_project_settings.vh"
`endif

module A2_div_r2 (
   output wire [63:0]   result,  // Double Length  // Multiply (Double Length Product, Divide (Remainder, Quotient)
   output wire          ordy,    // Result ready
   input  wire          read,    // Read Result

   output wire          irdy,    // Ready for new job
   input  wire [31:0]   left,    // LHS Operand
   input  wire [31:0]   right,   // RHS Operand
   input  wire          load,    // Write operands and command
   input  wire [ 1:0]   fn,      // Multiply SS,SU,US,UU, divide SS,SU,US,UU

   input  wire          reset,
   input  wire          clock
   );

// Registers --------------------------------------------------------------------------------------------------------------------
reg  [2:0]  FSM;  localparam [2:0]  IDLE = 0,  RUN = 1, LAST = 2,  DONE  = 3;

reg  [63:0] A, B, R;
reg  [ 4:0] C;
reg         NegL, NegE;

// Main State Machine -----------------------------------------------------------------------------------------------------------
always @(posedge clock)
   if (reset)           FSM   <= `tCQ IDLE;
   else (* parallel_case *) case (FSM)
      IDLE  : if (load) begin
                        NegL  <= `tCQ  fn[1] & left[31];
                        NegE  <= `tCQ (fn[1] & left[31]) ^ (fn[0] & right[31]);
                        C     <= `tCQ  31;
                        FSM   <= `tCQ  RUN;
                        end
      RUN   : if (C==0) FSM   <= `tCQ LAST;
              else      C     <= `tCQ C - 1'b1;
      LAST  :           FSM   <= `tCQ DONE;
      default if (read) FSM   <= `tCQ IDLE;   // 'DONE'
      endcase

wire  run   = (FSM == RUN ),
      last  = (FSM == LAST);

// Sign Conversions, Add & Compare ---------------------------------------------------------------------------------------------------
wire [63:0] chsR  =  NegE     ? -R           : R;
wire [31:0] chsA  =  NegL     ? -A           : A,
            absL  =  fn[1] &&  left[31] ? -left [31:0] :  left[31:0],
            absR  =  fn[0] && right[31] ? -right[31:0] : right[31:0];
wire [63:0] sum   =  A + ~B  + 1'b1;
wire        ageb  =  A >= B;

// Registers --------------------------------------------------------------------------------------------------------------------
always @(posedge clock) begin
   if (irdy &&  load) R[63:0]  <= `tCQ 64'h0;
   if (run  &&  ageb) R[C]     <= `tCQ 1'b1;
   if (last         ) R[63:0]  <= `tCQ {chsA[31:0], chsR[31:0]};
   end

always @(posedge clock) begin
   if (irdy &&  load) A[63:0]  <= `tCQ { 32'h0, absL };
   if (run  &&  ageb) A[63:0]  <= `tCQ sum;
   end

always @(posedge clock) begin
   if (irdy &&  load) B[63:0]  <= `tCQ { absR,  32'h0 } >> 1;
   if (run          ) B[63:0]  <= `tCQ B[63:0] >> 1;
   end

// Outputs ---------------------------------------------------------------------------------------------------------------------
assign irdy    = (FSM == IDLE),
       ordy    = (FSM == DONE),
       result  =  R[63:0];

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
`ifdef   verify_A2_div_r2

module   t;

localparam     iterations = 1000000;
integer        errors;
integer        s00count, s01count, s10count, s11count;
reg   [33:0]   i,j,k,r;

wire  [63:0]   result;
wire           done;
wire           idle;
reg            read;
reg            write;
reg   [31:0]   left,  right;
reg   [ 1:0]   fn;
reg            clock;
reg            reset;

A2_div_r2 mut (
   .result  (result),
   .ordy    (done),
   .read    (read),
   .irdy    (idle),
   .load    (write),
   .left    (left),
   .right   (right),
   .fn      (fn),
   .reset   (reset),
   .clock   (clock)
   );

initial begin
   clock = 0;
   forever
      clock = #10 ~clock;
   end

initial begin
   reset =  1;
   repeat (10) @(posedge clock); #1;
   reset =  0;
   repeat (10) @(posedge clock); #1;

   s00count =  0;
   s01count =  0;
   s10count =  0;
   s11count =  0;
   errors   =  0;

   $display("\n BEGIN RANDOM DIVIDE TESTS");

   i        =  0;
   j        =  0;
   repeat (10) @(posedge clock);
   for (k=0; k<iterations; k=k+1) begin : D_loop
      i= ($random & 32'h ffff_ffff);
      j= ($random & 32'h ffff_ffff);
      for (r=0; r<4; r=r+1) begin : Div_loop
         fn    = r[1:0];
         left  = i;
         right = j;
         while  (!idle)
         repeat (01) @(posedge clock); #1;   write = 1;
         repeat (01) @(posedge clock); #1;   write = 0;
         while  (!done) repeat (01) @(posedge clock); #1;
         divide_test(left,right);
         repeat (10) @(posedge clock); #1;   read = 1;
         repeat (01) @(posedge clock); #1;   read = 0;
         if (errors > 16)  disable Div_loop;
         end
      end


   $display("\n COMPLETED %5d RANDOM TESTS WITH %5d ERRORS\n",8*iterations, errors);

   $finish;
   end

task divide_test;
input [31:0] LHS;
input [31:0] RHS;
reg   [63:0] divisor, dividend, expected_quotient, expected_remainder, quo, rem;
reg          opposite_signs;
   begin
   dividend =   fn[1] & LHS[31] ? {32'h0, -LHS[31:0]} : {32'h0,LHS[31:0]};
   divisor  =   fn[0] & RHS[31] ? {32'h0, -RHS[31:0]} : {32'h0,RHS[31:0]};
   opposite_signs     = (fn[1] && LHS[31]) ^ (fn[0] & RHS[31]);
   quo                =  dividend  /  divisor;
   expected_quotient  =  opposite_signs ? -quo : quo;
   rem                =  dividend  %  divisor;
   expected_remainder = (fn[1] && LHS[31]) ? -rem : rem;

   if ( result[31:00] !== expected_quotient[31:0])  begin
      errors = errors +1;
      case (fn[1:0])
         2'b00: $display(" TEST %5d UU QUOTIENT  ERROR: %h / %h should be %h, received %h",k,dividend,divisor,expected_quotient,result[31:0]);
         2'b01: $display(" TEST %5d US QUOTIENT  ERROR: %h / %h should be %h, received %h",k,dividend,divisor,expected_quotient,result[31:0]);
         2'b10: $display(" TEST %5d SU QUOTIENT  ERROR: %h / %h should be %h, received %h",k,dividend,divisor,expected_quotient,result[31:0]);
         2'b11: $display(" TEST %5d SS QUOTIENT  ERROR: %h / %h should be %h, received %h",k,dividend,divisor,expected_quotient,result[31:0]);
         endcase
      end
   else if (k%100000 == 0)
      case (fn[1:0])
         2'b00: $display(" TEST %5d UU QUOTIENT  CORRECT %h / %h should be %h, received %h",k,dividend,divisor,expected_quotient,result[31:0]);
         2'b01: $display(" TEST %5d US QUOTIENT  CORRECT %h / %h should be %h, received %h",k,dividend,divisor,expected_quotient,result[31:0]);
         2'b10: $display(" TEST %5d SU QUOTIENT  CORRECT %h / %h should be %h, received %h",k,dividend,divisor,expected_quotient,result[31:0]);
         2'b11: $display(" TEST %5d SS QUOTIENT  CORRECT %h / %h should be %h, received %h",k,dividend,divisor,expected_quotient,result[31:0]);
         endcase

   if ( result[63:32] !== expected_remainder[31:0]) begin
      errors = errors +1;
      case (fn[1:0])
         2'b00: $display(" TEST %5d UU REMAINDER ERROR: %h %% %h should be %h, received %h\n",k,dividend,divisor,expected_remainder,result[63:32]);
         2'b01: $display(" TEST %5d US REMAINDER ERROR: %h %% %h should be %h, received %h\n",k,dividend,divisor,expected_remainder,result[63:32]);
         2'b10: $display(" TEST %5d SU REMAINDER ERROR: %h %% %h should be %h, received %h\n",k,dividend,divisor,expected_remainder,result[63:32]);
         2'b11: $display(" TEST %5d SS REMAINDER ERROR: %h %% %h should be %h, received %h\n",k,dividend,divisor,expected_remainder,result[63:32]);
         endcase
      end
   else if (k%100000 == 0)
      case (fn[1:0])
         2'b00: $display(" TEST %5d UU REMAINDER CORRECT %h %% %h should be %h, received %h\n",k,dividend,divisor,expected_remainder,result[63:32]);
         2'b01: $display(" TEST %5d US REMAINDER CORRECT %h %% %h should be %h, received %h\n",k,dividend,divisor,expected_remainder,result[63:32]);
         2'b10: $display(" TEST %5d SU REMAINDER CORRECT %h %% %h should be %h, received %h\n",k,dividend,divisor,expected_remainder,result[63:32]);
         2'b11: $display(" TEST %5d SS REMAINDER CORRECT %h %% %h should be %h, received %h\n",k,dividend,divisor,expected_remainder,result[63:32]);
         endcase
   end
endtask

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
