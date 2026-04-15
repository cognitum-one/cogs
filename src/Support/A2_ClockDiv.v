// ==============================================================================================================================
//
//    Copyright © 2022 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : Edradour
//
//    Description          : Programmable Clock Divider
//
// ==============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
// ==============================================================================================================================
//
//    Johnson Ring Counters:
//         16        15        14       13       12      11      10     9      8     7     6    5    4   3   2
//      0  00000000            0000000           000000          00000         0000        000       00      0
//      1  00000001  00000001  0000001  0000001  000001  000001  00001  00001  0001  0001  001  001  01  01  1
//      2  00000011  00000011  0000011  0000011  000011  000011  00011  00011  0011  0011  011  011  11  11
//      3  00000111  00000111  0000111  0000111  000111  000111  00111  00111  0111  0111  111  111  10  10
//      4  00001111  00001111  0001111  0001111  001111  001111  01111  01111  1111  1111  110  110
//      5  00011111  00011111  0011111  0011111  011111  011111  11111  11111  1110  1110  100  100
//      6  00111111  00111111  0111111  0111111  111111  111111  11110  11110  1100  1100
//      7  01111111  01111111  1111111  1111111  111110  111110  11100  11100  1000  1000
//      8  11111111  11111111  1111110  1111110  111100  111100  11000  11000
//      9  11111110  11111110  1111100  1111100  111000  111000  10000  10000
//      10 11111100  11111100  1111000  1111000  110000  110000
//      11 11111000  11111000  1110000  1110000  100000  100000
//      12 11110000  11110000  1100000  1100000
//      13 11100000  11100000  1000000  1000000
//      14 11000000  11000000
//      15 10000000  10000000
//
// ==============================================================================================================================
//
//    Prototype:
//
//  A2_ClockDiv i_ckDv (
//     // System Interconnect
//     .iCD        (iCD),
//     .wCD        (wCD),
//     // Local Interconnect
//     .rCD        (rCD),
//     .CDo        ( CDo),
//     // Global
//     .div_clock  (slow_clock),
//     .db2_clock  (fast_clock),
//     .ref_clock  (ref_clock),
//     .xclock     (xclock),
//     );
//
//  Control Register C [7:4]
//     0000  :   xclock                            xclock
//     0001  :   ref_clock DIV 2          12 / 2   = 6
//     0010  :   ref_clock DIV 3          12 / 3   = 4
//     0011  :   ref_clock DIV 4          12 / 4   = 3
//     0100  :   ref_clock DIV 5          12 / 5   = 2.4
//     0101  :   ref_clock DIV 6          12 / 6   = 2
//     0110  :   ref_clock DIV 7          12 / 7   = 1.714285r
//     0111  :   ref_clock DIV 8          12 / 8   = 1.5
//     1000  :   ref_clock DIV 9          12 / 9   = 1.333333r
//     1001  :   ref_clock DIV 10         12 / 10  = 1.2
//     1010  :   ref_clock DIV 11         12 / 11  = 1.090909r
//     1011  :   ref_clock DIV 12         12 / 12  = 1.0
//     1100  :   ref_clock DIV 13         12 / 13  = 0.923076r
//     1101  :   ref_clock DIV 14         12 / 14  = 0.857142r
//     1110  :   ref_clock DIV 15         12 / 15  = 0.8
//     1111  :   ref_clock DIV 16         12 / 16  = 0.75
//
// ==============================================================================================================================

module A2_ClockDiv (
   // System Interconnect
   input  wire [3:0]       iCD,           // Input data
   input  wire             wCD,           // Write synchronised to xclock
   // Local Interconnect
   input  wire             rCD,           // Asynchonous Read
   output wire [3:0]        CDo,          // Output data
   // Global
   output wire             div_clock,     // Resulting divided down clock
   output wire             db2_clock,     // Divide by 2 ref clock
   output wire             db3_clock,     // Divide by 3 ref clock
   input  wire             ref_clock,     // Reference clock to be divided down
   input  wire             resetn,
   input  wire             clock          // Control clock: used to load Control Register
   );

// Control register -------------------------------------------------------------------------------------------------------------
reg    [3:0] c;
always @(posedge clock or negedge resetn)
   if (!resetn)
      c[3:0]   <= 4'b0000;
   else if (wCD)
      c[3:0]   <= iCD[3:0];

assign   CDo   =  rCD ? c[3:0] : 4'b0000;

reg  [15:0] divctrl;    /* false path  'reg [3:0] c' through 'ckctrl' to 'reg[7:0] ring'  */

always @* case(c[3:0])
   //                        ck_enable  masking
   //                        111111
   //                        54321098 76543210
   4'b0000 :  divctrl  = 16'b00000000_11111111;     // DIV 1 -- Use xclock
   4'b0001 :  divctrl  = 16'b00000001_11111110;     // DIV 2    hold bit 7,6,5,4,3,2,1
   4'b0010 :  divctrl  = 16'b00000011_11111100;     // DIV 3    hold bit 7,6,5,4,3,2
   4'b0011 :  divctrl  = 16'b00000011_11111101;     // DIV 4    hold bit 7,6,5,4,3,2
   4'b0100 :  divctrl  = 16'b00000111_11111001;     // DIV 5    hold bit 7,6,5,4,3
   4'b0101 :  divctrl  = 16'b00000111_11111011;     // DIV 6    hold bit 7,6,5,4,3
   4'b0110 :  divctrl  = 16'b00001111_11110011;     // DIV 7    hold bit 7,6,5,4
   4'b0111 :  divctrl  = 16'b00001111_11110111;     // DIV 8    hold bit 7,6,5,4
   4'b1000 :  divctrl  = 16'b00011111_11100111;     // DIV 9    hold bit 7,6,5
   4'b1001 :  divctrl  = 16'b00011111_11101111;     // DIV 10   hold bit 7,6,5
   4'b1010 :  divctrl  = 16'b00111111_11001111;     // DIV 11   hold bit 7,6
   4'b1011 :  divctrl  = 16'b00111111_11011111;     // DIV 12   hold bit 7,6
   4'b1100 :  divctrl  = 16'b01111111_10011111;     // DIV 13   hold bit 7
   4'b1101 :  divctrl  = 16'b01111111_10111111;     // DIV 14   hold bit 7
   4'b1110 :  divctrl  = 16'b11111111_00111111;     // DIV 15
   4'b1111 :  divctrl  = 16'b11111111_01111111;     // DIV 16
   endcase

// Divider register -------------------------------------------------------------------------------------------------------------

reg   [7:0] ring;
reg   [1:0] dv3_clock;
reg         hlf_clock;
wire        feedback = ~&(ring[7:0] | divctrl[7:0]);

//always @(posedge ref_clock)
   //hlf_clock   <=  wCD ? 1'b0 : ~hlf_clock;

always @(posedge ref_clock or posedge wCD )
   if (wCD) begin
      ring[7:0]   <=   8'h0;
      dv3_clock   <=   2'b00;
      hlf_clock   <=   1'b0;
      end
   else begin
      hlf_clock   <=  ~hlf_clock;
      dv3_clock   <=  {dv3_clock[0], ~&dv3_clock[1:0]};
      if (divctrl[15] && !hlf_clock)  ring[7]  <=   ring[6];
      if (divctrl[14] && !hlf_clock)  ring[6]  <=   ring[5];
      if (divctrl[13] && !hlf_clock)  ring[5]  <=   ring[4];
      if (divctrl[12] && !hlf_clock)  ring[4]  <=   ring[3];
      if (divctrl[11] && !hlf_clock)  ring[3]  <=   ring[2];
      if (divctrl[10] && !hlf_clock)  ring[2]  <=   ring[1];
      if (divctrl[09] && !hlf_clock)  ring[1]  <=   ring[0];
      if (divctrl[08] && !hlf_clock)  ring[0]  <=   feedback;
      end

// Outputs ---------------------------------------------------------------------------------------------------------------------
wire  sel_ctl_clock  =  c[3:0] == 4'b0000;

assign   db2_clock   =  sel_ctl_clock ? clock : hlf_clock;
assign   db3_clock   =  sel_ctl_clock ? clock : dv3_clock;
assign   div_clock   =  sel_ctl_clock ? clock : ring[0];

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

`ifdef   verify_CD
`timescale  1ps/1fs

module t;

reg   [3:0] iCD;
reg         rCD;
reg         wCD;
reg         ref_clock, xclock, sel_clock, mod_clock;
reg         xresetn;

wire  [3:0] CDo;
wire        CDk;
wire        div_clock, db2_clock;
integer     i;

A2_ClockDiv (
   // System Interconnect
   .iCD(iCD[3:0]),           // Input data
   .wCD(wCD),
   // Local Interconnect
   .rCD(rCD),
   .CDo(CDo[3:0]),
   // Global
   .div_clock(div_clock),     // Resulting divided down clock
   .db2_clock(db2_clock),     // Divide by 2 ref clock
   .ref_clock(ref_clock),     // Reference clock to be divided down
   .xclock   (xclock   )      // Control clock: used to load Control Register
   );


initial begin
   xclock = 0;
   forever
      xclock = #(100) ~xclock;
   end

initial begin
   ref_clock = 0;
   forever
      ref_clock = #(41.666666666) ~ref_clock;
   end

initial begin
   $display("Test Setup");
   iCD      =  4'h0;
   rCD      =  1'b0;
   wCD      =  1'b0;
   @(posedge xclock);
   wCD      =  1'b1;
   @(posedge xclock);
   wCD      =  1'b0;
   $display(" Loop through all values");
   repeat (64)  @(posedge xclock) #1;
   for (i=0; i<16; i=i+1) begin
      iCD = 4'h0 +  i;
      wCD = 1'b1;
      @(posedge xclock) #1;
      wCD = 1'b0;
      repeat (64)  @(posedge xclock) #1;
      end

   #500;

   $stop;
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

